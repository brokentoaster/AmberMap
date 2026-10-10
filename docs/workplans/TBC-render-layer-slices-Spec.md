# TBC-render-layer-slices: Multi-layer Y-slice rendering with web layer selection

## Goal

Users can render a world as a set of vertical Y-slice maps ("layers") per
dimension, and switch between those layers in the embedded web viewer to see
what exists at different depths (e.g. caves, tunnels, underground bases), not
just the top-down surface.

## Context

Today AmberMap renders one top-down surface map per dimension (`overworld`,
`nether`, `the_end`). The voxel traverser composites top-down with early ray
termination, so underground features are invisible. The web viewer already
switches dimensions via buttons backed by per-dimension metadata
(`/api/status?dim=`) and tile URLs `/tiles/{dim}/{z}/{x}/{y}.{ext}`.

## Scope

### In scope

- CLI option on `render` to render multiple Y-layer slices (layer definition:
  range and step over block Y; a layer covers blocks `y_start ..= y_end`).
- Per-layer tile output stored alongside existing tiles (layer becomes part of
  the on-disk path and the tile URL).
- Per-layer metadata (center, bounds, max_zoom, format, layer label/Y range)
  served through the status API.
- Web UI: a layer selector (dropdown or slider) next to the existing
  dimension buttons; switching layers swaps the tile layer and metadata the
  same way dimension switching does.
- Default behaviour unchanged when the option is omitted: a single surface
  layer, backward-compatible with existing tile directories and the current
  web UI defaults.
- `--low-memory` compatibility: pyramid generation works for layer slices the
  same as surface maps.

### Out of scope

- 3D/isometric views, cave-only filters, or block-type filtering beyond the
  Y range.
- Serving layers from a live/re-rendering world (static rendered output only).
- New tile formats or zoom levels beyond what exists.
- Changes to `inspect` / `render-chunk` commands.

## Constraints & Assumptions

- A Y-slice layer is produced by restricting the traverser's ray-march to the
  layer's Y range; pixels above the range must not leak into the slice.
- Empty layers (no blocks in range) are skipped with a warning, consistent
  with empty-dimension handling.
- Tile URL path gains a layer segment; existing clients (current `app.js`)
  must keep working against old tile trees without a layer segment.

## Decisions

- Layers are rendered in the same run as the dimension (no separate command);
  layer selection is a render option, not a viewer-side re-render.
- The viewer fetches the available layer list from the status API rather than
  guessing Y ranges.
- **CLI syntax** (resolved): repeatable value on `render`,
  `--layers <y_start>:<y_end>[,<y_start>:<y_end>...]` with inclusive block-Y
  ranges, e.g. `--layers 0:63,-64:-1` renders two layers. Whitespace is
  ignored; each range becomes one layer. A future preset shorthand may be
  added without breaking this form.
- **Tile path layout** (resolved): insert the layer as a directory segment
  directly under the dimension, before zoom:
  `output/<dim>/<layer>/z/x/y.ext` and URL
  `/tiles/{dim}/{layer}/{z}/{x}/{y}.{ext}`. The default/surface layer uses
  the segment `surface`. Legacy trees without the segment keep working:
  when the URL has no layer segment, the server and viewer fall back to the
  plain `output/<dim>/z/x/y.ext` layout.
- **Status API shape** (resolved): `/api/status?dim=<name>` gains a
  `"layers"` array, each entry `{"id": "surface", "y_min": 0, "y_max": 63,
  "label": "Y 0..63"}`. The default layer is always present with id
  `surface` and covers the full renderable Y range. Responses for a specific
  layer use `/api/status?dim=<name>&layer=<id>` and additionally carry the
  per-layer center/bounds/max_zoom.
- **Layer labels** (resolved): auto-generated from the Y range
  (`Y 0..63`); no user naming in this iteration.

## Acceptance Criteria

1. `ambermap render <WORLD> --layers <spec>` renders every non-empty Y slice
   layer for every rendered dimension; each layer's tiles are retrievable via
   the tile HTTP route with a layer identifier.
2. Without `--layers`, output is byte-for-byte compatible with today's layout
   (no layer segment in paths, current web UI unaffected).
3. `/api/status` returns, for a dimension, the list of rendered layers with
   their Y ranges and labels; the response for a layer includes usable
   center/bounds/max_zoom.
4. In the web UI, selecting a layer loads that layer's tiles, updates
   HUD/bounds/zoom like dimension switching does, and selecting the default
   layer reproduces the current surface view.
5. Rendering a world with `--low-memory --layers <spec>` completes with the
   same per-layer tile results (pyramid included) as the default path for
   that layer.
6. Layers with no blocks in their Y range are skipped, reported, and do not
   appear in the status API layer list.

## Verification Plan

- Rust integration tests: traverser honours Y-range restriction (blocks above
  the range ignored); empty layers produce no tiles and are omitted from
  metadata; default path unchanged.
- Web test: status API returns the layer list and per-layer metadata.
- Manual demo: render a small world with several layers, serve, and switch
  dimension and layer in the browser; confirm tiles, HUD, and recenter all
  follow the selection.
- Memory check: run the multi-layer render with `--low-memory` and confirm
  peak RAM stays comparable to the single-surface run.

## Rollback Plan

- The feature is opt-in via `--layers`; dropping the flag and redeploying the
  binary restores previous behaviour. Old rendered tile trees remain valid.

## Open Questions

- None — all previously open items are resolved in Decisions. (Confirmed
  TBC from scratch: no Jira issue key exists for this work.)