# AGENTS.md

Guidance for AI agents working in this repository.

## What this is

AmberMap is a Minecraft: Bedrock Edition world map renderer written in Rust. It reads a Bedrock world's LevelDB database directly, decodes subchunks, ray-marches voxels top-down with alpha compositing, applies hillshading, and emits a WebP/PNG tile pyramid. The binary embeds a Leaflet.js web viewer (via `rust-embed`) and serves it with `tiny_http` — no external runtime dependencies.

## Commands

```bash
cargo build --release        # requires Rust 1.75+
cargo test                   # integration tests live in tests/
cargo fmt --check            # CI enforces formatting
cargo clippy -- -D warnings  # CI fails on any warning
```

CI (`.github/workflows/ci.yml`) runs fmt, check, clippy `-D warnings`, and tests on Linux, Windows, and macOS — so keep code warning-free and rustfmt-clean. Releases are built automatically on GitHub release publish (no manual packaging).

Quick end-to-end sanity run (needs a Bedrock world folder):

```bash
cargo run -- render <WORLD_DIR> --serve --open
```

## Architecture / data flow

Pipeline stages, each a top-level module under `src/`:

1. **`leveldb/`** — opens the world `db/` folder (`WorldDb::open`), enumerates subchunk keys (`scan_chunks`), fetches and decompresses payloads into `ChunkData` (chunk pos, dimension, version, `Vec<(layer, SubChunk)>`). Compression (Zstd/Snappy/raw-deflate) is detected in `decompressor.rs`.
2. **`subchunk/`** — pure binary decoding: `SubChunkParser` reads palette + bit-packed block indices (storage versions 8/9, 1–16 bits/voxel) into a 16³ `SubChunk` block array. No I/O.
3. **`raster/`** — renders one chunk to 16×16 RGBA: `traverse_chunk` does a top-down column scan with front-to-back alpha compositing, `ColorMap` maps block names to colors, `hillshade.rs` applies 2.5D NW-lit shading. Entry point: `rasterize_chunk`.
4. **`tiler/`** — `pipeline.rs::render_world_map` orchestrates everything: rayon-parallel 256×256 base tiles (16×16 chunks blitted into `TileBuffer`), then iterative 2×2 downsampling to build the zoom pyramid, writes tiles + `metadata.json`.
5. **`web/`** — `start_server` serves tiles at `/tiles/{dim}/{z}/{x}/{y}.{ext}` and `/api/status`; `embedded.rs` embeds `web_client/` into the binary via `#[derive(RustEmbed)]`.

The web client (`web_client/js/app.js`) is a no-build-step vanilla JS Leaflet SPA. It fetches `/api/status` for center/bounds/max_zoom/format and constructs tile URLs accordingly. If you change tile layout or `metadata.json` shape, update `app.js` to match.

Public API is re-exported broadly: each `mod.rs` does `pub use` of its modules' items, and `lib.rs` re-exports the whole surface. Submodules are also `pub`, so both `ambermap::raster::rasterize_chunk` and `ambermap::raster::chunk_raster::rasterize_chunk` work. When adding a public item, wire it into the module's `mod.rs` re-exports (and consider `lib.rs`) — tests and the README examples use the re-exported paths.

## Conventions

- Errors: hand-rolled `thiserror` enum `AmberError` plus crate alias `pub type Result<T> = std::result::Result<T, AmberError>` (see `src/core/error.rs`). Use structured variants with context fields rather than stringly errors; no `anyhow`.
- Every source file starts with the same ASCII-art license header block (copy from an existing file).
- Tests are integration-style in `tests/*.rs` (one file per module), built on **synthetic data generators**, not binary fixtures — e.g. `raster_tests.rs` builds palettes and 4096-block `SubChunk`s programmatically. Follow that pattern; don't add fixture files.
- CLI is clap-derive with subcommands (`render`, `serve`, `inspect`, `render-chunk`) in `src/main.rs`.

## Gotchas

- **LevelDB key format** (`src/leveldb/keys.rs`): x(i32 LE) + z(i32 LE) + tag byte, with an optional 4-byte dimension id inserted between coords and tag. Overworld keys are 9/10 bytes; other dimensions 13/14 bytes; anything else is rejected. The dimension byte order and optional-y-byte layout are easy to get wrong.
- **Lenient decompression is intentional**: `decompressor.rs` dispatches on the Zstd magic, tries Snappy, and otherwise returns the raw bytes as `Ok`. Unknown payloads silently pass through — don't "fix" this into an error.
- **Nether roof skipping**: the ray traverser skips `minecraft:bedrock` encountered before any terrain *only* when `ChunkData.dimension == Nether` (the nether's solid bedrock ceiling would otherwise render as an opaque roof). Overworld bedrock still renders.
- **Per-dimension metadata**: each render writes `output/<dim>/metadata.json` (center/bounds/max_zoom/format); the root `output/metadata.json` is a compatibility copy of whichever dimension rendered last. The web viewer must use `/api/status?dim=<name>` (`status_metadata` in `src/web/server.rs`), never the root file, when switching dimensions.
- **Memory model**: `WorldDb` loads the entire LevelDB into a `HashMap` and `render_world_map` keeps all base `TileBuffer`s (256 KB each) in memory for pyramid downsampling — large worlds are memory-heavy. `--low-memory` opts into: pruning the DB map to chunk-relevant keys (`WorldDb::prune_to_chunk_keys`) and building the pyramid by re-reading saved tiles from disk (`build_pyramid_from_disk`), at the cost of speed and slight WebP lossiness in upper zoom layers. Failed chunk decodes are skipped silently (`if let Ok(...)`).
- **`render_world_map` wipes the output directory** (`remove_dir_all`) on every run. Don't assume the output dir survives a re-render, and don't point `--output` at anything you care about.
- Release profile uses `panic = "abort"` and LTO; keep code compatible (e.g. avoid relying on unwinding).
