/*
 *     _             _                __  __
 *    / \   _ __ ___ | |__   ___ _ __ |  \/  | __ _ _ __
 *   / _ \ | '_ ` _ \| '_ \ / _ \ '__|| |\/| |/ _` | '_ \
 *  / ___ \| | | | | | |_) |  __/ |   | |  | | (_| | |_) |
 * /_/   \_\_| |_| |_|_.__/ \___|_|   |_|  |_|\__,_| .__/
 *                                                 |_|
 *
 * AmberMap - High-Performance Bedrock World Map Renderer
 * https://github.com/Amber-PM/AmberMap
 *
 * Copyright (c) 2026 Amber-PM
 * Licensed under Apache-2.0 or MIT
 */

use ambermap::web::{get_embedded_asset, resolve_mime, status_metadata};

#[test]
fn test_status_metadata_per_dimension() {
    let base = std::env::temp_dir().join(format!("ambermap_status_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("overworld")).unwrap();
    std::fs::create_dir_all(base.join("nether")).unwrap();
    std::fs::create_dir_all(base.join("overworld").join("y0-63")).unwrap();

    std::fs::write(
        base.join("overworld").join("metadata.json"),
        r#"{"center_x":10,"max_zoom":4,"layer_id":"surface","layer_label":"Surface","y_min":-64,"y_max":319}"#,
    )
    .unwrap();
    std::fs::write(
        base.join("overworld")
            .join("y0-63")
            .join("metadata.json"),
        r#"{"center_x":10,"max_zoom":4,"layer_id":"y0-63","layer_label":"Y 0..63","y_min":0,"y_max":63}"#,
    )
    .unwrap();
    std::fs::write(
        base.join("nether").join("metadata.json"),
        r#"{"center_x":99,"max_zoom":2,"layer_id":"surface","y_min":0,"y_max":255}"#,
    )
    .unwrap();

    // dimension status gains a layers array listing surface + discovered layers
    let overworld = status_metadata(&base, "overworld", "");
    assert!(overworld.contains(r#""center_x":10"#));
    assert!(overworld.contains(r#""layers":["#));
    assert!(overworld.contains(r#"{"id":"surface","label":"Surface","y_min":-64,"y_max":319}"#));
    assert!(overworld.contains(r#"{"id":"y0-63","label":"Y 0..63","y_min":0,"y_max":63}"#));
    // must remain valid JSON: layers array closed once, single trailing brace
    assert!(overworld.ends_with(r#"]}"#));
    assert!(!overworld.contains("}}}}"));

    // nether has no extra layer dirs: surface only
    let nether = status_metadata(&base, "nether", "");
    assert!(nether.contains(r#""center_x":99"#));
    assert!(nether.contains(r#"{"id":"surface","label":"Surface","y_min":0,"y_max":255}"#));

    // per-layer metadata
    let slice = status_metadata(&base, "overworld", "y0-63");
    assert!(slice.contains(r#""layer_id":"y0-63""#));
    assert!(slice.contains(r#""y_max":63"#));

    // unknown layer returns rendered:false stub
    let missing = status_metadata(&base, "overworld", "y999-1000");
    assert!(missing.contains(r#""rendered":false"#));

    // surface layer resolves like the plain dimension metadata
    let surface = status_metadata(&base, "overworld", "surface");
    assert!(surface.contains(r#""center_x":10"#));

    let fallback = status_metadata(&base, "the_end", "");
    assert!(fallback.contains(r#""rendered":false"#));

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_mime_type_resolution() {
    assert_eq!(resolve_mime("index.html"), "text/html; charset=utf-8");
    assert_eq!(resolve_mime("style.css"), "text/css; charset=utf-8");
    assert_eq!(
        resolve_mime("app.js"),
        "application/javascript; charset=utf-8"
    );
    assert_eq!(resolve_mime("0.webp"), "image/webp");
    assert_eq!(resolve_mime("0.png"), "image/png");
    assert_eq!(resolve_mime("data.json"), "application/json");
    assert_eq!(resolve_mime("icon.svg"), "image/svg+xml");
    assert_eq!(resolve_mime("7.blocks.bin"), "application/octet-stream");
    assert_eq!(resolve_mime("unknown.bin"), "application/octet-stream");
}

#[test]
fn test_embedded_assets_retrieval() {
    let index = get_embedded_asset("index.html");
    assert!(index.is_some());
    let (data, mime) = index.unwrap();
    assert_eq!(mime, "text/html; charset=utf-8");
    let html_str = std::str::from_utf8(&data).unwrap();
    assert!(html_str.contains("AmberMap"));
    assert!(html_str.contains("map-viewport"));

    let css = get_embedded_asset("css/map.css");
    assert!(css.is_some());
    let (css_data, css_mime) = css.unwrap();
    assert_eq!(css_mime, "text/css; charset=utf-8");
    let css_str = std::str::from_utf8(&css_data).unwrap();
    assert!(css_str.contains(".hud-overlay"));

    let js = get_embedded_asset("js/app.js");
    assert!(js.is_some());
    let (js_data, js_mime) = js.unwrap();
    assert_eq!(js_mime, "application/javascript; charset=utf-8");
    let js_str = std::str::from_utf8(&js_data).unwrap();
    assert!(js_str.contains("AmberTileLayer"));

    let none = get_embedded_asset("does_not_exist.xyz");
    assert!(none.is_none());
}
