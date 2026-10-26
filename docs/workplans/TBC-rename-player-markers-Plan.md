# TBC-rename-player-markers: Assign player names by clicking markers — Plan

## Spec

- Spec: docs/workplans/TBC-rename-player-markers-Spec.md
- Jira: n/a

## Overview

- Outcome: clicking a player marker in the web viewer opens a popup with a
  name input; saving persists to `player_names.json` in the output dir and
  the name shows immediately, surviving re-renders.
- Approach: add overrides-file load/merge to the `markers` module (reusing
  the `player_name_for_key` precedence chain), a POST `/api/players/rename`
  endpoint in the existing `tiny_http` request loop, a render-time merge
  before `collect_markers` writes `markers.json`, and a small click-popup
  UI in `app.js` that POSTs and updates the tooltip in place.

## Tasks

- [x] Task 1: Load and merge player-name overrides in the markers module
  - Notes: Add `load_player_overrides(output_dir) -> HashMap<String, String>`
    parsing `player_names.json` (flat object, key = key label; empty string
    value = clear), missing/corrupt file → empty map. Extend the name
    resolution to merge: overrides > allowlist > XUID label, applied in
    `collect_markers`. Since `collect_markers` currently receives only one
    map, combine allowlist + overrides into the lookup map before the call
    (overrides inserted last so they win; skip empty values so clearing
    falls through to the allowlist/XUID).
  - Touchpoints: `src/markers/mod.rs`
  - Done when: unit tests cover missing file, corrupt JSON, override
    precedence, and empty-string clearing (spec AC 4, 5); `cargo test` green.

- [x] Task 2: Merge overrides into rendered markers.json
  - Notes: In `render_world_map_with_db`, after tile work, load overrides
    from `options.output_dir` and merge with the allowlist map before
    calling `collect_markers` (overrides must win). The output dir is
    recreated by the pipeline, so load overrides *before* `remove_dir_all`
    if the wipe happens after markers collection begins — check the wipe
    ordering while implementing and load first if needed.
  - Touchpoints: `src/tiler/pipeline.rs`
  - Done when: a render with a pre-existing `player_names.json` emits
    `markers.json` with override names (spec AC 4); tiles unchanged.

- [x] Task 3: POST /api/players/rename endpoint in the web server
  - Notes: In the `start_server` request loop, handle `path ==
    "/api/players/rename"` with method POST (reject non-POST with 405).
    Read the request body (JSON `{"key": "...", "name": "..."}`; no serde
    on the server side yet — a tiny hand-rolled parse or reuse serde_json,
    now a dependency), validate the key is non-empty and matches
    `player_*`/`~local_player` shape, then read-modify-write
    `player_names.json` in `config.tiles_dir`: empty name removes the key,
    otherwise upsert. Return 200 with `{"ok":true}`; 400 for malformed
    body/unknown key shape. Escape/serialize via serde_json.
  - Touchpoints: `src/web/server.rs`
  - Done when: server tests cover valid save, clear, malformed body, and
    non-POST (spec AC 2, 3, 7).

- [x] Task 4: Click-to-rename popup in the web client
  - Notes: In `app.js`, change player markers from circleMarker hover-only
    to click-openable: on click open a Leaflet popup containing the current
    name, a text input, and a Save button. Save POSTs to
    `/api/players/rename`, then updates the stored player entry and tooltip
    content in place (no reload). Empty input = clear override. Escape all
    name strings when injecting into DOM/tooltip (no innerHTML with raw
    names). Feature-detect: if the POST fails (older binary), show the
    popup without Save or a small error note.
  - Touchpoints: `web_client/js/app.js`
  - Done when: manual demo (`--serve --open`) — click marker, rename,
    tooltip updates immediately, page reload keeps the name (spec AC 1-3).

- [x] Task 5: End-to-end persistence and final verification
  - Notes: Manual flow: rename → re-render same output dir → name persists
    (merge at render time) and wins over allowlist. Security spot-check:
    name with `"` `\` `<` `>` survives round-trip and renders as text.
    Run fmt, clippy `-D warnings`, full test suite.
  - Done when: spec AC 1-8 all verified; CI-equivalent checks green.

## Dependencies & prerequisites

- Task 2 after Task 1 (needs the overrides loader/merge).
- Task 4 after Task 3 (needs the endpoint contract).
- Task 5 after all tasks.
- Prerequisite: a world with at least one player save for the manual demo.

## Verification

- See spec Acceptance Criteria and Verification Plan
- Extra: check the output-dir wipe ordering during Task 2 — if
  `player_names.json` is deleted by `remove_dir_all`, persistence breaks
  silently; the load-before-wipe ordering in Task 2's notes must be proven
  by a test or explicit manual check.

## Risks & mitigations

- Risk: `remove_dir_all` on the output dir wipes `player_names.json` on
  re-render → Mitigate: load overrides before the wipe (Task 2 notes) and
  rewrite the file at render time.
- Risk: hand-rolled JSON body parsing on the server accepts ambiguous
  payloads → Mitigate: use serde_json for the request body too; reject on
  parse error.
- Open: whether the `local_player` label is renamable (spec Open Question)
  — resolve in Task 3's key validation.

## Out of scope

- Writing to the world's `allowlist.json`, sign renaming, live tracking,
  auth, bulk/CLI renaming (see spec Out of scope).

## Rollback

- See spec Rollback Plan (revert branch; `player_names.json` is additive
  and ignored by the previous code).
