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

    std::fs::write(
        base.join("overworld").join("metadata.json"),
        r#"{"center_x":10,"max_zoom":4}"#,
    )
    .unwrap();
    std::fs::write(
        base.join("nether").join("metadata.json"),
        r#"{"center_x":99,"max_zoom":2}"#,
    )
    .unwrap();

    assert!(status_metadata(&base, "overworld").contains(r#""center_x":10"#));
    assert!(status_metadata(&base, "nether").contains(r#""center_x":99"#));

    let fallback = status_metadata(&base, "the_end");
    assert!(fallback.contains(r#""rendered":false"#));

    let no_meta_at_all = status_metadata(&base, "overworld");
    assert!(no_meta_at_all.contains("center_x"));

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
