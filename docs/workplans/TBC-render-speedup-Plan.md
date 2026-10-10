# TBC-render-speedup: Speed up world map rendering — Plan

## Spec

- Spec: docs/workplans/TBC-render-speedup-Spec.md
- Jira: n/a

## Overview

- Outcome: map rendering is at least 2x faster with pixel-identical output.
- Approach: three measured stages — (1) per-layer palette LUT so the voxel
  loop does no string lookups, (2) skip subchunks with all-air palettes,
  (3) parallel chunk fetch/decode ahead of rasterization. Re-baseline wall
  time after each stage; stop adding optimizations once the 2x target is met.
- Baseline first: time `cargo run --release -- render <WORLD_DIR> --output
  <tmp>` on a real world and save output tiles for pixel-diffing.

## Tasks

- [x] Task 1: Establish baseline timing and output snapshot
  - Notes: Run a release render on a representative world, record wall time,
    thread count, and copy the output tile tree to a reference directory for
    pixel-diffing in later tasks.
  - Done when: baseline time + reference tiles captured and noted.

- [x] Task 2: Add per-layer palette lookup table to the rasterizer
  - Notes: Build a small per-layer entry (color, air flag, nether-bedrock
    flag) once per subchunk layer, indexed by palette index, in
    `traverse_chunk_y_range`. Replace `colormap.is_air(name)` /
    `colormap.get(name)` calls with LUT array indexing. Keep behavior
    identical, including nether roof skipping and Y-slice filtering.
  - Touchpoints: `src/raster/traverser.rs`, `src/raster/colormap.rs`,
    `src/raster/mod.rs` (re-exports if new public types)
  - Done when: `cargo test` passes, clippy/fmt clean, output pixel-identical
    to baseline (see spec AC 1-3, 5).

- [x] Task 3: Skip subchunks whose palette has no renderable blocks
  - Notes: Precompute an all-air flag per subchunk layer (from the palette)
    and skip the whole layer in the ray march when set. Re-measure timing.
  - Touchpoints: `src/raster/traverser.rs` (or `src/subchunk/` if the flag
    belongs on the decoded struct)
  - Done when: `cargo test` passes, output pixel-identical, timing improved
    or neutral vs Task 2.

- [x] Task 4: Parallelize chunk fetch/decode ahead of rasterization
  - Notes: If the LevelDB reader cannot be shared across rayon threads,
    prefetch + decompress chunks into owned `ChunkData` values with a
    `par_iter` pass, then rasterize from the decoded set. Re-measure timing.
  - Touchpoints: `src/tiler/pipeline.rs`, `src/leveldb/reader.rs`
  - Done when: `cargo test` passes, output pixel-identical, end-to-end time
    meets spec AC 4 (>= 2x faster than Task 1 baseline) or remaining gap is
    documented.

- [x] Task 5: Final verification against spec acceptance criteria
  - Notes: Pixel-diff sample of base + pyramid tiles vs baseline, run fmt,
    clippy `-D warnings`, and full test suite; confirm nether roof and
    Y-layer-slice behavior unchanged (spec AC 1-6).
  - Done when: all spec AC verified.

## Dependencies & prerequisites

- Task 1 before all others (baseline and reference tiles are the yardstick).
- Tasks 2-4 are sequential by design: each stage is measured before deciding
  whether the next is needed.
- Prerequisite: a real Bedrock world folder available on this machine for
  benchmarking (synthetic tests alone cannot measure wall time).

## Verification

- See spec Acceptance Criteria and Verification Plan
- Extra: keep the Task 1 reference tile tree and pixel-diff a sample of
  tiles after each stage; record per-stage timings in the task notes.

## Risks & mitigations

- Risk: LevelDB reader is not thread-safe for concurrent `get`s → Mitigate:
  decode into owned `ChunkData` in a parallel prefetch pass instead of
  sharing the reader.
- Risk: palette-LUT refactor subtly changes nether-roof or air handling →
  Mitigate: pixel-diff against baseline plus a synthetic nether chunk test.
- Open: whether fixed-point alpha compositing is needed to hit the 2x target
  (spec Open Question; decide after Tasks 2-4 timings).

## Out of scope

- Tile format/layout/metadata changes; pyramid-stage memory restructuring;
  encoder changes; incremental re-render caching (see spec Out of scope).

## Rollback

- See spec Rollback Plan (revert branch; no data migration).