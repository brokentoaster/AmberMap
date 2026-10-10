/*
 *     _             _                __  __
 *    / \   _ __ ___ | |__   ___ _ __ |  \/  | __ _ _ __
 *   / _ \ | '_ ` _ \| '_ \ / _ \ '__|| |\/| |/ _` | '_ \
 *  / ___ \| | | | | | |_) |  __/ |   | |  | | (_| | |_) |
 * /_/   \_\_| |_| |_|_.__/ \___|_|   |_|  |_|\__,_| .__/
 *                                                 |_|
 *
 * AmberMap - High-Performance Bedrock World Map Renderer
 * https://github.com/Amber-PM/AmberMap
 *
 * Copyright (c) 2026 Amber-PM
 * Licensed under Apache-2.0 or MIT
 */

use crate::core::coordinates::ChunkPos;
use crate::core::error::{AmberError, Result};
use crate::leveldb::keys::Dimension;
use crate::leveldb::reader::WorldDb;
use crate::raster::chunk_raster::rasterize_chunk;
use crate::raster::colormap::ColorMap;
use crate::tiler::compositor::TileBuffer;
use crate::tiler::coordinates::TileCoord;
use crate::tiler::encoder::{load_tile, save_tile, tile_path, TileFormat};
use crate::tiler::pyramid::downsample_quadrants;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RenderOptions {
    pub world_path: PathBuf,
    pub output_dir: PathBuf,
    pub dimension: Dimension,
    pub zoom_levels: u8,
    pub format: TileFormat,
    pub threads: usize,
    pub enable_hillshading: bool,
    pub low_memory: bool,
}

pub struct RenderStats {
    pub total_chunks: usize,
    pub base_tiles: usize,
    pub total_tiles: usize,
    pub elapsed: Duration,
}

pub fn render_world_map(options: RenderOptions) -> Result<RenderStats> {
    let start_time = Instant::now();

    if options.threads > 0 {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(options.threads)
            .build_global();
    }

    let mut db = WorldDb::open(&options.world_path)?;
    if options.low_memory {
        db.prune_to_chunk_keys();
    }
    let chunks = db.scan_chunks(options.dimension);

    if chunks.is_empty() {
        return Ok(RenderStats {
            total_chunks: 0,
            base_tiles: 0,
            total_tiles: 0,
            elapsed: start_time.elapsed(),
        });
    }

    let max_zoom = options.zoom_levels.saturating_sub(1);
    let dim_name = match options.dimension {
        Dimension::Overworld => "overworld",
        Dimension::Nether => "nether",
        Dimension::TheEnd => "the_end",
    };

    let dim_dir = options.output_dir.join(dim_name);
    let _ = std::fs::remove_dir_all(&dim_dir);
    let _ = std::fs::create_dir_all(&dim_dir);

    let mut tile_groups: HashMap<(i32, i32), Vec<ChunkPos>> = HashMap::new();
    for chunk in &chunks {
        let coord = TileCoord::from_chunk_pos(*chunk, max_zoom);
        tile_groups
            .entry((coord.x, coord.y))
            .or_default()
            .push(*chunk);
    }

    let colormap = ColorMap::new();
    let total_tiles_count = AtomicUsize::new(0);

    let base_tiles_map: Mutex<HashMap<TileCoord, TileBuffer>> = Mutex::new(HashMap::new());
    let base_tile_bounds: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

    tile_groups
        .par_iter()
        .try_for_each(|(&(tx, ty), chunk_list)| -> Result<()> {
            let mut tile_buf = TileBuffer::new();
            let tile_coord = TileCoord::new(max_zoom, tx, ty);

            for chunk_pos in chunk_list {
                if let Ok(chunk_data) = db.get_chunk(*chunk_pos, options.dimension) {
                    if !chunk_data.subchunks.is_empty() {
                        let (ox, oy) = TileCoord::chunk_offset_in_tile(*chunk_pos);
                        let raster =
                            rasterize_chunk(&chunk_data, &colormap, options.enable_hillshading);
                        tile_buf.blit_chunk(ox, oy, &raster.pixels);
                    }
                }
            }

            if !tile_buf.is_empty() {
                save_tile(
                    &tile_buf,
                    &options.output_dir,
                    dim_name,
                    tile_coord,
                    options.format,
                )?;

                total_tiles_count.fetch_add(1, Ordering::Relaxed);

                {
                    let mut b = base_tile_bounds
                        .lock()
                        .map_err(|_| AmberError::Io(std::io::Error::other("lock poisoned")))?;
                    let next = match *b {
                        None => (tile_coord.x, tile_coord.y, tile_coord.x, tile_coord.y),
                        Some((min_x, min_y, max_x, max_y)) => (
                            min_x.min(tile_coord.x),
                            min_y.min(tile_coord.y),
                            max_x.max(tile_coord.x),
                            max_y.max(tile_coord.y),
                        ),
                    };
                    *b = Some(next);
                }

                if !options.low_memory {
                    let mut map = base_tiles_map
                        .lock()
                        .map_err(|_| AmberError::Io(std::io::Error::other("lock poisoned")))?;
                    map.insert(tile_coord, tile_buf);
                }
            }

            Ok(())
        })?;

    let base_tiles_count = total_tiles_count.load(Ordering::Relaxed);
    let mut current_layer = base_tiles_map.into_inner().unwrap_or_default();

    let (min_tx, max_tx, min_ty, max_ty) = base_tile_bounds
        .into_inner()
        .unwrap_or(None)
        .unwrap_or((0, 0, 0, 0));

    let center_block_x = ((min_tx as i64 + max_tx as i64) * 128 + 128) as i32;
    let center_block_z = ((min_ty as i64 + max_ty as i64) * 128 + 128) as i32;

    let min_block_x = min_tx * 256;
    let min_block_z = min_ty * 256;
    let max_block_x = (max_tx + 1) * 256;
    let max_block_z = (max_ty + 1) * 256;

    let meta_json = format!(
        r#"{{"center_x":{},"center_z":{},"min_block_x":{},"min_block_z":{},"max_block_x":{},"max_block_z":{},"max_zoom":{},"format":"{}"}}"#,
        center_block_x,
        center_block_z,
        min_block_x,
        min_block_z,
        max_block_x,
        max_block_z,
        max_zoom,
        options.format.extension()
    );
    let _ = std::fs::write(dim_dir.join("metadata.json"), &meta_json);
    let _ = std::fs::write(options.output_dir.join("metadata.json"), &meta_json);

    if options.low_memory {
        let disk_tiles =
            build_pyramid_from_disk(&options.output_dir, dim_name, max_zoom, options.format)?;
        total_tiles_count.fetch_add(disk_tiles, Ordering::Relaxed);
    } else if max_zoom > 0 {
        for current_z in (1..=max_zoom).rev() {
            let parent_z = current_z - 1;
            let mut parent_coords: HashMap<(i32, i32), [Option<TileBuffer>; 4]> = HashMap::new();

            for (coord, tile) in &current_layer {
                if let Some(parent) = coord.parent() {
                    let quad = coord.quadrant_in_parent();
                    let quad_idx = match quad {
                        (0, 0) => 0,
                        (1, 0) => 1,
                        (0, 1) => 2,
                        _ => 3,
                    };
                    let entry = parent_coords
                        .entry((parent.x, parent.y))
                        .or_insert_with(|| [None, None, None, None]);
                    entry[quad_idx] = Some(tile.clone());
                }
            }

            let next_layer_mutex: Mutex<HashMap<TileCoord, TileBuffer>> =
                Mutex::new(HashMap::new());

            parent_coords
                .into_par_iter()
                .try_for_each(|((px, py), children)| -> Result<()> {
                    let child_refs = [
                        children[0].as_ref(),
                        children[1].as_ref(),
                        children[2].as_ref(),
                        children[3].as_ref(),
                    ];

                    let parent_tile = downsample_quadrants(&child_refs);
                    let parent_coord = TileCoord::new(parent_z, px, py);

                    if !parent_tile.is_empty() {
                        save_tile(
                            &parent_tile,
                            &options.output_dir,
                            dim_name,
                            parent_coord,
                            options.format,
                        )?;

                        total_tiles_count.fetch_add(1, Ordering::Relaxed);
                        let mut map = next_layer_mutex
                            .lock()
                            .map_err(|_| AmberError::Io(std::io::Error::other("lock poisoned")))?;
                        map.insert(parent_coord, parent_tile);
                    }

                    Ok(())
                })?;

            current_layer = next_layer_mutex.into_inner().unwrap_or_default();
        }
    }

    Ok(RenderStats {
        total_chunks: chunks.len(),
        base_tiles: base_tiles_count,
        total_tiles: total_tiles_count.load(Ordering::Relaxed),
        elapsed: start_time.elapsed(),
    })
}

