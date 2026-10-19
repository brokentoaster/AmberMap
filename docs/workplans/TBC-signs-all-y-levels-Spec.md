# TBC-signs-all-y-levels: Surface-visible sign markers with web UI toggle

## Goal

When viewing the surface layer of any dimension in the web client, every sign
in the world is visible on the map regardless of its Y level, and the user can
show or hide sign markers with a UI toggle (like the existing Players toggle).

## Context

Sign markers were added in `TBC-map-markers` (block entities, key tag 0x31).
Two gaps remain for surface viewing:

- Sign visibility depends on where the user happens to be looking, not on
  what is rendered: block-entity collection is currently scoped by the
  rendered layer/tile pass, so signs at Y levels outside the current view
  (e.g. underground or high-altitude builds on the surface layer) are
  missing from the overlay.
- There is no web UI control for signs; player markers have a
  `btn-toggle-players` toggle (persisted in `localStorage`), signs do not.

AmberMap renders a per-dimension tile pyramid. Block entities are stored
per-chunk in LevelDB and are independent of the rasterized Y range, so a
full-dimension sign sweep is a cheap, single pass over existing records.

## Scope

### In scope

- Collect signs for a whole dimension (all chunks, all Y levels) during
  render, independent of the rendered layer or Y slice, so the surface view
  shows every sign.
- Emit one sign overlay dataset per dimension (extend or replace the current
  `markers.json` shape) covering all collected signs.
- Serve the per-dimension sign overlay to the web client (existing
  `/tiles/<dim>/…markers.json` pattern or equivalent additive endpoint).
- Web client: render sign markers on the active dimension's map, honoring a
  new sign toggle button in the header actions, persisted in `localStorage`.
- Toggle state survives page reloads and dimension/layer switches (markers
  re-render per dimension, same as players today).
- Malformed/unparseable records are skipped silently (consistent with the
  lenient decompression and parser conventions).

### Out of scope

- Real-time updates: sign data is a snapshot at render time.
- Editing signs, click-through navigation, or sign-side (front/back)
  selection beyond the existing first-visible-side rule.
- Markers for other block entities (chests, item frames, etc.).
- Changes to tile pixels or the tile pyramid; signs are a data overlay only.
- Player-marker behavior changes beyond reusing the existing toggle pattern.

## Constraints & Assumptions

- Worlds may be large; the sign sweep must not noticeably slow the render
  pipeline (it is a lightweight pass over block-entity records, no chunk
  voxel decode).
- Marker JSON must stay small (position + text lines only).
- The web client keeps the existing `/api/status` contract; changes are
  additive.
- Nether display scaling (1 nether block = 8 overworld blocks) applies to
  sign positions, same as player markers use `coordScale()`.
- CI enforces `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`; tests use synthetic data generators, no binary fixtures.

## Decisions

- Signs are collected in a dedicated full-dimension pass over block-entity
  records (not per-tile during rasterization), so coverage is complete and
  independent of the layer being rendered.
- The toggle mirrors the existing Players button pattern (class, icon style,
  `localStorage` persistence, `classList.toggle('active')`).

## Acceptance Criteria

1. After a render, the per-dimension sign overlay contains every sign found
   in all chunks of that dimension, including signs above and below the
   rendered surface Y range (e.g. a sign at Y -60 and one at Y 250 both
   appear in the surface view).
2. The sign overlay is produced for each dimension (overworld, nether,
   the_end) rendered, and is empty-but-valid when a dimension has no signs.
3. The web client displays sign markers for the active dimension on the
   surface view and on any Y-slice layer view.
4. A header toggle button turns sign markers on and off; the state persists
   across page reloads (localStorage) and defaults to on.
5. Toggling signs off and back on restores the markers without a data
   refetch (client-side layer removal/re-add).
6. Switching dimensions or layers re-renders the sign overlay for the new
   view; nether sign positions are displayed at overworld-equivalent scale.
7. Hovering a sign marker shows its text (all non-empty lines) as today.
8. Renders with no signs, or with malformed block-entity records, succeed
   without errors; malformed records are skipped.
9. Tile pixels are unchanged by this feature; render time increase is
   negligible.
10. `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`
    pass; CI stays green.

## Verification Plan

- Synthetic-data tests: a synthetic world DB containing signs at multiple Y
  levels (below surface range, above surface range, within it) verifies the
  overlay contains all of them for the surface layer.
- Tests for empty and malformed block-entity records producing valid,
  empty-where-appropriate overlays.
- Web client test or manual check: toggle button exists, toggling hides/shows
  markers, state persists on reload.
- Manual demo: render a real world with signs at varied Y levels
  (`cargo run -- render <WORLD> --serve --open`), confirm all signs visible
  on surface view, toggle behavior, and dimension switching.
- Confirm existing tile outputs unchanged (no pixel diff on tiles).

## Rollback Plan

- Revert the branch. The sign toggle defaults to on; without the new code the
  web client simply ignores any stale overlay data (feature detection in
  `app.js`), so no migration is needed.

## Open Questions

- Should the sign overlay merge into the existing `markers.json` (additive
  shape change) or a separate file? Prefer extending `markers.json` unless
  file-size concerns arise.
- Whether deeply large worlds need pagination/lazy loading of the overlay;
  assume a single JSON file is fine initially.
