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

use ambermap::leveldb::keys::Dimension;
use ambermap::leveldb::reader::WorldDb;
use ambermap::markers::{collect_markers, Markers, SignMarker};
use ambermap::nbt::reader::NbtReader;
use ambermap::nbt::tag::NbtTag;

fn nbt_string(s: &str) -> Vec<u8> {
    let mut out = (s.len() as i16).to_le_bytes().to_vec();
    out.extend_from_slice(s.as_bytes());
    out
}

fn nbt_entry(tag_type: u8, name: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![tag_type];
    out.extend(nbt_string(name));
    out.extend_from_slice(payload);
    out
}

fn nbt_compound_open(name: &str) -> Vec<u8> {
    let mut out = vec![0x0a];
    out.extend(nbt_string(name));
    out
}

/// Builds a little-endian NBT payload: root compound with the given fields.
/// field type: 3 = int(i32 LE), 8 = string
fn build_nbt(fields: &[(u8, &str, Vec<u8>)]) -> Vec<u8> {
    let mut out = nbt_compound_open("");
    for (t, name, payload) in fields {
        out.extend(nbt_entry(*t, name, payload));
    }
    out.push(0x00);
    out
}

fn parse(payload: &[u8]) -> Option<NbtTag> {
    let mut reader = NbtReader::new(payload);
    reader.read_root_compound().ok().map(|(_, tag)| tag)
}

fn sign_nbt(id: &str, x: i32, y: i32, z: i32, front_text: &str) -> Vec<u8> {
    let mut front = Vec::new();
    front.extend(nbt_entry(0x08, "Text", &nbt_string(front_text)));
    front.push(0x00);

    let mut out = nbt_compound_open("");
    out.extend(nbt_entry(0x08, "id", &nbt_string(id)));
    out.extend(nbt_entry(0x03, "x", &x.to_le_bytes()));
    out.extend(nbt_entry(0x03, "y", &y.to_le_bytes()));
    out.extend(nbt_entry(0x03, "z", &z.to_le_bytes()));
    out.extend(nbt_entry(0x0a, "FrontText", &front));
    out.push(0x00);
    out
}

fn player_nbt(dim: i32, pos: [f32; 3]) -> Vec<u8> {
    let mut list = vec![0x05];
    list.extend_from_slice(&3i32.to_le_bytes());
    for v in pos {
        list.extend_from_slice(&v.to_le_bytes());
    }

    let mut out = nbt_compound_open("");
    out.extend(nbt_entry(0x03, "Dimension", &dim.to_le_bytes()));
    out.extend(nbt_entry(0x09, "Pos", &list));
    out.push(0x00);
    out
}

fn player_nbt_string_dimension(dim: &str, pos: [f32; 3]) -> Vec<u8> {
    let mut list = vec![0x05];
    list.extend_from_slice(&3i32.to_le_bytes());
    for v in pos {
        list.extend_from_slice(&v.to_le_bytes());
    }

    let mut out = nbt_compound_open("");
    out.extend(nbt_entry(0x08, "Dimension", &nbt_string(dim)));
    out.extend(nbt_entry(0x09, "Pos", &list));
    out.push(0x00);
    out
}

#[test]
fn parses_modern_sign_with_front_text() {
    let payload = sign_nbt("minecraft:sign", 10, 64, -20, "Hello\nWorld\n\n");
    let tag = parse(&payload).expect("valid nbt");
    let marker = ambermap::markers::SignMarker::from_nbt(&tag).expect("sign parsed");
    assert_eq!((marker.x, marker.y, marker.z), (10, 64, -20));
    assert_eq!(marker.lines[0], "Hello");
    assert_eq!(marker.lines[1], "World");
    assert_eq!(marker.lines.len(), 4);
}

#[test]
fn parses_legacy_sign_text_field() {
    let legacy = "A\nB\n\n";
    let payload = build_nbt(&[
        (0x08, "id", nbt_string("minecraft:sign")),
        (0x03, "x", (-1i32).to_le_bytes().to_vec()),
        (0x03, "y", (70i32).to_le_bytes().to_vec()),
        (0x03, "z", (5i32).to_le_bytes().to_vec()),
        (0x08, "Text", nbt_string(legacy)),
    ]);
    let tag = parse(&payload).unwrap();
    let marker = ambermap::markers::SignMarker::from_nbt(&tag).expect("legacy sign parsed");
    assert_eq!(marker.lines[0], "A");
    assert_eq!(marker.lines[1], "B");
}

#[test]
fn rejects_non_sign_block_entities() {
    let payload = build_nbt(&[
        (0x08, "id", nbt_string("minecraft:chest")),
        (0x03, "x", (0i32).to_le_bytes().to_vec()),
        (0x03, "y", (0i32).to_le_bytes().to_vec()),
        (0x03, "z", (0i32).to_le_bytes().to_vec()),
    ]);
    let tag = parse(&payload).unwrap();
    assert!(ambermap::markers::SignMarker::from_nbt(&tag).is_none());
}

#[test]
fn skips_blank_signs() {
    let payload = sign_nbt("minecraft:sign", 0, 0, 0, "   \n\n\n\n");
    let tag = parse(&payload).unwrap();
    assert!(ambermap::markers::SignMarker::from_nbt(&tag).is_none());
}

