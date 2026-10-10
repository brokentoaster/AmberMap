# TBC-render-layer-slices: Multi-layer Y-slice rendering with web layer selection — Plan

## Spec

- Spec: docs/workplans/TBC-render-layer-slices-Spec.md
- Jira: n/a

## Overview

- Outcome: `ambermap render --layers <ranges>` emits one tile pyramid per
  Y-slice per dimension under `output/<dim>/<layer>/…`, the status API
  exposes the layer list and per-layer metadata, and the web viewer gains a
  layer selector that swaps tiles/coords like the dimension buttons do.
  Default behaviour (no flag) is unchanged.
- Approach:
  - Thread a `y_range: Option<(i16, i16)>` through `RenderOptions` into
    `rasterize_chunk`/`traverse_chunk`; the ray-march skips blocks whose
    `world_y` is outside the range (keeps compositing behaviour otherwise
    identical).
  - Reuse the existing per-dimension render loop: layer becomes an inner
    loop; the `dimension_name`/layer join is centralised in one path helper
    so disk and URL layouts can't drift.
  - The viewer's existing fetch-metadata→rebuild-map flow is reused for
    layers; the layer selector only changes what it fetches.

## Tasks

- [x] Task 1: Add Y-range filtering to the chunk raster pipeline
  - Notes: add `y_range: Option<(i16, i16)>` to `RenderOptions`; pass it
    through `rasterize_chunk` → `traverse_chunk`; in the ray loop, when a
    range is set, skip blocks with `world_y` outside `[min, max]` inclusive.
    `None` must behave exactly as today.
  - Touchpoints: `src/tiler/pipeline.rs`, `src/raster/chunk_raster.rs`,
    `src/raster/traverser.rs`
  - Done when: unit/integration tests show a slice with blocks above the
    range renders as if those blocks were air; `None` passes the existing
    raster test suite unchanged (spec AC 1, 2 foundations).
- [x] Task 2: Centralise tile path/URL layout and add the layer segment
  - Notes: extend `tile_path`/`save_tile` (`src/tiler/encoder.rs`) to take
    an optional layer id (`None` → legacy path, `Some(id)` →
    `<dim>/<layer>/z/x/y.ext`); surface layer id is `"surface"`. Keep the
    server's `/tiles/` handler layer-agnostic (it joins the subpath) but add
    a path-traversal guard for the new segment.
  - Touchpoints: `src/tiler/encoder.rs`, `src/tiler/pipeline.rs`,
    `src/web/server.rs`
  - Done when: roundtrip test covers a layered path; legacy (no-layer)
    layout byte-identical to today (spec AC 2).
- [x] Task 3: Parse `--layers` CLI ranges and render all layers per dimension
  - Notes: `--layers <y_start>:<y_end>[,...]` parse helper returning
    `Vec<(i16, i16)>`; in `main.rs`'s render loop, when layers are given,
    loop ranges inside the dimension loop and pass a layer id + y_range to
    `render_world_map`. Skip ranges with zero rendered tiles (warn, per
    empty-dimension handling). Surface render (no flag) stays a single pass.
    Ensure `--low-memory` pyramid and metadata writing work per layer
    (bounds/center already tracked per render call).
  - Touchpoints: `src/main.rs`, `src/tiler/pipeline.rs`
  - Done when: `--layers 0:63,-64:-1` produces two pyramids per dimension;
    empty ranges skipped with warning; no-flag run unchanged (spec AC 1, 5, 6).
- [x] Task 4: Serve layer list and per-layer metadata
  - Notes: extend metadata written per render to include the layer id and
    Y range; `status_metadata` (in `src/web/server.rs`) returns a `layers`
    array for `?dim=` and per-layer center/bounds/max_zoom for
    `?dim=&layer=`. Fallback rules preserved (missing layer →
    `rendered:false`-style stub).
  - Touchpoints: `src/tiler/pipeline.rs` (metadata JSON), `src/web/server.rs`
  - Done when: web test asserts layers array contents and per-layer metadata
    resolution (spec AC 3).
- [x] Task 5: Add the layer selector to the web viewer
  - Notes: on load and dimension switch, fetch `/api/status?dim=` to get
    `layers`; render a selector (dropdown or buttons) near the dimension
    buttons; selection triggers the same fetch-metadata→`initMap` flow with
    `&layer=<id>` and tile URLs `/tiles/{dim}/{layer}/z/x/y.ext`. Legacy
    trees (no `layers` in status) hide the selector. HUD/coords/recenter
    follow per-layer metadata.
  - Touchpoints: `web_client/js/app.js`, `web_client/index.html`,
    `web_client/css/map.css`
  - Done when: manual demo shows layer switching with correct tiles and
    coords; selector hidden for legacy output (spec AC 4).
- [x] Task 6: End-to-end verification and docs
  - Notes: run full `cargo fmt --check && cargo clippy -D warnings && cargo
    test`; manual multi-layer render + `--serve` walk-through of AC 1–6;
    update README CLI reference and AGENTS.md gotchas (layer segment in
    tile paths, status API shape).
  - Touchpoints: `README.md`, `AGENTS.md`
  - Done when: all spec AC verified; docs updated.

## Dependencies & prerequisites

- Task 2 after Task 1 (encoder/pipeline path changes build on the
  y_range-aware render options).
- Task 3 needs Task 1 + 2 (render loop and paths).
- Task 4 needs Task 3 (metadata content comes from the render loop).
- Task 5 needs Task 4 (viewer consumes the status API shape).
- Task 6 last (full validation).

## Verification

- See spec Acceptance Criteria and Verification Plan
- Extra build-time check: a regression test asserting the no-flag render
  writes the exact legacy tile path layout (guards AC 2 against drift)

## Risks & mitigations

- Risk: Y-range filtering interacts with the nether roof skip (bedrock skip
  happens only while `recorded_height.is_none()`) → Mitigate: apply the
  range filter independently of the roof-skip condition; add a nether slice
  regression test.
- Risk: memory for many layers is `layers × dims × peak-of-one-render` in
  default mode → Mitigate: document combining `--layers` with `--low-memory`
  for big worlds; layers render sequentially so peak is one layer at a time.
- Open: WebP lossiness compounds across pyramid re-decodes for layered
  low-memory renders (accepted; PNG avoids it).

## Out of scope

- 3D/isometric views, block-type filters, live re-render, `inspect` /
  `render-chunk` changes (see spec Out of scope).

## Rollback

- See spec Rollback Plan (feature is opt-in via `--layers`; revert binary).