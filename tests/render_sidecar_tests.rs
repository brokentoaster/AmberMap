/*
 *     _             _                __  __
 *    / \   _ __ ___ | |__   ___ _ __ |  \/  | __ _ _ __
 *   / _ \ | '_ \| | | | | | | (_| | |_) |  __/ |   | |  | | (_| | |_) |
 *  / ___ \| | | | | | | | | | (_| | |_) |  __/ |   | |  | | (_| | |_) |
 * /_/   \_\_| |_| |_| |_|\__,_| .__/
 *                                                 |_|
 *
 * AmberMap - High-Performance Bedrock World Map Renderer
 * https://github.com/Amber-PM/AmberMap
 *
 * Copyright (c) 2026 Amber-PM
 * Licensed under Apache-2.0 or MIT
 */

//! End-to-end render test: builds a synthetic LevelDB-style world
//! directory with one overworld chunk of known blocks, runs the full
//! tile pipeline, and asserts that block identity sidecars
//! (`blocks.bin`) are emitted beside the base tiles with correct
//! contents.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ambermap::leveldb::keys::{DbKey, Dimension, KeyTag};
use ambermap::leveldb::WorldDb;
use ambermap::tiler::block_index::BlockIndex;
use ambermap::tiler::pipeline::{render_world_map_with_db, RenderOptions};
use ambermap::tiler::TileFormat;

fn nbt_string(s: &str) -> Vec<u8> {
    let mut out = (s.len() as i16).to_le_bytes().to_vec();
    out.extend_from_slice(s.as_bytes());
    out
}

/// Encodes a persistent NBT palette entry: root compound with a string
/// `name` field, as written by `NbtReader::read_root_compound`.
fn palette_entry(name: &str) -> Vec<u8> {
    let mut out = vec![0x0a];
    out.extend(nbt_string(""));
    out.extend([0x08]);
    out.extend(nbt_string("name"));
    out.extend(nbt_string(name));
    out.push(0x00);
    out
}

/// Builds a subchunk payload (storage version 8): one 1-bit-per-block
/// packed layer over a 2-entry palette [air, solid].
fn subchunk_payload(solid_name: &str) -> Vec<u8> {
    let mut out = vec![8u8, 1u8, 2u8]; // version 8, 1 storage layer, header: 1 bit/block
                                       // 1 bit per block: 128 words of 32 blocks; palette index 1 = solid
    for _ in 0..128 {
        out.extend_from_slice(&u32::MAX.to_le_bytes());
    }
    out.extend_from_slice(&2i32.to_le_bytes());
    out.extend(palette_entry("minecraft:air"));
    out.extend(palette_entry(solid_name));
    out
}

fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        if v < 0x80 {
            out.push(v as u8);
            return;
        }
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
}

fn build_block(mut entries_block: Vec<u8>) -> Vec<u8> {
    // content = entries + restart array (1 restart at 0) + restart
    // count. The compression-type byte is written after the content
    // when the block is placed, and `block_size` covers content only
    // (matching leveldb's BlockHandle semantics).
    entries_block.extend(0u32.to_le_bytes());
    entries_block.extend(1u32.to_le_bytes());
    entries_block
}

fn block_size(block: &[u8]) -> u64 {
    block.len() as u64
}

fn write_sstable(path: &Path, entries: &HashMap<Vec<u8>, Vec<u8>>) {
    let table_magic: [u8; 8] = [0x57, 0xfb, 0x80, 0x8b, 0x24, 0x75, 0x47, 0xdb];

    let mut data_block = Vec::new();
    let mut sorted: Vec<(&Vec<u8>, &Vec<u8>)> = entries.iter().collect();
    sorted.sort();
    for (k, v) in sorted {
        // leveldb block entry order: shared, unshared, val_len, key, value.
        // Internal keys carry an 8-byte (sequence,type) trailer that
        // `read_sstable` strips; append zeros so the user key survives.
        write_varint(&mut data_block, 0);
        write_varint(&mut data_block, (k.len() + 8) as u64);
        write_varint(&mut data_block, v.len() as u64);
        data_block.extend_from_slice(k);
        data_block.extend_from_slice(&[0u8; 8]);
        data_block.extend_from_slice(v);
    }
    let data_block = build_block(data_block);

    let data_offset: u64 = 0;
    let index_block_entry = {
        let mut e = Vec::new();
        let handle = {
            let mut h = Vec::new();
            write_varint(&mut h, data_offset);
            write_varint(&mut h, block_size(&data_block));
            h
        };
        // leveldb entry order: shared, unshared, val_len, key, value
        write_varint(&mut e, 0);
        write_varint(&mut e, 3);
        write_varint(&mut e, handle.len() as u64);
        e.extend_from_slice(&[0xff, 0xff, 0xff]); // separator key (ignored)
        e.extend_from_slice(&handle);
        e
    };
    let index_block = build_block(index_block_entry);

    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&data_block);
    out.push(0u8); // compression type: none

    let index_offset = out.len() as u64;
    let index_size = block_size(&index_block);
    out.extend_from_slice(&index_block);
    out.push(0u8);

    // empty metaindex block
    let meta_block = build_block(Vec::new());
    out.extend_from_slice(&meta_block);
    out.push(0u8);

    // footer (48 bytes): metaindex handle, index handle, padding, magic
    let mut footer = Vec::new();
    write_varint(&mut footer, index_offset + index_size + 1); // metaindex offset
    write_varint(&mut footer, block_size(&meta_block));
    write_varint(&mut footer, index_offset);
    write_varint(&mut footer, index_size);
    while footer.len() < 40 {
        footer.push(0);
    }
    footer.truncate(40);
    out.extend_from_slice(&footer);
    out.extend_from_slice(&table_magic);

    std::fs::write(path, out).unwrap();
}