#[test]
fn parses_player_position_and_dimension() {
    let payload = player_nbt(1, [12.5f32, -30.0f32, 44.25f32]);
    let tag = parse(&payload).unwrap();
    let player = ambermap::markers::PlayerMarker::from_nbt(&tag).expect("player parsed");
    assert_eq!(player.dimension, Dimension::Nether);
    assert!((player.x - 12.5).abs() < 1e-4);
    assert!((player.z - 44.25).abs() < 1e-4);
    assert!(!player.name.is_empty());
}

#[test]
fn parses_string_dimension_ids_from_modern_saves() {
    let nether = player_nbt_string_dimension("minecraft:nether", [10.0f32, 64.0f32, 20.0f32]);
    let tag = parse(&nether).unwrap();
    let player = ambermap::markers::PlayerMarker::from_nbt(&tag).expect("player parsed");
    assert_eq!(player.dimension, Dimension::Nether);

    let overworld = player_nbt_string_dimension("minecraft:overworld", [0.0f32, 0.0f32, 0.0f32]);
    let tag = parse(&overworld).unwrap();
    let player = ambermap::markers::PlayerMarker::from_nbt(&tag).expect("player parsed");
    assert_eq!(player.dimension, Dimension::Overworld);

    let the_end = player_nbt_string_dimension("minecraft:the_end", [0.0f32, 0.0f32, 0.0f32]);
    let tag = parse(&the_end).unwrap();
    let player = ambermap::markers::PlayerMarker::from_nbt(&tag).expect("player parsed");
    assert_eq!(player.dimension, Dimension::TheEnd);
}

#[test]
fn falls_back_to_overworld_for_unknown_dimension_ids() {
    let payload = player_nbt_string_dimension("custom:foo", [0.0f32, 0.0f32, 0.0f32]);
    let tag = parse(&payload).unwrap();
    let player = ambermap::markers::PlayerMarker::from_nbt(&tag).expect("player parsed");
    assert_eq!(player.dimension, Dimension::Overworld);
}

#[test]
fn rejects_player_without_position() {
    let payload = build_nbt(&[(0x03, "Dimension", 0i32.to_le_bytes().to_vec())]);
    let tag = parse(&payload).unwrap();
    assert!(ambermap::markers::PlayerMarker::from_nbt(&tag).is_none());
}

#[test]
fn handles_malformed_nbt_silently() {
    let truncated = &sign_nbt("minecraft:sign", 1, 2, 3, "x")[..10];
    let parsed = parse(truncated);
    assert!(
        parsed.is_none() || ambermap::markers::SignMarker::from_nbt(&parsed.unwrap()).is_none()
    );
}

#[test]
fn markers_json_shape() {
    let markers = Markers {
        signs: vec![SignMarker {
            x: 1,
            y: 2,
            z: 3,
            lines: vec![
                "he said \"hi\"".into(),
                String::new(),
                String::new(),
                String::new(),
            ],
        }],
        players: vec![],
    };
    let json = markers.to_json();
    assert!(json.starts_with("{\"signs\":["));
    assert!(json.contains("he said \\\"hi\\\""));
    assert!(json.ends_with("]}"));
}

#[test]
fn collect_markers_on_empty_db() {
    let tmp = std::env::temp_dir().join("ambermap_markers_empty_test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let db = WorldDb::open(&tmp).unwrap();
    let markers = collect_markers(&db, Dimension::Overworld, &std::collections::HashMap::new());
    assert!(markers.is_empty());
    assert_eq!(markers.to_json(), "{\"signs\":[],\"players\":[]}");
}

#[test]
fn resolves_player_names_from_allowlist() {
    let tmp = std::env::temp_dir().join("ambermap_allowlist_test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(
        tmp.join("allowlist.json"),
        r#"[{"name":"Steve","xuid":"2535410123456789"},{"name":"Alex","xuid":"2535419999999999"}]"#,
    )
    .unwrap();

    let names = ambermap::markers::load_player_names(&tmp);
    assert_eq!(
        names.get("2535410123456789").map(|s| s.as_str()),
        Some("Steve")
    );

    let lookup = names;
    assert_eq!(
        ambermap::markers::player_name_for_key("player_2535410123456789", &lookup),
        "Steve"
    );
    assert_eq!(
        ambermap::markers::player_name_for_key("player_0000000000000000", &lookup),
        "0000000000000000"
    );
    assert_eq!(
        ambermap::markers::player_name_for_key("~local_player", &lookup),
        "local_player"
    );
}

#[test]
fn missing_allowlist_yields_empty_names() {
    let tmp = std::env::temp_dir().join("ambermap_allowlist_missing_test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    assert!(ambermap::markers::load_player_names(&tmp).is_empty());
}

#[test]
fn parses_sign_payload_from_raw_bytes() {
    let payload = sign_nbt("minecraft:sign", 100, 65, -300, "Spawn is\nthis way");
    let marker = ambermap::markers::parse_sign_payload(&payload).expect("raw sign parsed");
    assert_eq!((marker.x, marker.y, marker.z), (100, 65, -300));
    assert_eq!(marker.lines[0], "Spawn is");
    assert_eq!(marker.lines[1], "this way");

    let not_sign = build_nbt(&[(0x08, "id", nbt_string("minecraft:furnace"))]);
    assert!(ambermap::markers::parse_sign_payload(&not_sign).is_none());
    assert!(ambermap::markers::parse_sign_payload(&[0xff, 0x00]).is_none());
}
