# TBC-nether-coords: Nether coordinate alignment and stable viewport

## Goal

When viewing the Nether in the web viewer, the map is displayed at
overworld-equivalent scale (1 nether block = 8 overworld blocks) and all
positions (markers, HUD coordinates) are shown in both nether and overworld
coordinates. Switching dimensions or layers preserves the viewport center and
zoom level instead of resetting.

## Context

The viewer (`web_client/js/app.js`) currently renders every dimension with
raw block coordinates in `L.CRS.Simple`. The Nether is 1:8 scale relative to
the Overworld, so the raw-coordinate Nether map is 8x too small next to the
overworld map and portal-linked positions do not line up between views.

Marker data (`markers.json`, produced by `src/tiler/pipeline.rs` from
`src/markers/`) is collected per dimension and rendered with raw block
coordinates. Player tooltips show only that dimension's native coordinates.
Dimension and layer switching call `loadDimension`, which rebuilds the map
(`initMap`) and re-fits bounds, discarding the user's viewport and zoom.

## Scope

### In scope

- Nether map displayed at overworld-equivalent scale: nether block
  coordinates are scaled by 8 in the viewer's CRS so the nether map covers
  the same world extent as its overworld counterpart.
- Player and sign markers positioned at the scaled (overworld-equivalent)
  coordinates when viewing the Nether.
- HUD block/chunk readout in the Nether shows both nether coordinates and
  the converted overworld coordinates (nether x/z multiplied by 8; the
  overworld y is nether y + 64 for player tooltips and HUD where height is
  shown).
- Player tooltip in the Nether lists nether position and overworld position.
- Switching dimension (overworld <-> nether <-> the end) keeps the current
  viewport center and zoom level instead of re-fitting bounds.
- Switching layer (surface / other slices) within a dimension keeps the
  current viewport center and zoom level.

### Out of scope

- Rendering/tiling changes: tile pyramid layout and tile pixel content are
  unchanged; scaling is a viewer-side coordinate transform.
- Portal detection or linking markers to portal pairs.
- Changes to the End dimension's scale (it is 1:1; only behavior shared with
  the viewport-preservation change applies).
- Live player tracking.

## Constraints & Assumptions

- The overworld-to-nether conversion factor is exactly 8 horizontally and
  the nether ceiling offset is 64 vertically (Bedrock Edition standard).
- `/api/status` per-dimension metadata (center/bounds) remains in native
  block coordinates; the viewer applies any scaling.
- All changes to `app.js` must remain additive to the existing status
  contract; older rendered worlds must still display sensibly.
- CI enforces `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`; web client has no build step or JS test harness.

## Decisions

- Scaling happens entirely in the web client (CRS transformation), not by
  re-rendering nether tiles at a different scale, so existing rendered
  worlds work without a re-render.
- Both coordinate sets are displayed where height is shown (player tooltips
  in the Nether); the mouse-tracking HUD shows both X/Z pairs.
- Each player record is a snapshot in its own dimension's native
  coordinates and is shown only on the map of the dimension the player was
  in at render time.

## Acceptance Criteria

1. Viewing the Nether, the rendered map extent corresponds to the same
   overworld block range as the overworld map (nether blocks scaled by 8);
   hovering the HUD at a portal location shows overworld coordinates that
   match the same location on the overworld map.
2. A player whose stored position is in the Nether appears on the Nether
   map at the overworld-equivalent position (8x scaled), and their tooltip
   shows both nether and overworld X/Y/Z.
3. Signs in the Nether appear at overworld-equivalent positions.
4. Switching from the Overworld to the Nether (and back) keeps the current
   center and zoom level; only tile content and marker set change.
5. Switching layers within a dimension keeps the current center and zoom.
6. Worlds rendered before this change still load in the viewer without
   errors; the Nether still displays correctly.
7. Existing `cargo fmt --check`, `cargo clippy -- -D warnings`, and
   `cargo test` pass.

## Verification Plan

- Manual demo with a world that has Nether content and a player in the
  Nether: `cargo run -- render <WORLD> --serve --open`. Verify HUD
  conversion, marker positions, and tooltip dual coordinates.
- Manual check: pan/zoom somewhere distinctive, switch dimension and layer,
  confirm the viewport is unchanged.
- Renderer-side marker JSON is unchanged, so existing `markers_tests.rs`
  and `tiler_tests.rs` confirm no tile/data regressions.

## Rollback Plan

- Revert the branch; the change is client-only and additive, no data or
  output migration is involved.

## Open Questions

- Should the End dimension's viewport also snap to overworld coordinates
  (it is 1:1, so no scaling, only the viewport-preservation behavior
  applies)? Assumed yes.
- When the user has never moved the viewport and switches dimension, is
  "keep viewport" the same as fitting the new dimension's bounds, or should
  the first switch also keep the default view? Assumed: keep the current
  viewport once the user has interacted; initial load still fits bounds.
- Whether layer switching should also preserve the currently selected layer
  across dimensions when that layer exists there (currently resets to
  `surface`). Assumed out of scope for this change.
