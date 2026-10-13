# TBC-tile-info-overlay: Block information overlay on the map viewer

## Goal

While browsing the rendered map, a user can see what block is under the
mouse cursor — e.g. dirt, grass, gold ore — in an on-screen overlay. The
overlay shows the block's type and position for the top visible block at
the cursor's column, so the user can inspect the world directly from the
map without digging in-game.

## Context

The map is a flat top-down raster; the client (`web_client/js/app.js`)
only knows pixel colors, not block identities. Block identity is known at
render time inside the tile pipeline: `rasterize_chunk` resolves each
voxel column's top visible block via the subchunk palette (`SubChunk` →
block names). To show block info in the viewer, the renderer must export
a per-tile block index alongside the existing tile images, and the
embedded server must serve it.

The viewer already tracks the cursor and shows block/chunk coordinates in
a HUD; the overlay extends this with block identity, sourced from the
exported data.

## Scope

### In scope

- Renderer: during tile rasterization, record the top visible block name
  per world block for every base tile (256x256), and emit a compact
  per-tile sidecar file (e.g. `blocks.bin`/`blocks.json` next to the tile
  image at the base zoom level) mapping pixel position to block name.
- Server: serve the sidecar files alongside tiles under the existing
  `/tiles/...` URL scheme.
- Web client: an overlay (styled consistently with the existing HUD)
  that, while the cursor is over the map, shows:
  - block name (e.g. `minecraft:gold_ore`);
  - block position X/Z (and Y where available);
  - dimension and layer currently displayed.
- Fetching strategy for the client: lazy-load the sidecar for the base
  tile under the cursor and cache it for the session.
- When the cursor is over an unrendered area or a block with no entry,
  the overlay shows a clear "no block" state.
- The overlay must not block map interaction (pointer-events disabled).

### Out of scope

- Full column content (every block down the Y axis) — only the top
  visible block at the column.
- Historical/rendered-away layers beyond the currently selected layer.
- Click-to-pin, block search, or highlighting all blocks of a type.
- Changing tile image content or the tile pyramid layout.

## Constraints & Assumptions

- Sidecar data must not bloat output: one compact entry per rendered
  block column (block id or interned name index), not full NBT.
- The existing render already wipes the output directory and holds base
  tiles in memory; the sidecar generation must not increase peak memory
  materially (emit per tile, then release).
- The web client must keep working with the existing `/api/status`
  contract (additive changes only); worlds rendered before this change
  show the "no block" state gracefully.
- The Nether's display zoom extension means deep-zoom views upscale
  coarser tiles; block identity is only available at base-tile resolution
  (one entry per world block), which is independent of display zoom.
- CI enforces fmt/clippy/tests; renderer-side tests are synthetic-data
  based (no binary fixtures).

## Decisions

- Block identity is exported at render time as a per-base-tile sidecar,
  not queried live from LevelDB by the server (matches the markers
  snapshot approach).
- Only the top visible block per column is recorded — the same block the
  rasterizer chose as the visible color, so overlay and pixels agree.

## Acceptance Criteria

1. After a render, each base tile at the base zoom level has a sidecar
   file containing the top visible block name for every rendered block
   column of that tile.
2. Hovering the cursor over terrain shows an overlay with the block name
   and X/Z position; hovering over a grass, dirt, stone, and ore block
   shows the correct distinct names.
3. The block shown matches the visible pixel color mapping: re-rendering
   a known synthetic chunk and hovering its blocks yields the palette
   names used by `ColorMap`.
4. Selecting a different layer or dimension switches the block data
   source; hovering shows the top block of the selected layer.
5. Worlds rendered before this change still load; the overlay shows the
   "no block" state without errors.
6. Render time and output size increase is modest: sidecar data adds a
   small constant per rendered block column, and tile images are
   unchanged.
7. The overlay does not intercept mouse events; panning and zooming work
   normally.
8. `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`
   pass; synthetic tests cover sidecar generation and parsing.

## Verification Plan

- Synthetic renderer tests: build a chunk with a known palette, render a
  tile, and assert the sidecar maps specific pixels to expected block
  names (spec AC 3).
- Synthetic server test: sidecar files are served at the expected paths
  with a sensible content type.
- Manual demo: `cargo run -- render <WORLD> --serve --open`; hover over
  known builds (e.g. a gold block, grass field) and confirm names; switch
  dimension/layer and re-check (spec AC 2, 4).
- Compare output size before/after on a small world for AC 6.

## Rollback Plan

- Revert the branch. Sidecar files are additive; the client feature-
  detects their presence (as it does for markers), so older outputs
  simply show the "no block" state.

## Open Questions

- Sidecar format: binary with an interned block-name table (compact, fast)
  vs plain JSON per tile (simple, larger). Default assumption: binary with
  a name table at the top of the file.
- Should sidecars be emitted for every zoom level or only the base level?
  Default assumption: base level only; the client derives the base tile
  from the cursor position at any display zoom.
- Should the Y height of the top block be included in the sidecar?
  Default assumption: yes, as a single value per column.
- Memory approach on very large worlds (emit-then-release per tile vs
  whole-map buffer) — decide during implementation.