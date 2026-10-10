# TBC-map-markers: Interactive map markers for players and signs

## Goal

When viewing the rendered map in the web client, a user can hover (or mouse
over) a marker on the map to see where players are located and where signs
are placed, including the sign's full text content and player detail (name
and dimension/position). Markers are provided by the renderer as data the
web client overlays on the Leaflet map.

## Context

AmberMap already renders terrain tiles per dimension. The world's LevelDB
database contains the data needed for markers, but it is currently not
extracted:

- **Signs** live in block-entity records under the existing `BlockEntity`
  key tag (0x31) as NBT payloads (`id: "Sign"` / `"HangingSign"` with text
  fields and block position).
- **Player locations** live in the database under `~local_player` (single
  player worlds) and `player_...` keys (multiplayer), also NBT, including
  position and dimension.

The web client (`web_client/js/app.js`, no-build vanilla JS + Leaflet)
fetches `/api/status` and tile URLs from the embedded server. Adding
markers needs both a data source (embedded at render time or served from
an API endpoint) and Leaflet hover popups.

## Scope

### In scope

- Extract sign block entities during render: parse `BlockEntity` NBT records
  for sign types and record block position plus all four text lines.
- Extract player positions during render: parse `~local_player` and
  `player_*` NBT records for position, dimension, and player name.
- Emit markers as JSON per dimension (a `markers.json` alongside the
  existing per-dimension `metadata.json`, or equivalent).
- Serve the marker data from the embedded server (static file or
  `/api/markers?dim=`).
- Web client: draw markers on the Leaflet map per active dimension and show
  a hover tooltip/popup with the sign text or player detail.
- Marker rendering must not slow the tile pipeline noticeably (markers are
  a single lightweight pass).

### Out of scope

- Live/real-time player tracking (data is a snapshot at render time).
- Click-through navigation, editing signs, or any write path.
- Markers for other entity types (item frames, armor stands, mobs).
- Tile image changes; markers are a separate overlay layer.

## Constraints & Assumptions

- World DB format varies by Bedrock version; NBT tag layouts for signs and
  players may differ between versions. Parse leniently and skip malformed
  records silently (consistent with existing lenient decompression).
- Some worlds have many signs; marker JSON should stay small (position +
  text only, no full NBT).
- CI enforces fmt/clippy `-D warnings`/tests; tests are synthetic-data based.
- The web client must keep working with the existing `/api/status` contract
  (additive changes only).

## Decisions

- Markers are rendered into a JSON file at map-render time (snapshot
  semantics), not queried live from LevelDB by the web server.

## Acceptance Criteria

1. After a render, a markers data file exists per dimension containing every
   sign found (position + 4 text lines) and every player found (name,
   position, dimension).
2. Worlds with no signs or no players render normally with an empty marker
   list, and no errors.
3. The web client shows sign markers and player markers on the map for the
   active dimension; hovering a marker displays the sign text or player
   detail in a tooltip/popup.
4. Switching dimensions in the viewer switches the marker set shown.
5. Marker extraction does not corrupt or slow tile output: tiles are
   pixel-identical to a render without marker extraction, and render time
   increase is negligible.
6. Malformed/unparseable block-entity or player records are skipped without
   failing the render.
7. `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`
   pass; CI stays green.

## Verification Plan

- Synthetic NBT tests for sign and player record parsing (success, unknown
  id, malformed payload cases).
- Render a real world with signs/players and validate the markers JSON
  contents against expected count and sample text.
- Manual demo: `cargo run -- render <WORLD> --serve --open`, hover markers
  in browser, switch dimensions.
- Existing tile pixel-diff to confirm tile output unchanged.

## Rollback Plan

- Revert the branch. Marker JSON/endpoint is additive; without the new
  code the web client simply ignores any stale markers file (feature
  detection in app.js), so no migration is needed.

## Open Questions

- Player name availability: single-player worlds may not store a gamertag
  in `~local_player`; fall back to a generic label ("Player") — confirm
  during implementation.
- Whether hanging signs and double-sided signs have distinct NBT text
  fields to map (front/back) — decide when implementing the parser.
- Marker visual design (icon vs dot) is a follow-up; initial delivery can
  use default Leaflet markers.