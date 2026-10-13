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
use crate::markers::collect_markers;
use crate::raster::chunk_raster::rasterize_chunk_y_range;
use crate::raster::colormap::ColorMap;
use crate::tiler::block_index::BlockIndex;
use crate::tiler::compositor::TileBuffer;
use crate::tiler::coordinates::TileCoord;
use crate::tiler::encoder::{save_block_index, save_tile_layered, TileFormat};
use crate::tiler::pyramid::downsample_quadrants;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
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
    /// When set, render only the blocks inside this Y slice into
    /// `output/<dim>/<layer>/…` instead of the full surface map.
    pub layer: Option<RenderLayer>,
}

#[derive(Clone, Debug)]
pub struct RenderLayer {
    pub id: String,
    pub label: String,
    pub y_min: i16,
    pub y_max: i16,
}

impl RenderLayer {
    pub fn from_range(y_min: i16, y_max: i16) -> Self {
        Self {
            id: format!("y{}-{}", y_min, y_max),
            label: format!("Y {}..{}", y_min, y_max),
            y_min,
            y_max,
        }
    }
}

pub struct RenderStats {
    pub total_chunks: usize,
    pub base_tiles: usize,
    pub total_tiles: usize,
    pub elapsed: Duration,
}

pub fn render_world_map(options: RenderOptions) -> Result<RenderStats> {
    let db = WorldDb::open(&options.world_path)?;
    render_world_map_with_db(&db, options)
}

pub fn render_world_map_with_db(db: &WorldDb, options: RenderOptions) -> Result<RenderStats> {
    let start_time = Instant::now();

    if options.threads > 0 {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(options.threads)
            .build_global();
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
    let meta_path = match &options.layer {
        Some(layer) => {
            let layer_dir = dim_dir.join(&layer.id);
            let _ = std::fs::remove_dir_all(&layer_dir);
            let _ = std::fs::create_dir_all(&layer_dir);
            layer_dir.join("metadata.json")
        }
        None => {
            let _ = std::fs::remove_dir_all(&dim_dir);
            let _ = std::fs::create_dir_all(&dim_dir);
            dim_dir.join("metadata.json")
        }
    };

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

    tile_groups
        .par_iter()
        .try_for_each(|(&(tx, ty), chunk_list)| -> Result<()> {
            let mut tile_buf = TileBuffer::new();
            let mut block_index = BlockIndex::new();
            let tile_coord = TileCoord::new(max_zoom, tx, ty);

            for chunk_pos in chunk_list {
                if let Ok(chunk_data) = db.get_chunk(*chunk_pos, options.dimension) {
                    if !chunk_data.subchunks.is_empty() {
                        let (ox, oy) = TileCoord::chunk_offset_in_tile(*chunk_pos);
                        let y_range = options.layer.as_ref().map(|l| (l.y_min, l.y_max));
                        let raster = rasterize_chunk_y_range(
                            &chunk_data,
                            &colormap,
                            options.enable_hillshading,
                            y_range,
                        );
                        tile_buf.blit_chunk(ox, oy, &raster.pixels);
                        block_index.blit_chunk(ox, oy, &raster);
                    }
                }
            }

            if !tile_buf.is_empty() {
                save_tile_layered(
                    &tile_buf,
                    &options.output_dir,
                    dim_name,
                    options.layer.as_ref().map(|l| l.id.as_str()),
                    tile_coord,
                    options.format,
                )?;

                save_block_index(
                    &block_index,
                    &options.output_dir,
                    dim_name,
                    options.layer.as_ref().map(|l| l.id.as_str()),
                    tile_coord,
                )?;

                total_tiles_count.fetch_add(1, Ordering::Relaxed);
                let mut map = base_tiles_map
                    .lock()
                    .map_err(|_| AmberError::Io(std::io::Error::other("lock poisoned")))?;
                map.insert(tile_coord, tile_buf);
            }

            Ok(())
        })?;

    let base_tiles_count = total_tiles_count.load(Ordering::Relaxed);
    let mut current_layer = base_tiles_map.into_inner().unwrap_or_default();

    let (min_tx, max_tx, min_ty, max_ty) = if !current_layer.is_empty() {
        (
            current_layer.keys().map(|c| c.x).min().unwrap_or(0),
            current_layer.keys().map(|c| c.x).max().unwrap_or(0),
            current_layer.keys().map(|c| c.y).min().unwrap_or(0),
            current_layer.keys().map(|c| c.y).max().unwrap_or(0),
        )
    } else {
        (0, 0, 0, 0)
    };

    let center_block_x = ((min_tx as i64 + max_tx as i64) * 128 + 128) as i32;
    let center_block_z = ((min_ty as i64 + max_ty as i64) * 128 + 128) as i32;

    let min_block_x = min_tx * 256;
    let min_block_z = min_ty * 256;
    let max_block_x = (max_tx + 1) * 256;
    let max_block_z = (max_ty + 1) * 256;

    let (layer_id, layer_label, y_min, y_max) = match &options.layer {
        Some(l) => (
            l.id.as_str(),
            l.label.as_str(),
            l.y_min as i32,
            l.y_max as i32,
        ),
        None => (
            "surface",
            "Surface",
            surface_y_min(options.dimension),
            surface_y_max(options.dimension),
        ),
    };

    let meta_json = format!(
        r#"{{"center_x":{},"center_z":{},"min_block_x":{},"min_block_z":{},"max_block_x":{},"max_block_z":{},"max_zoom":{},"format":"{}","layer_id":"{}","layer_label":"{}","y_min":{},"y_max":{}}}"#,
        center_block_x,
        center_block_z,
        min_block_x,
        min_block_z,
        max_block_x,
        max_block_z,
        max_zoom,
        options.format.extension(),
        layer_id,
        layer_label,
        y_min,
        y_max
    );
    let _ = std::fs::write(&meta_path, &meta_json);
    if options.layer.is_none() {
        let _ = std::fs::write(options.output_dir.join("metadata.json"), &meta_json);
    }

    let markers = collect_markers(db, options.dimension);
    if !markers.is_empty() {
        let markers_path = match &options.layer {
            Some(layer) => dim_dir.join(&layer.id).join("markers.json"),
            None => dim_dir.join("markers.json"),
        };
        let _ = std::fs::write(markers_path, markers.to_json());
    }

    if max_zoom > 0 {
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
                        save_tile_layered(
                            &parent_tile,
                            &options.output_dir,
                            dim_name,
                            options.layer.as_ref().map(|l| l.id.as_str()),
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

/// Nominal renderable Y extent per dimension, used for surface-layer labels.
fn surface_y_min(dimension: Dimension) -> i32 {
    match dimension {
        Dimension::Overworld => -64,
        Dimension::Nether | Dimension::TheEnd => 0,
    }
}

fn surface_y_max(dimension: Dimension) -> i32 {
    match dimension {
        Dimension::Overworld => 319,
        Dimension::Nether | Dimension::TheEnd => 255,
    }
}
