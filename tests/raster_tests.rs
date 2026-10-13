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

use ambermap::core::coordinates::{block_subchunk_index, ChunkPos};
use ambermap::leveldb::keys::Dimension;
use ambermap::leveldb::reader::ChunkData;
use ambermap::raster::chunk_raster::rasterize_chunk;
use ambermap::raster::colormap::ColorMap;
use ambermap::raster::hillshade::compute_light_factor;
use ambermap::raster::traverser::traverse_chunk;
use ambermap::subchunk::palette::{BlockState, Palette};
use ambermap::subchunk::storage::{SubChunk, SubChunkStorage};

fn create_mock_subchunk(blocks_at_y: &[(usize, &str)]) -> SubChunk {
    let mut palette_entries = vec![BlockState::new("minecraft:air")];
    let mut name_to_idx = std::collections::HashMap::new();
    name_to_idx.insert("minecraft:air", 0u16);

    for &(_, name) in blocks_at_y {
        if !name_to_idx.contains_key(name) {
            let idx = palette_entries.len() as u16;
            palette_entries.push(BlockState::new(name));
            name_to_idx.insert(name, idx);
        }
    }

    let mut blocks = Box::new([0u16; 4096]);
    for x in 0..16 {
        for z in 0..16 {
            for &(y, name) in blocks_at_y {
                let idx = block_subchunk_index(x, y, z);
                let p_idx = name_to_idx[name];
                blocks[idx] = p_idx;
            }
        }
    }

    let layer = SubChunkStorage::new(blocks, Palette::new(palette_entries));
    SubChunk::new(8, vec![layer], Some(0))
}

#[test]
fn test_alpha_compositing_water_over_stone() {
    let colormap = ColorMap::new();

    let subchunk = create_mock_subchunk(&[(0, "minecraft:stone"), (1, "minecraft:water")]);

    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, subchunk)],
    };

    let result = traverse_chunk(&chunk, &colormap);

    let r = result.pixels[0];
    let g = result.pixels[1];
    let b = result.pixels[2];
    let a = result.pixels[3];

    assert_eq!(a, 255);
    assert_eq!(result.heightmap[0], 1);

    // water [44, 98, 204, 180] over stone [125, 125, 125, 255]
    assert_eq!(r, 68);
    assert_eq!(g, 106);
    assert_eq!(b, 181);
}

#[test]
fn test_ray_termination_opaque() {
    let colormap = ColorMap::new();

    let subchunk = create_mock_subchunk(&[
        (0, "minecraft:bedrock"),
        (9, "minecraft:dirt"),
        (10, "minecraft:grass_block"),
    ]);

    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, subchunk)],
    };

    let result = traverse_chunk(&chunk, &colormap);

    assert_eq!(result.heightmap[0], 10);
    assert_eq!(result.pixels[0], 92);
    assert_eq!(result.pixels[1], 157);
    assert_eq!(result.pixels[2], 59);
    assert_eq!(result.pixels[3], 255);
}

#[test]
fn test_hillshade_slopes() {
    let flat = compute_light_factor(64, 64, 64);
    assert!((flat - 1.0).abs() < 1e-5);

    let uphill = compute_light_factor(66, 64, 64);
    assert!(uphill > 1.0);
    assert!(uphill <= 1.30);

    let downhill = compute_light_factor(60, 64, 64);
    assert!(downhill < 1.0);
    assert!(downhill >= 0.60);

    let steep_up = compute_light_factor(200, 50, 50);
    assert_eq!(steep_up, 1.30);

    let steep_down = compute_light_factor(10, 100, 100);
    assert_eq!(steep_down, 0.60);
}

#[test]
fn test_nether_roof_bedrock_skipped() {
    let colormap = ColorMap::new();

    let roof = create_mock_subchunk(&[(0, "minecraft:bedrock")]);
    let terrain = create_mock_subchunk(&[(0, "minecraft:netherrack")]);

    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Nether,
        version: Some(42),
        subchunks: vec![(3, terrain), (12, roof)],
    };

    let result = traverse_chunk(&chunk, &colormap);

    // netherrack [111, 40, 40, 255] at world y = 48, roof ignored
    assert_eq!(result.heightmap[0], 48);
    assert_eq!(result.pixels[0], 111);
    assert_eq!(result.pixels[1], 40);
    assert_eq!(result.pixels[2], 40);
    assert_eq!(result.pixels[3], 255);
}

#[test]
fn test_overworld_bedrock_still_renders() {
    let colormap = ColorMap::new();

    let only_bedrock = create_mock_subchunk(&[(0, "minecraft:bedrock")]);

    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, only_bedrock)],
    };

    let result = traverse_chunk(&chunk, &colormap);

    assert_eq!(result.heightmap[0], 0);
    assert_eq!(result.pixels[3], 255);
}

