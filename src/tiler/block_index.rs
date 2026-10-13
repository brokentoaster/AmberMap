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

//! Per-base-tile block identity index emitted alongside tile images so
//! the web viewer can show the block under the cursor.
//!
//! Binary layout (little endian):
//!   - 4 bytes magic `AMBI`
//!   - u16 format version (currently 1)
//!   - u16 name count N
//!   - N null-terminated UTF-8 block names (without a length prefix;
//!     terminated by 0x00)
//!   - 256*256 u16 name indices, row-major by Z then X
//!   - 256*256 i16 top-block Y values, row-major by Z then X
//!   - u16 sentinel index reserved: index `u16::MAX` means "no block"

use crate::raster::chunk_raster::ChunkRasterResult;
use crate::raster::traverser::NO_BLOCK;
use crate::tiler::compositor::TILE_SIZE;

pub const BLOCK_INDEX_MAGIC: &[u8; 4] = b"AMBI";
pub const BLOCK_INDEX_VERSION: u16 = 1;

#[derive(Clone, Debug, Default)]
pub struct BlockIndex {
    /// Interned block names, indexed by the grid values.
    pub names: Vec<String>,
    /// Row-major by Z then X: interned name index or `NO_BLOCK`.
    pub grid: Vec<u16>,
    /// Row-major by Z then X: top visible block world Y per column.
    pub top_y: Vec<i16>,
}

impl BlockIndex {
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            grid: vec![NO_BLOCK; TILE_SIZE * TILE_SIZE],
            top_y: vec![0; TILE_SIZE * TILE_SIZE],
        }
    }

    /// Blits a 16x16 chunk result into the tile grid at the chunk's
    /// offset within the tile.
    pub fn blit_chunk(&mut self, ox: usize, oy: usize, result: &ChunkRasterResult) {
        for z in 0..16 {
            for x in 0..16 {
                let col = z * 16 + x;
                let idx = result.top_block[col];
                if idx == NO_BLOCK {
                    continue;
                }
                let Some(name) = result.block_names.get(idx as usize) else {
                    continue;
                };
                let interned = self.intern(name);
                let tile_col = (oy + z) * TILE_SIZE + (ox + x);
                self.grid[tile_col] = interned;
                self.top_y[tile_col] = result.heightmap[col];
            }
        }
    }

    fn intern(&mut self, name: &str) -> u16 {
        if let Some(i) = self.names.iter().position(|n| n == name) {
            return i as u16;
        }
        self.names.push(name.to_string());
        (self.names.len() - 1) as u16
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.grid.iter().all(|&i| i == NO_BLOCK)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.names.len() * 24 + self.grid.len() * 4);
        out.extend_from_slice(BLOCK_INDEX_MAGIC);
        out.extend_from_slice(&BLOCK_INDEX_VERSION.to_le_bytes());
        out.extend_from_slice(&(self.names.len() as u16).to_le_bytes());
        for name in &self.names {
            out.extend_from_slice(name.as_bytes());
            out.push(0);
        }
        for v in &self.grid {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in &self.top_y {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 || &data[0..4] != BLOCK_INDEX_MAGIC {
            return None;
        }
        let version = u16::from_le_bytes([data[4], data[5]]);
        if version != BLOCK_INDEX_VERSION {
            return None;
        }
        let name_count = u16::from_le_bytes([data[6], data[7]]) as usize;
        let mut pos = 8;
        let mut names = Vec::with_capacity(name_count);
        for _ in 0..name_count {
            let end = data[pos..].iter().position(|&b| b == 0)? + pos;
            names.push(String::from_utf8_lossy(&data[pos..end]).into_owned());
            pos = end + 1;
        }
        let grid_len = TILE_SIZE * TILE_SIZE;
        if data.len() < pos + grid_len * 2 + grid_len * 2 {
            return None;
        }
        let mut grid = Vec::with_capacity(grid_len);
        for _ in 0..grid_len {
            grid.push(u16::from_le_bytes([data[pos], data[pos + 1]]));
            pos += 2;
        }
        let mut top_y = Vec::with_capacity(grid_len);
        for _ in 0..grid_len {
            top_y.push(i16::from_le_bytes([data[pos], data[pos + 1]]));
            pos += 2;
        }
        Some(Self { names, grid, top_y })
    }

    /// Looks up the block name and Y at a tile-local column, if present.
    pub fn lookup(&self, local_x: usize, local_z: usize) -> Option<(&str, i16)> {
        if local_x >= TILE_SIZE || local_z >= TILE_SIZE {
            return None;
        }
        let col = local_z * TILE_SIZE + local_x;
        let idx = self.grid[col];
        if idx == NO_BLOCK {
            return None;
        }
        let name = self.names.get(idx as usize)?;
        Some((name.as_str(), self.top_y[col]))
    }
}