#[test]
fn render_emits_block_sidecars_for_base_tiles() {
    let world = std::env::temp_dir().join(format!(
        "ambermap_block_sidecar_world_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&world);
    std::fs::create_dir_all(world.join("db")).unwrap();

    let mut entries: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    let chunk = ambermap::core::coordinates::ChunkPos::new(0, 0);

    let version_key = DbKey {
        dimension: Dimension::Overworld,
        chunk_pos: chunk,
        tag: KeyTag::Version,
        subchunk_y: None,
    };
    entries.insert(version_key.to_bytes(), vec![8u8]);

    let sub_key = DbKey {
        dimension: Dimension::Overworld,
        chunk_pos: chunk,
        tag: KeyTag::SubChunkPrefix,
        subchunk_y: Some(0),
    };
    entries.insert(sub_key.to_bytes(), subchunk_payload("minecraft:gold_block"));

    write_sstable(&world.join("db").join("test.ldb"), &entries);

    let out = std::env::temp_dir().join(format!(
        "ambermap_block_sidecar_out_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&out);

    {
        use ambermap::leveldb::sstable::read_sstable;
        let mut map = std::collections::HashMap::new();
        read_sstable(world.join("db").join("test.ldb"), &mut map).expect("sstable parse");
        eprintln!(
            "sstable keys: {:?}",
            map.keys().map(|k| k.len()).collect::<Vec<_>>()
        );
        assert_eq!(map.len(), 2, "sstable should expose both entries");
    }

    let db = WorldDb::open(&world).expect("synthetic world opens");
    let chunks = db.scan_chunks(Dimension::Overworld);
    if chunks.is_empty() {
        panic!(
            "no chunks scanned; world at {:?} contains {} entries",
            world,
            entries.len()
        );
    }
    assert_eq!(chunks.len(), 1);
    let options = RenderOptions {
        world_path: world.clone(),
        output_dir: out.clone(),
        dimension: Dimension::Overworld,
        zoom_levels: 3,
        format: TileFormat::Png,
        threads: 0,
        enable_hillshading: false,
        layer: None,
    };

    let stats = render_world_map_with_db(&db, options).expect("render succeeds");
    assert!(stats.total_tiles > 0, "expected tiles to be generated");

    let sidecars: Vec<PathBuf> = walk_files(&out)
        .into_iter()
        .filter(|p| p.to_string_lossy().ends_with(".blocks.bin"))
        .collect();
    assert!(
        !sidecars.is_empty(),
        "expected at least one blocks.bin sidecar, found none in {:?}",
        out
    );

    // exactly one sidecar per base tile: no sidecars below the base zoom
    let non_base: Vec<&PathBuf> = sidecars
        .iter()
        .filter(|p| {
            p.parent()
                .and_then(|d| d.parent())
                .and_then(|d| d.file_name())
                .map(|d| d != "2")
                .unwrap_or(false)
        })
        .collect();
    assert!(non_base.is_empty(), "sidecars emitted below base zoom");

    for path in &sidecars {
        let bytes = std::fs::read(path).unwrap();
        let index = BlockIndex::from_bytes(&bytes).expect("valid sidecar");
        let found = index
            .grid
            .iter()
            .any(|&i| i != ambermap::raster::traverser::NO_BLOCK);
        assert!(found, "sidecar {:?} should contain block entries", path);
        assert!(
            index.names.iter().any(|n| n == "minecraft:gold_block"),
            "sidecar names should contain the palette block"
        );
    }

    let _ = std::fs::remove_dir_all(&world);
    let _ = std::fs::remove_dir_all(&out);
}

fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk_files(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}
