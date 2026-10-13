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

use crate::core::error::{AmberError, Result};
use crate::tiler::block_index::BlockIndex;
use crate::tiler::compositor::{TileBuffer, TILE_SIZE};
use crate::tiler::coordinates::TileCoord;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileFormat {
    Png,
    Webp,
}

impl std::str::FromStr for TileFormat {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "webp" => Ok(Self::Webp),
            _ => Ok(Self::Png),
        }
    }
}

impl TileFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "webp" => Self::Webp,
            _ => Self::Png,
        }
    }
}

pub fn save_tile(
    buffer: &TileBuffer,
    base_dir: impl AsRef<Path>,
    dimension_name: &str,
    coord: TileCoord,
    format: TileFormat,
) -> Result<PathBuf> {
    save_tile_layered(buffer, base_dir, dimension_name, None, coord, format)
}

pub fn save_tile_layered(
    buffer: &TileBuffer,
    base_dir: impl AsRef<Path>,
    dimension_name: &str,
    layer_id: Option<&str>,
    coord: TileCoord,
    format: TileFormat,
) -> Result<PathBuf> {
    let zoom_str = coord.zoom.to_string();
    let x_str = coord.x.to_string();
    let file_name = format!("{}.{}", coord.y, format.extension());

    let dim_dir = base_dir.as_ref().join(dimension_name);
    let target_dir = match layer_id {
        Some(layer) => dim_dir.join(layer).join(zoom_str).join(x_str),
        None => dim_dir.join(zoom_str).join(x_str),
    };

    fs::create_dir_all(&target_dir)?;

    let target_path = target_dir.join(file_name);

    let img =
        image::RgbaImage::from_raw(TILE_SIZE as u32, TILE_SIZE as u32, buffer.pixels.to_vec())
            .ok_or_else(|| {
                AmberError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "failed to wrap raw tile buffer into rgba image",
                ))
            })?;

    let img_format = match format {
        TileFormat::Png => image::ImageFormat::Png,
        TileFormat::Webp => image::ImageFormat::WebP,
    };

    img.save_with_format(&target_path, img_format)
        .map_err(|e| AmberError::Io(std::io::Error::other(e)))?;

    Ok(target_path)
}

/// Saves a block identity sidecar (`blocks.bin`) beside a base tile,
/// mirroring the tile path layout so the client can derive the sidecar
/// URL from the tile URL.
pub fn save_block_index(
    block_index: &BlockIndex,
    base_dir: impl AsRef<Path>,
    dimension_name: &str,
    layer_id: Option<&str>,
    coord: TileCoord,
) -> Result<PathBuf> {
    let zoom_str = coord.zoom.to_string();
    let x_str = coord.x.to_string();
    let file_name = format!("{}.blocks.bin", coord.y);

    let dim_dir = base_dir.as_ref().join(dimension_name);
    let target_dir = match layer_id {
        Some(layer) => dim_dir.join(layer).join(zoom_str).join(x_str),
        None => dim_dir.join(zoom_str).join(x_str),
    };

    fs::create_dir_all(&target_dir)?;

    let target_path = target_dir.join(file_name);
    fs::write(&target_path, block_index.to_bytes())
        .map_err(|e| AmberError::Io(std::io::Error::other(e)))?;

    Ok(target_path)
}
