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

use crate::core::error::{AmberError, Result};
use crate::web::embedded::{get_embedded_asset, resolve_mime};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tiny_http::{Header, Response, Server, StatusCode};

pub struct ServerConfig {
    pub bind_addr: String,
    pub port: u16,
    pub tiles_dir: PathBuf,
    pub auto_open: bool,
    pub tile_format: String,
    pub max_zoom: u8,
}

pub fn status_metadata(tiles_dir: &Path, dim: &str, layer: &str) -> String {
    if !dim.is_empty() {
        let dim_dir = tiles_dir.join(dim);

        if !layer.is_empty() && layer != "surface" {
            let layer_meta = dim_dir.join(layer).join("metadata.json");
            if let Ok(meta) = std::fs::read_to_string(&layer_meta) {
                return meta;
            }
            return format!(
                r#"{{"status":"ok","rendered":false,"dim":"{}","layer_id":"{}"}}"#,
                dim, layer
            );
        }

        let dim_meta = dim_dir.join("metadata.json");
        if let Ok(meta) = std::fs::read_to_string(&dim_meta) {
            let layers = discover_layers(&dim_dir, &meta);
            return inject_layers(&meta, &layers);
        }
    }

    match std::fs::read_to_string(tiles_dir.join("metadata.json")) {
        Ok(meta) => meta,
        Err(_) => r#"{"status":"ok","rendered":false}"#.to_string(),
    }
}

/// Builds the `layers` array for a dimension: the surface entry from the
/// dimension metadata, plus one entry per `<dim>/<layer>/metadata.json`.
fn discover_layers(dim_dir: &Path, surface_meta: &str) -> Vec<String> {
    let mut entries = Vec::new();

    let surface = LayerInfo::from_metadata(surface_meta, "surface", "Surface");
    if let Some(s) = surface {
        entries.push(s.to_json());
    }

    if let Ok(read_dir) = std::fs::read_dir(dim_dir) {
        for entry in read_dir.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let meta_path = entry.path().join("metadata.json");
            let meta = match std::fs::read_to_string(&meta_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if let Some(info) =
                LayerInfo::from_metadata(&meta, &entry.file_name().to_string_lossy(), "")
            {
                entries.push(info.to_json());
            }
        }
    }

    entries
}

struct LayerInfo {
    id: String,
    label: String,
    y_min: i64,
    y_max: i64,
}

impl LayerInfo {
    /// Parses layer id/label/y_min/y_max out of a metadata JSON. Missing
    /// fields fall back to the provided id and a generated label.
    fn from_metadata(meta: &str, default_id: &str, default_label: &str) -> Option<Self> {
        let get = |key: &str| -> Option<String> {
            let needle = format!("\"{}\":", key);
            let start = meta.find(&needle)? + needle.len();
            let rest = &meta[start..];
            let rest = rest.trim_start();
            if let Some(quoted) = rest.strip_prefix('"') {
                let end = quoted.find('"')?;
                Some(quoted[..end].to_string())
            } else {
                let end = rest.find([',', '}']).unwrap_or(rest.len());
                Some(rest[..end].trim().to_string())
            }
        };

        let id = get("layer_id").unwrap_or_else(|| default_id.to_string());
        let label = get("layer_label").unwrap_or_default();
        let label = if label.is_empty() {
            default_label.to_string()
        } else {
            label
        };
        if id.is_empty() {
            return None;
        }
        let y_min = get("y_min").and_then(|v| v.parse().ok()).unwrap_or(0);
        let y_max = get("y_max").and_then(|v| v.parse().ok()).unwrap_or(0);

        Some(Self {
            id,
            label,
            y_min,
            y_max,
        })
    }

    fn to_json(&self) -> String {
        format!(
            r#"{{"id":"{}","label":"{}","y_min":{},"y_max":{}}}"#,
            self.id, self.label, self.y_min, self.y_max
        )
    }
}

fn inject_layers(meta: &str, layers: &[String]) -> String {
    let array = format!(r#""layers":[{}]"#, layers.join(","));
    match meta.rfind('}') {
        Some(pos) => format!("{},{}}}", &meta[..pos], array),
        None => format!("{{{}}}", array),
    }
}

pub fn start_server(config: ServerConfig) -> Result<()> {
    let addr = format!("{}:{}", config.bind_addr, config.port);
    let server = Server::http(&addr).map_err(|e| {
        AmberError::Io(std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            format!("failed to bind http server on {addr}: {e}"),
        ))
    })?;

    let url = format!("http://{}:{}", config.bind_addr, config.port);
    println!("[info] AmberMap Web Server listening on {}", url);
    println!("[info] Serving map tiles from {:?}", config.tiles_dir);
    println!("[info] Press Ctrl+C to stop the server");

    if config.auto_open {
        open_browser(&url);
    }

    for request in server.incoming_requests() {
        let raw_url = request.url().to_string();
        let path = raw_url.split('?').next().unwrap_or("/");

        if path == "/api/status" {
            let query = raw_url.split('?').nth(1).unwrap_or("");
            let param = |name: &str| {
                query
                    .split('&')
                    .find_map(|p| p.strip_prefix(&format!("{}=", name)))
                    .unwrap_or("")
            };
            let json_body = status_metadata(&config.tiles_dir, param("dim"), param("layer"));
            let response = Response::from_string(json_body)
                .with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                )
                .with_header(
                    Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                )
                .with_header(
                    Header::from_bytes(&b"Cache-Control"[..], &b"no-cache, no-store"[..]).unwrap(),
                );
            let _ = request.respond(response);
            continue;
        }

        if let Some(tile_subpath) = path.strip_prefix("/tiles/") {
            let file_path = config.tiles_dir.join(tile_subpath);
            if file_path.is_file() {
                if let Ok(mut file) = File::open(&file_path) {
                    let mut data = Vec::new();
                    if file.read_to_end(&mut data).is_ok() {
                        let mime = resolve_mime(tile_subpath);
                        let response = Response::from_data(data)
                            .with_header(
                                Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap(),
                            )
                            .with_header(
                                Header::from_bytes(
                                    &b"Cache-Control"[..],
                                    &b"public, max-age=3600"[..],
                                )
                                .unwrap(),
                            )
                            .with_header(
                                Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                                    .unwrap(),
                            );
                        let _ = request.respond(response);
                        continue;
                    }
                }
            }

            let not_found = Response::from_string("Tile Not Found")
                .with_status_code(StatusCode(404))
                .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..]).unwrap());
            let _ = request.respond(not_found);
            continue;
        }

        if let Some((data, mime)) = get_embedded_asset(path) {
            let response = Response::from_data(data.into_owned())
                .with_header(Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap())
                .with_header(
                    Header::from_bytes(
                        &b"Cache-Control"[..],
                        &b"no-cache, no-store, must-revalidate"[..],
                    )
                    .unwrap(),
                );
            let _ = request.respond(response);
            continue;
        }

        if let Some((data, mime)) = get_embedded_asset("index.html") {
            let response = Response::from_data(data.into_owned())
                .with_header(Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap())
                .with_header(
                    Header::from_bytes(
                        &b"Cache-Control"[..],
                        &b"no-cache, no-store, must-revalidate"[..],
                    )
                    .unwrap(),
                );
            let _ = request.respond(response);
            continue;
        }

        let not_found = Response::from_string("Not Found").with_status_code(StatusCode(404));
        let _ = request.respond(not_found);
    }

    Ok(())
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}
