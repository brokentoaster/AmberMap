# TBC-map-markers: Interactive map markers for players and signs — Plan

## Spec

- Spec: docs/workplans/TBC-map-markers-Spec.md
- Jira: n/a

## Overview

- Outcome: hovering map markers in the web viewer shows sign text and
  player detail for the active dimension, from a per-dimension markers
  JSON snapshot produced at render time.
- Approach: one new `markers` pipeline stage — a `markers/` module that
  parses sign block-entity NBT (tag 0x31) and player NBT (`~local_player`,
  `player_*`) from the already-in-memory `WorldDb`, emitting
  `output/<dim>/markers.json`. Serve it and wire Leaflet hover popups in
  `app.js` with feature detection (missing/empty file = no markers).

## Tasks

- [x] Task 1: Parse sign block entities into marker structs
  - Notes: Add a `markers` module with `Marker` types (SignMarker with
    position + 4 text lines; PlayerMarker with name, position, dimension)
    and a sign NBT parser reading `BlockEntity` keys for `Sign` /
    `HangingSign` ids. Reuse the existing `nbt` module; skip malformed
    records silently. Wire into `mod.rs`/`lib.rs` re-exports.
  - Touchpoints: new `src/markers/` module, `src/leveldb/reader.rs`
    (block-entity key access), `src/nbt/`
  - Done when: synthetic NBT tests cover valid sign, unknown id, and
    malformed payload; `cargo test` green.

- [x] Task 2: Parse player records into player markers
  - Notes: Parse `~local_player` and `player_*` keys from the entries map
    for position, dimension, and name if present; fall back to a generic
    "Player" label when a name is unavailable (spec Open Question).
  - Touchpoints: `src/markers/`, `src/leveldb/reader.rs`
  - Done when: synthetic player NBT tests pass; marker structs complete.

- [x] Task 3: Emit markers.json during render and serve it
  - Notes: In `render_world_map_with_db`, after tile rendering, run a
    single-threaded marker pass over the db and write
    `output/<dim>/markers.json`. Serve it via the existing embedded server
    (static file or `/api/markers?dim=` route) consistent with how
    `metadata.json` is served. Keep the pass lightweight; empty list when
    none found.
  - Touchpoints: `src/tiler/pipeline.rs`, `src/web/server.rs`
  - Done when: a real-world render produces valid markers JSON with
    expected signs/players; tiles remain pixel-identical (spec AC 1, 5, 6).

- [x] Task 4: Web client hover markers per dimension
  - Notes: In `app.js`, feature-detect marker data (fetch markers JSON,
    tolerate 404/empty), draw Leaflet markers for the active dimension,
    bind hover tooltip/popup showing sign text or player detail, and swap
    the marker set when the dimension changes. No-build vanilla JS.
  - Touchpoints: `web_client/js/app.js`
  - Done when: manual demo (`--serve --open`) shows hover popups and
    dimension switching updates markers (spec AC 3, 4).

- [x] Task 5: Final verification against spec AC
  - Notes: Run fmt, clippy `-D warnings`, full test suite; pixel-diff a
    render against a pre-change baseline; demo the viewer end to end.
  - Done when: spec AC 1-7 all verified.

## Dependencies & prerequisites

- Task 2 after Task 1 (shares marker types and module skeleton).
- Task 3 after Tasks 1-2 (needs the parsers).
- Task 4 after Task 3 (needs the served data format).
- Prerequisite: a world with signs (and ideally a player save) for manual
  verification, e.g. the "Bedrock level" test world.

## Verification

- See spec Acceptance Criteria and Verification Plan
- Extra: keep a pre-change tile output snapshot and pixel-diff after
  Tasks 3-4 to prove tiles unchanged.

## Risks & mitigations

- Risk: sign/player NBT layouts differ across Bedrock versions → Mitigate:
  lenient parsing with silent skip (spec Constraint), synthetic tests for
  the common v1 layout, manual check on the test world.
- Risk: `~local_player` may lack a readable name → Mitigate: generic
  label fallback, decided in Task 2.
- Open: whether double-sided/hanging signs use distinct text fields;
  resolve in Task 1 while implementing the parser.

## Out of scope

- Live tracking, marker editing, other entity types, tile changes (see
  spec Out of scope).

## Rollback

- See spec Rollback Plan (revert branch; additive data + feature-detected
  client).