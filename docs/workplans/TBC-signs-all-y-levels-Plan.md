# TBC-signs-all-y-levels: Surface-visible sign markers with web UI toggle — Plan

## Spec

- Spec: docs/workplans/TBC-signs-all-y-levels-Spec.md
- Jira: n/a

## Overview

- Outcome: every sign in a dimension appears on the surface (and Y-slice)
  views regardless of its Y level, and a new header toggle (like the
  Players toggle) shows/hides sign markers with state persisted across
  reloads.
- Approach: signs are already collected via a full block-entity sweep
  (`collect_markers`); the remaining work is to guarantee the sweep is
  emitted per-dimension rather than per-layer, serve it consistently, and
  add the client-side toggle. Extend the existing `markers.json` shape
  (spec Open Question resolved in favor of extending, per spec preference).

## Tasks

- [x] Task 1: Guarantee per-dimension sign emission for every render
  - Notes: `pipeline.rs` currently writes `markers.json` to a layer
    subdirectory when a layer is set and skips writing when
    `markers.is_empty()` (around lines 244-250). Always write
    `markers.json` (even empty) to the dimension root, and additionally
    write/copy into any layer subdirectory so layer views resolve it.
    Verify `collect_markers` sweeps all block entities for the dimension
    (no Y or layer filtering).
  - Touchpoints: `src/tiler/pipeline.rs` (emission), `src/markers/mod.rs`
    (verify sweep scope)
  - Done when: a synthetic render with a layer set produces a
    `markers.json` in both the dimension dir and layer dir; empty
    dimensions produce a valid empty file (spec AC 1, 2, 8).

- [x] Task 2: Serve markers.json consistently for surface and layer views
  - Notes: `status_metadata` resolves per-layer metadata; ensure the
    server serves `markers.json` for the active dimension/layer
    combination (surface = dimension root, layer = layer dir), with
    feature detection (404/empty tolerated). Additive only; `/api/status`
    contract unchanged.
  - Touchpoints: `src/web/server.rs`
  - Done when: the web client can fetch markers for both surface and
    Y-slice layer views (spec AC 3).

- [x] Task 3: Web client sign toggle button and persistence
  - Notes: Add a header action button mirroring `btn-toggle-players`
    (class, icon style, `classList.toggle('active')`). Persist sign
    visibility in `localStorage` (default on). Toggling removes/re-adds
    the sign markers client-side without refetching data.
  - Touchpoints: `web_client/index.html`, `web_client/js/app.js`
  - Done when: toggle exists, hides/shows markers without refetch, and
    state survives page reload (spec AC 4, 5).

- [x] Task 4: Render sign markers per dimension/layer with scaling
  - Notes: On dimension or layer switch, re-render the sign overlay for
    the new view; apply `coordScale()` to sign positions so nether signs
    display at overworld-equivalent scale. Hover popup shows all
    non-empty lines (existing behavior, verify only).
  - Touchpoints: `web_client/js/app.js`
  - Done when: dimension/layer switching updates the overlay and nether
    signs land at the correct scaled coordinates (spec AC 6, 7).

- [x] Task 5: Final verification against spec AC
  - Notes: Run fmt, clippy `-D warnings`, full test suite; pixel-diff
    tiles against a pre-change baseline; manual demo
    (`cargo run -- render <WORLD> --serve --open`) with signs at varied Y
    levels.
  - Done when: spec AC 1-10 all verified.

## Dependencies & prerequisites

- Task 2 after Task 1 (needs the emitted files in the right locations).
- Task 4 after Task 3 (toggle wiring precedes re-render behavior).
- Task 5 last (full validation).
- Prerequisite: a world with signs at varied Y levels (and ideally in
  multiple dimensions) for manual verification.

## Verification

- See spec Acceptance Criteria and Verification Plan
- Extra: keep a pre-change tile output snapshot and pixel-diff after
  Tasks 1-4 to prove tiles unchanged (spec AC 9).

## Risks & mitigations

- Risk: layer-directory emission duplicates marker data (dimension root +
  layer copy) → Mitigate: measure `markers.json` size on the test world;
  if large, prefer dimension-root-only + server resolution instead of
  duplication.
- Open: whether any existing Y/layer filtering exists in
  `collect_markers` to remove; resolve in Task 1 during verification.

## Out of scope

- Live tracking, sign editing, other block-entity types, tile pixel
  changes, player-marker behavior changes (see spec Out of scope).

## Rollback

- See spec Rollback Plan (revert branch; feature-detected client, no
  migration).
