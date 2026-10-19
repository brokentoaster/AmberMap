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

use crate::leveldb::decompressor::decompress_payload;
use crate::leveldb::keys::Dimension;
use crate::leveldb::reader::WorldDb;
use crate::nbt::reader::NbtReader;
use crate::nbt::tag::NbtTag;
use std::fmt::Write as _;

#[derive(Clone, Debug)]
pub struct SignMarker {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct PlayerMarker {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub dimension: Dimension,
}

#[derive(Clone, Debug, Default)]
pub struct Markers {
    pub signs: Vec<SignMarker>,
    pub players: Vec<PlayerMarker>,
}

impl Markers {
    pub fn is_empty(&self) -> bool {
        self.signs.is_empty() && self.players.is_empty()
    }

    pub fn to_json(&self) -> String {
        let mut out = String::from("{\"signs\":[");
        for (i, s) in self.signs.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let lines: Vec<String> = s.lines.iter().map(|l| json_escape(l)).collect();
            let _ = write!(
                out,
                "{{\"x\":{},\"y\":{},\"z\":{},\"lines\":[{}]}}",
                s.x,
                s.y,
                s.z,
                lines
                    .iter()
                    .map(|l| format!("\"{}\"", l))
                    .collect::<Vec<String>>()
                    .join(",")
            );
        }
        out.push_str("],\"players\":[");
        for (i, p) in self.players.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let dim = match p.dimension {
                Dimension::Overworld => "overworld",
                Dimension::Nether => "nether",
                Dimension::TheEnd => "the_end",
            };
            let _ = write!(
                out,
                "{{\"name\":\"{}\",\"x\":{:.2},\"y\":{:.2},\"z\":{:.2},\"dimension\":\"{}\"}}",
                json_escape(&p.name),
                p.x,
                p.y,
                p.z,
                dim
            );
        }
        out.push_str("]}");
        out
    }
}

impl SignMarker {
    pub fn from_nbt(tag: &NbtTag) -> Option<Self> {
        parse_sign(tag)
    }
}

impl PlayerMarker {
    pub fn from_nbt(tag: &NbtTag) -> Option<Self> {
        parse_player(tag)
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn parse_nbt_root(payload: &[u8]) -> Option<NbtTag> {
    let mut reader = NbtReader::new(payload);
    reader.read_root_compound().ok().map(|(_, tag)| tag)
}

fn nbt_pos(tag: &NbtTag) -> Option<(i32, i32, i32)> {
    Some((tag.get_i32("x")?, tag.get_i32("y")?, tag.get_i32("z")?))
}

/// Extracts the four sign text lines. Modern Bedrock stores text in
/// `FrontText`/`BackText` compounds; older versions in a top-level `Text`
/// string with newline-separated lines. Returns the first side that has
/// visible text, falling back to the legacy field.
fn sign_lines(tag: &NbtTag) -> Option<Vec<String>> {
    let raw = tag
        .get_compound("FrontText")
        .and_then(|f| f.get("Text"))
        .and_then(|t| match t {
            NbtTag::String(s) => Some(s.as_str()),
            _ => None,
        })
        .or_else(|| tag.get_string("Text"))
        .or_else(|| {
            tag.get_compound("BackText")
                .and_then(|f| f.get("Text"))
                .and_then(|t| match t {
                    NbtTag::String(s) => Some(s.as_str()),
                    _ => None,
                })
        })?;

    let cleaned: String = raw
        .replace("\\u00a7", "\u{a7}")
        .chars()
        .filter(|c| *c != '\u{a7}')
        .collect();
    let mut lines: Vec<String> = cleaned.split('\n').map(|l| l.trim().to_string()).collect();
    while lines.len() < 4 {
        lines.push(String::new());
    }
    let has_content = lines.iter().any(|l| !l.is_empty());
    if has_content {
        Some(lines)
    } else {
        None
    }
}

/// Resolves a player's dimension. Older saves store `Dimension` as an
/// Int (0 = overworld, 1 = nether, 2 = the end); saves since custom
/// dimensions (~1.21.x) store a namespaced String id such as
/// "minecraft:nether". Both forms are accepted; unknown values fall
/// back to the overworld.
fn parse_dimension(tag: &NbtTag) -> Dimension {
    if let Some(id) = tag.get_string("Dimension") {
        let local_id = id.rsplit(':').next().unwrap_or(id);
        return match local_id {
            "nether" => Dimension::Nether,
            "the_end" => Dimension::TheEnd,
            _ => Dimension::Overworld,
        };
    }
    match tag.get_i32("Dimension") {
        Some(1) => Dimension::Nether,
        Some(2) => Dimension::TheEnd,
        _ => Dimension::Overworld,
    }
}

fn parse_sign(tag: &NbtTag) -> Option<SignMarker> {
    let id = tag.get_string("id")?;
    let is_sign = id.ends_with("sign")
        || id.ends_with("Sign")
        || id.ends_with("HangingSign")
        || id.ends_with("hanging_sign");
    if !is_sign {
        return None;
    }
    let (x, y, z) = nbt_pos(tag)?;
    let lines = sign_lines(tag)?;
    Some(SignMarker { x, y, z, lines })
}

fn parse_player(tag: &NbtTag) -> Option<PlayerMarker> {
    let pos = match tag {
        NbtTag::Compound(map) => match map.get("Pos") {
            Some(NbtTag::List(items)) if items.len() == 3 => {
                let f = |t: &NbtTag| match t {
                    NbtTag::Float(v) => *v as f64,
                    NbtTag::Double(v) => *v,
                    _ => 0.0,
                };
                (f(&items[0]), f(&items[1]), f(&items[2]))
            }
            _ => return None,
        },
        _ => return None,
    };
    let dimension = parse_dimension(tag);
    Some(PlayerMarker {
        name: "Player".to_string(),
        x: pos.0,
        y: pos.1,
        z: pos.2,
        dimension,
    })
}

/// Parses a raw block-entity payload into a sign marker if it is a sign.
pub fn parse_sign_payload(payload: &[u8]) -> Option<SignMarker> {
    parse_nbt_root(payload).and_then(|tag| parse_sign(&tag))
}

pub fn collect_markers(db: &WorldDb, dimension: Dimension) -> Markers {
    let mut markers = Markers::default();

    for payload in db.block_entity_payloads(dimension) {
        let payload = decompress_payload(&payload).unwrap_or(payload);
        let parsed = parse_nbt_root(&payload).and_then(|tag| parse_sign(&tag));
        if let Some(sign) = parsed {
            markers.signs.push(sign);
        }
    }

    for payload in db.player_payloads() {
        let parsed = parse_nbt_root(&payload).and_then(|tag| parse_player(&tag));
        if let Some(player) = parsed {
            markers.players.push(player);
        }
    }

    markers
}
