# TBC-render-speedup: Speed up world map rendering

## Goal

Rendering a Bedrock world to a tile map completes noticeably faster, with
byte-identical output tiles for the same world and options. Target: at least
2x faster end-to-end wall time on a typical overworld render.

## Context

Profiling and code review identified the hot path as per-voxel block-name
resolution: `traverse_chunk_y_range` (src/raster/traverser.rs) calls
`ColorMap::is_air` and `ColorMap::get` with `&str` keys for every voxel of
every subchunk layer (up to ~65k string hash lookups per chunk). Secondary
costs: scanning all 4096 indices of subchunks whose palette contains only
air, and LevelDB fetch/decompress/palette-decode happening serially with
respect to rasterization order inside the tile loop
(src/tiler/pipeline.rs). The existing rayon parallelism is per base tile
only.

## Scope

### In scope

- Precompute a per-layer palette lookup table (color, air flag, nether-bedrock
  flag) so the voxel loop does zero string lookups.
- Skip subchunks whose palette is entirely air/non-rendering blocks.
- Prefetch and decode chunks in parallel ahead of rasterization (or make
  `WorldDb` reads safely parallel), so decode overlaps with raster work.
- Keep the nether bedrock-roof skipping behavior and Y-layer slicing
  (`rasterize_chunk_y_range`) semantics unchanged.

### Out of scope

- Changing the tile output format, directory layout, or `metadata.json` shape.
- Memory-footprint restructuring of the pyramid stage (Mutex tile cache stays
  as-is); may be a follow-up.
- Encoder (WebP/PNG) changes or alternate compression settings.
- Multi-level caching of previously rendered tiles / incremental re-render.

## Constraints & Assumptions

- CI enforces `cargo fmt --check` and `cargo clippy -- -D warnings`.
- Release profile uses `panic = "abort"` and LTO; avoid relying on unwinding.
- Tests are synthetic-data integration tests in `tests/*.rs`; no fixture files.
- LevelDB reads may not be thread-safe; if so, decode into owned `ChunkData`
  values in a parallel prefetch pass rather than sharing the reader.

## Decisions

- Optimize the raster inner loop first (palette LUT + air-skip), then decode
  parallelism; measure after each stage before adding further micro-optimizations
  (e.g. fixed-point compositing).

## Acceptance Criteria

1. Rendered output for a fixed test world is pixel-identical before and after
   the change (same tiles, format, dimensions, layer renders).
2. Nether renders still skip the bedrock roof; overworld bedrock still renders.
3. Layer (Y-slice) renders produce the same output as before the change.
4. End-to-end render time on a representative world is at least 2x faster than
   the pre-change baseline on the same machine and thread count.
5. `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` all pass.
6. Existing tests pass unchanged (or are updated only where their assertions
   matched the old string-lookup internals, not observable behavior).

## Verification Plan

- Baseline: time `cargo run --release -- render <WORLD_DIR> --output <tmp>` on
  a real world folder before the change; repeat after each stage.
- Pixel-diff a sample of output tiles (base tiles plus a few pyramid levels)
  between baseline and new build.
- Run the existing integration suite plus a new test that renders a synthetic
  nether chunk and asserts bedrock-roof skipping still applies.
- CI (fmt, clippy, tests) on Linux/Windows/macOS.

## Rollback Plan

- Revert the branch; rendering pipeline and output format are unchanged, so no
  data migration or cleanup is needed.

## Open Questions

- Does the LevelDB reader wrapper permit concurrent reads from multiple rayon
  threads (external to the block cache)? Determines prefetch design.
- Whether fixed-point alpha compositing is worth pursuing if the 2x target is
  not met after the first two stages.