#[test]
fn test_y_range_filtering_ignores_blocks_above_range() {
    use ambermap::raster::traverser::traverse_chunk_y_range;

    let colormap = ColorMap::new();

    let subchunk = create_mock_subchunk(&[
        (0, "minecraft:stone"),
        (5, "minecraft:diamond_ore"),
        (15, "minecraft:grass_block"),
    ]);

    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, subchunk)],
    };

    let result = traverse_chunk_y_range(&chunk, &colormap, Some((0, 10)));

    // grass at y=15 is outside the range; surface becomes diamond_ore
    // [114, 142, 140] over stone, terminated at y=5
    assert_eq!(result.heightmap[0], 5);
    assert_eq!(result.pixels[3], 255);
    assert_eq!(result.pixels[0], 114);
    assert_eq!(result.pixels[1], 142);
    assert_eq!(result.pixels[2], 140);
}

#[test]
fn test_nether_y_range_slice_skips_roof_and_out_of_range() {
    use ambermap::raster::traverser::traverse_chunk_y_range;

    let colormap = ColorMap::new();

    let roof = create_mock_subchunk(&[(0, "minecraft:bedrock")]);
    let upper = create_mock_subchunk(&[(0, "minecraft:netherrack")]);
    let lower = create_mock_subchunk(&[(0, "minecraft:glowstone")]);

    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Nether,
        version: Some(42),
        subchunks: vec![(2, lower), (7, upper), (12, roof)],
    };

    // slice y 32..47 => only subchunk 2 (glowstone at world y=32)
    let result = traverse_chunk_y_range(&chunk, &colormap, Some((32, 47)));

    assert_eq!(result.heightmap[0], 32);
    // glowstone [189, 147, 94, 255]
    assert_eq!(result.pixels[0], 189);
    assert_eq!(result.pixels[1], 147);
    assert_eq!(result.pixels[2], 94);
    assert_eq!(result.pixels[3], 255);
}

#[test]
fn test_chunk_rasterize_pipeline() {
    let colormap = ColorMap::new();
    let subchunk = create_mock_subchunk(&[(0, "minecraft:grass_block")]);

    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, subchunk)],
    };

    let result = rasterize_chunk(&chunk, &colormap, true);
    assert_eq!(result.pixels.len(), 16 * 16 * 4);
    assert_eq!(result.heightmap.len(), 16 * 16);
    assert_eq!(result.heightmap[0], 0);
}

#[test]
fn test_top_block_names_recorded_per_column() {
    let colormap = ColorMap::new();

    // Every column's top visible block is grass (water above grass would
    // still be water as the first visible block).
    let subchunk = create_mock_subchunk(&[(0, "minecraft:grass_block")]);
    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, subchunk)],
    };

    let result = traverse_chunk(&chunk, &colormap);
    assert_eq!(result.block_names.len(), 1);
    assert_eq!(result.block_names[0], "minecraft:grass_block");
    for col in 0..256 {
        assert_eq!(result.top_block[col], 0);
    }
}

#[test]
fn test_top_block_distinguishes_water_over_stone() {
    let colormap = ColorMap::new();

    let subchunk = create_mock_subchunk(&[(0, "minecraft:stone"), (1, "minecraft:water")]);
    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, subchunk)],
    };

    let result = traverse_chunk(&chunk, &colormap);
    // Water is the first visible block in every column (it is above stone).
    assert_eq!(result.block_names.len(), 1);
    assert_eq!(result.block_names[0], "minecraft:water");
    assert_eq!(result.top_block[0], 0);
}

#[test]
fn test_top_block_empty_column_sentinel() {
    let colormap = ColorMap::new();

    // Only one block at one location; all other columns render empty.
    let subchunk = create_mock_subchunk(&[(0, "minecraft:gold_block")]);
    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, subchunk)],
    };

    let result = traverse_chunk(&chunk, &colormap);
    assert_eq!(result.block_names.len(), 1);
    assert_eq!(result.block_names[0], "minecraft:gold_block");
    for col in 0..256 {
        assert_eq!(result.top_block[col], 0);
    }
}

#[test]
fn test_rasterize_chunk_exposes_top_block_data() {
    let colormap = ColorMap::new();
    let subchunk = create_mock_subchunk(&[(0, "minecraft:dirt")]);

    let chunk = ChunkData {
        chunk_pos: ChunkPos::new(0, 0),
        dimension: Dimension::Overworld,
        version: Some(42),
        subchunks: vec![(0, subchunk)],
    };

    let result = rasterize_chunk(&chunk, &colormap, true);
    assert_eq!(result.block_names.len(), 1);
    assert_eq!(result.block_names[0], "minecraft:dirt");
    assert_eq!(result.top_block[0], 0);
}