/// Builds the zoom pyramid by re-reading saved base tiles from disk instead
/// of keeping them all in memory. Only one parent tile and its four children
/// are resident at any time. Returns the number of parent tiles written.
fn build_pyramid_from_disk(
    output_dir: &Path,
    dim_name: &str,
    max_zoom: u8,
    format: TileFormat,
) -> Result<usize> {
    let mut written = 0usize;

    for parent_z in (0..max_zoom).rev() {
        let child_z = parent_z + 1;
        let child_dir = output_dir.join(dim_name).join(child_z.to_string());

        let mut parents: HashMap<(i32, i32), Vec<TileCoord>> = HashMap::new();
        for x_dir in fs::read_dir(&child_dir)?.flatten() {
            let x: i32 = match x_dir.file_name().to_string_lossy().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            for y_file in fs::read_dir(x_dir.path())?.flatten() {
                let name = y_file.file_name().to_string_lossy().into_owned();
                if !name.ends_with(format.extension()) {
                    continue;
                }
                let y: i32 = match name
                    .trim_end_matches(format.extension())
                    .trim_end_matches('.')
                    .parse()
                {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let child = TileCoord::new(child_z, x, y);
                if let Some(parent) = child.parent() {
                    parents.entry((parent.x, parent.y)).or_default().push(child);
                }
            }
        }

        let dim_dir = output_dir.join(dim_name);
        for ((px, py), children) in parents {
            let parent_coord = TileCoord::new(parent_z, px, py);
            let mut quads: [Option<TileBuffer>; 4] = [None, None, None, None];

            for child in children {
                let quad = child.quadrant_in_parent();
                let quad_idx = match quad {
                    (0, 0) => 0,
                    (1, 0) => 1,
                    (0, 1) => 2,
                    _ => 3,
                };
                let path = tile_path(&dim_dir, dim_name, child, format);
                quads[quad_idx] = load_tile(&path).ok();
            }

            let child_refs = [
                quads[0].as_ref(),
                quads[1].as_ref(),
                quads[2].as_ref(),
                quads[3].as_ref(),
            ];
            let parent_tile = downsample_quadrants(&child_refs);

            if !parent_tile.is_empty() {
                save_tile(&parent_tile, output_dir, dim_name, parent_coord, format)?;
                written += 1;
            }
        }
    }

    Ok(written)
}
