# TBC-tile-info-overlay: Block information overlay on the map viewer — Plan

## Spec

- Spec: `docs/workplans/TBC-tile-info-overlay-Spec.md`
- Jira: n/a

## Overview

- Outcome: hovering the map shows the top visible block under the cursor
  (name, X/Z, Y, dimension/layer), sourced from per-base-tile sidecar
  files emitted at render time and served by the embedded server.
- Approach:
  - The column ray marcher (`traverse_chunk_y_range`) already picks the
    first visible block per column; extend it to also record that block's
    identity. Record a per-chunk interned index (u16 per column + name
    table) to keep memory flat.
  - The tile pipeline aggregates chunk results into a 256x256 u16 grid
    plus one name table per base tile and writes a binary sidecar next to
    the base tile image at the base zoom level only.
  - The server serves sidecars under the existing `/tiles/...` scheme
    (content-type addition only).
  - The client lazy-loads and caches the sidecar for the base tile under
    the cursor and renders the overlay from it.

## Tasks

- [x] Task 1: Record the top visible block per column in the ray marcher
  - Notes: extend `ColumnScanResult` with a per-column interned block
    index (`[u16; 256]`, sentinel for "none") and a chunk-local name
    table (`Vec<String>` of unique top-block names). At the point where
    `recorded_height` is first set (the first visible block), also record
    the palette name. Thread through `rasterize_chunk_y_range` /
    `ChunkRasterResult`. Keep hillshading untouched.
  - Touchpoints: `src/raster/traverser.rs`, `src/raster/chunk_raster.rs`,
    `tests/raster_tests.rs`
  - Done when: synthetic raster tests assert top-block name per column
    (dirt/grass/water column cases), existing tests still pass.
- [x] Task 2: Emit per-base-tile block sidecar files
  - Notes: define the binary sidecar format (header + magic + version,
    name table, 256x256 u16 block-name indices, 256x256 i16 Y column,
    matching the spec default decisions). Aggregate in
    `render_world_map_with_db`: alongside each base `TileBuffer`, build a
    block-index grid from each chunk's result, write
    `blocks.bin` next to the base tile at the base zoom level only
    (skip for pyramid levels and skip when the tile is empty). Emit and
    release per tile to avoid extra peak memory. Write-sidecars flag not
    needed — always emit.
  - Touchpoints: `src/tiler/pipeline.rs`, `src/tiler/encoder.rs` (new
    `save_block_index`), new module e.g. `src/tiler/block_index.rs`,
    `tests/tiler_tests.rs`
  - Done when: synthetic render produces `blocks.bin` beside base tiles,
    round-trip parse (encode→parse) matches the expected grid and names,
    and no sidecars exist for non-base zoom levels.
- [x] Task 3: Serve sidecar files from the embedded server
  - Notes: confirm the tile request path handling accepts `.bin` under
    `/tiles/...` and add a content-type mapping
    (`application/octet-stream`). No routing changes expected.
  - Touchpoints: `src/web/server.rs`, `tests/web_tests.rs`
  - Done when: synthetic web test fetches a sidecar path successfully.
- [x] Task 4: Client overlay showing block info under the cursor
  - Notes: add an overlay element to `web_client/index.html` and styling
    to `css/map.css` (pointer-events: none, HUD-consistent look). In
    `app.js`: on `mousemove`, compute the cursor's world block coords
    (already available in `updateCoords`), derive the base tile
    (`blockX >> 8`, `blockZ >> 8`), fetch
    `/tiles/<dim>[/<layer>]/<maxZoom>/<tx>/<ty>.blocks.bin` (final path
    per Task 2) once and cache per dimension/layer/tile in a `Map`.
    Parse the binary (name table + grids), look up column
    (`blockX & 255`, `blockZ & 255`), and display block name, X/Z, Y,
    dimension/layer. Show a "no block" state for missing sidecars, empty
    columns, or cursors outside rendered tiles. Keep the existing HUD
    readout unchanged.
  - Touchpoints: `web_client/index.html`, `web_client/css/map.css`,
    `web_client/js/app.js`
  - Done when: spec AC 2, 4, 5, 7 observed in a manual demo with a real
    world.
- [x] Task 5: Final verification
  - Notes: run `cargo fmt --check`, `cargo clippy -- -D warnings`,
    `cargo test`; manual demo per spec Verification Plan including
    dimension/layer switching and output-size comparison on a small
    world.
  - Done when: spec AC 1, 6, 8 hold and CI-equivalent checks are green.

## Dependencies & prerequisites

- Task 2 depends on Task 1 (block identity per column).
- Task 4 depends on Task 2 (format) and Task 3 (serving).
- Task 5 after all others.

## Risks & mitigations

- Risk: per-chunk name tables string-allocate per chunk; on large worlds
  this adds render overhead. → Mitigate: intern only the single top-block
  name per column into a chunk-local table (typically a handful of
  entries); keep the hot loop allocation-free (store palette index during
  the march, resolve names once after the column scan).
- Risk: sidecar doubling output size for large worlds. → Mitigate: u16
  indices + name table ≈ 128 KB per full tile plus a small table; measure
  on a small world in Task 5 and, if needed, shrink to per-tile unique
  names with 8-bit indices when a tile has ≤256 distinct blocks.
- Risk: path/filename collision with the tile encoder layout (layer
  directories). → Mitigate: place the sidecar exactly beside the base
  tile image and mirror the `save_tile_layered` path logic.
- Open: none beyond spec Open Questions (format/y-inclusion defaults are
  recorded in the spec Decisions).

## Out of scope

- Full column content, block search/highlighting, tile image changes,
  live LevelDB queries (see spec Scope).

## Rollback

- See spec Rollback Plan (revert branch; sidecars are additive and
  feature-detected by the client).