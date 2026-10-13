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

use crate::leveldb::keys::Dimension;
use crate::leveldb::reader::ChunkData;
use crate::raster::colormap::{BlockColor, ColorMap};
use crate::subchunk::storage::SubChunkStorage;

pub struct ColumnScanResult {
    pub pixels: [u8; 16 * 16 * 4],
    pub heightmap: [i16; 16 * 16],
    /// Per column, the interned index into `block_names` of the first
    /// visible block, or `NONE` when the column rendered empty.
    pub top_block: [u16; 16 * 16],
    /// Chunk-local interned top-block names, indexed by `top_block`.
    pub block_names: Vec<String>,
}

/// Sentinel in `top_block` for a column with no visible block.
pub const NO_BLOCK: u16 = u16::MAX;

struct VoxelLut {
    color: BlockColor,
    air: bool,
    bedrock: bool,
}

struct LayerLut {
    entries: Vec<VoxelLut>,
    /// True when no palette entry can ever contribute a pixel for this
    /// dimension, so the whole layer can be skipped in the ray march.
    skippable: bool,
}

fn build_layer_lut(layer: &SubChunkStorage, colormap: &ColorMap, nether: bool) -> LayerLut {
    let entries: Vec<VoxelLut> = (0..layer.palette.len())
        .map(|i| {
            let name = layer
                .palette
                .get(i)
                .map(|b| b.name.as_str())
                .unwrap_or_default();
            VoxelLut {
                color: colormap.get(name),
                air: colormap.is_air(name),
                bedrock: name == "minecraft:bedrock",
            }
        })
        .collect();

    let skippable = entries
        .iter()
        .all(|e| e.air || e.color.rgba[3] == 0 || (nether && e.bedrock));

    LayerLut { entries, skippable }
}

/// Renders a full column scan from the top of the world down.
pub fn traverse_chunk(chunk: &ChunkData, colormap: &ColorMap) -> ColumnScanResult {
    traverse_chunk_y_range(chunk, colormap, None)
}

/// Renders a column scan restricted to an inclusive block-Y range. Blocks
/// outside `[y_min, y_max]` are treated as air; `None` scans the full range.
pub fn traverse_chunk_y_range(
    chunk: &ChunkData,
    colormap: &ColorMap,
    y_range: Option<(i16, i16)>,
) -> ColumnScanResult {
    let mut pixels = [0u8; 16 * 16 * 4];
    let mut heightmap = [0i16; 16 * 16];
    let mut top_block = [NO_BLOCK; 16 * 16];
    let mut block_names: Vec<String> = Vec::new();
    // Resolved after the march so the hot loop stays allocation-free:
    // palette indices recorded here, names interned once per column.
    let mut top_palette: [u16; 16 * 16] = [NO_BLOCK; 16 * 16];
    let mut top_palette_names: Vec<Option<String>> = Vec::new();

    // One lookup-table entry per palette index per layer, built once per
    // chunk instead of resolving block names per voxel.
    let layer_luts: Vec<Vec<LayerLut>> = chunk
        .subchunks
        .iter()
        .map(|(_, subchunk)| {
            subchunk
                .layers
                .iter()
                .map(|layer| build_layer_lut(layer, colormap, chunk.dimension == Dimension::Nether))
                .collect()
        })
        .collect();

    for z in 0..16 {
        for x in 0..16 {
            let col_idx = z * 16 + x;
            let pixel_offset = col_idx * 4;

            let mut acc_r = 0.0f32;
            let mut acc_g = 0.0f32;
            let mut acc_b = 0.0f32;
            let mut acc_a = 0.0f32;
            let mut recorded_height: Option<i16> = None;

            // ray-march top-down through subchunks
            'ray: for (sub_idx, (sub_y, subchunk)) in chunk.subchunks.iter().enumerate().rev() {
                let base_y = (*sub_y as i32) * 16;

                for local_y in (0..16).rev() {
                    let world_y = base_y + (local_y as i32);

                    if let Some((y_min, y_max)) = y_range {
                        if world_y < y_min as i32 || world_y > y_max as i32 {
                            continue;
                        }
                    }

                    // inspect layers top-to-bottom (layer 1 waterlogged before layer 0 base)
                    for (layer_idx, layer) in subchunk.layers.iter().enumerate().rev() {
                        let layer_lut = &layer_luts[sub_idx][layer_idx];
                        if layer_lut.skippable {
                            continue;
                        }
                        let lut = &layer_lut.entries;
                        let block_idx = (x << 8) | (z << 4) | local_y;
                        let palette_idx = layer.blocks[block_idx] as usize;

                        let entry = match lut.get(palette_idx) {
                            Some(e) => e,
                            None => continue,
                        };

                        if entry.air {
                            continue;
                        }

                        // The nether has a solid bedrock roof at the top of the
                        // dimension; skipping bedrock above all terrain keeps
                        // the map from rendering as an opaque roof.
                        if chunk.dimension == Dimension::Nether
                            && recorded_height.is_none()
                            && entry.bedrock
                        {
                            continue;
                        }

                        let color = entry.color;
                        if color.rgba[3] == 0 {
                            continue;
                        }

                        if recorded_height.is_none() {
                            recorded_height = Some(world_y as i16);
                            let name = layer.palette.get(palette_idx).map(|b| b.name.clone());
                            let intern = top_palette_names
                                .iter()
                                .position(|n| n.as_deref() == name.as_deref());
                            top_palette[col_idx] = match intern {
                                Some(i) => i as u16,
                                None => {
                                    top_palette_names.push(name);
                                    (top_palette_names.len() - 1) as u16
                                }
                            };
                        }

                        let src_a = (color.rgba[3] as f32) / 255.0;
                        let weight = src_a * (1.0 - acc_a);

                        acc_r += (color.rgba[0] as f32) * weight;
                        acc_g += (color.rgba[1] as f32) * weight;
                        acc_b += (color.rgba[2] as f32) * weight;
                        acc_a += weight;

                        if acc_a >= 0.98 {
                            acc_a = 1.0;
                            break 'ray;
                        }
                    }
                }
            }

            if acc_a > 0.0 {
                pixels[pixel_offset] = acc_r.round().clamp(0.0, 255.0) as u8;
                pixels[pixel_offset + 1] = acc_g.round().clamp(0.0, 255.0) as u8;
                pixels[pixel_offset + 2] = acc_b.round().clamp(0.0, 255.0) as u8;
                pixels[pixel_offset + 3] = (acc_a * 255.0).round().clamp(0.0, 255.0) as u8;
                heightmap[col_idx] = recorded_height.unwrap_or(0);
                let intern = top_palette[col_idx];
                if (intern as usize) < top_palette_names.len() {
                    let name = top_palette_names[intern as usize]
                        .clone()
                        .unwrap_or_default();
                    let slot = block_names.iter().position(|n| *n == name);
                    top_block[col_idx] = match slot {
                        Some(i) => i as u16,
                        None => {
                            block_names.push(name);
                            (block_names.len() - 1) as u16
                        }
                    };
                }
            }
        }
    }

    ColumnScanResult {
        pixels,
        heightmap,
        top_block,
        block_names,
    }
}
