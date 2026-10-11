# TBC-nether-coords: Nether coordinate alignment and stable viewport — Plan

## Spec

- Spec: `docs/workplans/TBC-nether-coords-Spec.md`
- Jira: n/a

## Overview

- Outcome: the Nether map renders at overworld-equivalent scale with dual
  nether/overworld coordinates in HUD and player tooltips, and switching
  dimension or layer no longer resets the viewport or zoom.
- Approach: all changes are in the web client (`web_client/js/app.js`).
  Apply an 8x horizontal coordinate transformation when
  `currentDimension === 'nether'` (Leaflet `CRS.Simple` transformation with
  scale 8, matching the existing `scaleFactor` pattern), keep
  `/api/status` metadata in native nether blocks and multiply center/bounds
  by 8 client-side before fitting. Marker positions and tooltips apply the
  same transform. Viewport preservation is done by capturing
  `map.getCenter()`/`getZoom()` before a switch and restoring after the new
  tile layer is ready instead of `fitBounds`.
- No renderer (Rust) changes: markers.json and tiles are unchanged.

## Tasks

- [x] Task 1: Nether map at overworld scale with dual HUD coordinates
  - Notes: extend the existing `AmberCRS` transformation with an 8x scale
    when the active dimension is `nether`; scale `center_x/center_z` and
    block bounds from `/api/status` by 8 before `fitBounds`/`setView`.
    `updateCoords` reports the on-screen (overworld-equivalent) coordinates
    plus the native nether coordinates (`overworld / 8`), labeled per
    dimension (e.g. `X/Z (nether)` alongside `X/Z`).
  - Touchpoints: `web_client/js/app.js` (`initMap`, `loadDimension`,
    `updateCoords`)
  - Done when: spec AC 1 and 6 — hovering the same portal on both maps
    shows matching overworld coords; pre-change worlds still render.
- [x] Task 2: Markers positioned at scaled coordinates with dual-coordinate
  tooltips
  - Notes: in `renderMarkers`, map marker nether block coords through the
    same 8x transform as Task 1 (one shared helper for both CRS and
    markers). Player tooltip in the Nether shows nether X/Y/Z and
    overworld X/Y/Z (x/z * 8, y + 64). Sign markers in the Nether use
    scaled positions. Other dimensions keep current behavior.
  - Touchpoints: `web_client/js/app.js` (`renderMarkers`)
  - Done when: spec AC 2 and 3 — a nether player appears at the
    overworld-equivalent spot with both coordinate sets in the tooltip.
- [x] Task 3: Viewport and zoom preserved across dimension and layer switches
  - Notes: in the dimension and layer click handlers (and `loadDimension`),
    capture `map.getCenter()` and `map.getZoom()` before switching and
    restore after the new tile layer is added, instead of the unconditional
    `fitBounds`/`setView` in `initMap`. Keep initial load fitting bounds.
    Handle the nether 8x mapping when carrying center between dimensions of
    different scale. Honor spec Open Question: keep viewport only after the
    user has interacted; otherwise fall back to fit-bounds.
  - Touchpoints: `web_client/js/app.js` (`loadDimension`, `initMap`, click
    handlers)
  - Done when: spec AC 4 and 5 — pan/zoom somewhere distinctive, switch
    dimension and layer, viewport unchanged.
- [x] Task 4: Final verification
  - Notes: run `cargo fmt --check`, `cargo clippy -- -D warnings`,
    `cargo test` (should be untouched/green), and the manual demo in the
    spec Verification Plan.
  - Done when: spec AC 7 and Verification Plan demo pass.

## Dependencies & prerequisites

- Task 2 depends on Task 1's shared coordinate-transform helper.
- Task 3 depends on Task 1 (scale-aware center restoration).
- Task 4 after all others.

## Risks & mitigations

- Risk: Leaflet `CRS.Simple` scaling by 8 interacts with the existing
  `scaleFactor = 1/2^maxZoom` transformation and `maxNativeZoom`, changing
  how zoom maps to tile resolution for the Nether. → Mitigate: apply the 8
  as an additional multiplier in the same `L.Transformation`, and verify
  tile alignment at each zoom level during the Task 1 demo.
- Risk: restoring viewport across dimensions with different bounds may
  restore a center outside the new map's useful area. → Mitigate: clamp the
  restored center into the target dimension's bounds when available.
- Open: none beyond spec Open Questions.

## Out of scope

- Portal detection/linking, End-dimension scaling (1:1), tile/pyramid or
  renderer changes, live player tracking (see spec Scope).

## Rollback

- See spec Rollback Plan (revert branch; client-only change).
