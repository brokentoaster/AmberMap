# TBC-rename-player-markers: Assign player names by clicking markers in the web viewer

## Goal

When viewing the rendered map in the web client, a user can click a player
marker and assign a display name to that player. The name is persisted on the
server and appears on the marker (tooltip and any future display) after a
page reload, so manually assigned names survive re-renders and browser
restarts.

## Context

Player markers already exist (`TBC-map-markers`). Names currently come from
the world's `allowlist.json` (XUID → gamertag) at render time; unmapped
players fall back to their XUID. Bedrock's LevelDB never stores gamertags, so
names assigned in the UI cannot be derived from world data — they must be
stored separately. The web server (`src/web/server.rs`, `tiny_http`) serves
the render output directory and shares its filesystem, making the output dir
the natural persistence location.

## Scope

### In scope

- Web client: clicking a player marker opens a Leaflet popup with a text
  input and Save button; empty input removes the assigned name (falls back
  to XUID/allowlist name).
- Server: a POST endpoint (e.g. `/api/players/rename`) accepting the
  player's key label (XUID, `server_<UUID>`, or `local_player`) and the new
  name, writing an overrides file `player_names.json` in the output
  directory.
- Rendering: when generating `markers.json`, merge the output-dir overrides
  file over the `allowlist.json` names, so re-renders keep UI-assigned names
  and take precedence over allowlist entries.
- Serving: the live server also merges `player_names.json` into the markers
  data it serves, so names assigned without a re-render show up immediately.
- XSS safety: names are user-supplied; all rendering into tooltips/popups
  and JSON must escape appropriately.

### Out of scope

- Writing to the world's `allowlist.json` (read-only world access).
- Renaming signs or other marker types.
- Live/real-time tracking; markers remain a render-time snapshot.
- Authentication or multi-user concurrency (single-user local tool).
- Editing names in bulk or any CLI renaming command.

## Constraints & Assumptions

- The server only knows the output directory (`serve --dir`), so persistence
  must live in the output dir; `render` knows both the world and output dirs
  and can merge at render time.
- `player_names.json` format: flat JSON object mapping player key label
  (XUID string or `local_player`) to display name; same key space as
  `player_name_for_key` lookups.
- Empty name string in the overrides file means "clear this override".
- Corrupt or missing overrides file is ignored silently (lenient, like the
  allowlist loader).
- The web client is no-build vanilla JS; popup UI must stay small.
- CI enforces fmt, clippy `-D warnings`, and tests on synthetic data.

## Decisions

- Persistence is an output-dir overrides file merged over the allowlist, not
  edits to the world's `allowlist.json`.
- Interaction is an inline Leaflet popup with a text input and Save button
  (not `prompt()`).
- Overrides take precedence: overrides file > allowlist.json > XUID label.

## Acceptance Criteria

1. Clicking a player marker in the web viewer opens a popup containing the
   current name and a text input with a Save button.
2. Saving a non-empty name persists it to `player_names.json` in the output
   directory and the marker tooltip shows the new name immediately, without
   a re-render.
3. Saving an empty name removes the override; the marker falls back to the
   allowlist name or XUID.
4. After a full re-render, previously assigned names still appear on the
   markers (merge at render time), and override names win over allowlist
   names for the same XUID.
5. A missing or corrupt `player_names.json` does not fail rendering or
   serving; markers fall back to existing name resolution.
6. Names containing quotes, backslashes, `<`/`>` or control characters are
   stored, served, and displayed safely (escaped JSON, no HTML injection in
   tooltips/popups).
7. `POST` with an unknown key label or malformed body returns an error
   status and changes nothing.
8. `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`
   pass; CI stays green.

## Verification Plan

- Unit tests for the overrides loader/merger: missing file, corrupt JSON,
  override precedence over allowlist, empty-string clearing.
- Server test for the rename endpoint: valid save, empty save (clear),
  malformed body, unknown key.
- Render test: overrides file present in output dir → merged into
  `markers.json`.
- Manual demo: `cargo run -- render <WORLD> --serve --open`, click a player
  marker, rename, reload, re-render and confirm persistence.

## Rollback Plan

- Revert the branch. `player_names.json` is additive data; the previous
  code simply ignores it, and existing markers continue to resolve names
  from `allowlist.json` / XUID as today.

## Open Questions

- Resolved: the `local_player` label is renamable via the same popup (the
  rename endpoint accepts it), and `server_<UUID>` player records are
  collected and renamable with their UUID as the fallback label.
- Whether assigned names should also feed any future marker display change
  (icons, labels on the map) — follow-up feature.
