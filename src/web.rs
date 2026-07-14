use axum::{
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};

use crate::config::{load_config, save_config, Config};
use crate::drm::{check_drm_status, fix_drm};
use crate::sync::{get_helium_profile_dir, trigger_push};
use crate::watcher::{add_log, get_logs, is_helium_running};

#[derive(RustEmbed)]
#[folder = "frontend/dist"]
struct Assets;

#[derive(Serialize)]
struct StatusResponse {
    provider: String,
    github_connected: bool,
    github_gist_id: String,
    webdav_url: String,
    last_sync_time: String,
    last_sync_size_bytes: u64,
    encryption_active: bool,
    browser_running: bool,
    drm_status: String,
    profile_path: String,
    platform: String,
}

#[derive(Deserialize)]
struct SettingsRequest {
    provider: String,
    webdav_url: String,
    webdav_username: String,
    webdav_password: String,
    webdav_folder: String,
    encryption_active: bool,
    encryption_password: String,
    profile_path: String,
    github_token: String,
    github_gist_id: String,
}

pub fn create_router() -> Router {
    Router::new()
        .route("/api/status", get(get_status_handler))
        .route("/api/settings", get(get_settings_handler).post(post_settings_handler))
        .route("/api/sync", post(post_sync_handler))
        .route("/api/fix-drm", post(post_fix_drm_handler))
        .route("/api/logs", get(get_logs_handler))
        .fallback(static_handler)
}

// Serve embedded React static assets
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            ).into_response()
        }
        None => {
            // SPA fallback: return index.html for unknown client-side routes
            match Assets::get("index.html") {
                Some(content) => {
                    (
                        [(header::CONTENT_TYPE, "text/html")],
                        content.data.into_owned(),
                    ).into_response()
                }
                None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
            }
        }
    }
}

// GET /api/status
async fn get_status_handler() -> Json<StatusResponse> {
    let config = load_config();
    let profile_path = get_helium_profile_dir(&config)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
        
    let platform = if cfg!(target_os = "linux") {
        "linux".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "other".to_string()
    };

    Json(StatusResponse {
        provider: config.provider,
        github_connected: !config.github_token.is_empty(),
        github_gist_id: config.github_gist_id,
        webdav_url: config.webdav_url,
        last_sync_time: config.last_sync_time,
        last_sync_size_bytes: config.last_sync_size_bytes,
        encryption_active: config.encryption_active,
        browser_running: is_helium_running(),
        drm_status: check_drm_status(),
        profile_path,
        platform,
    })
}

// GET /api/settings
async fn get_settings_handler() -> Json<Config> {
    Json(load_config())
}

// POST /api/settings
async fn post_settings_handler(Json(payload): Json<SettingsRequest>) -> impl IntoResponse {
    let mut config = load_config();
    config.provider = payload.provider;
    config.webdav_url = payload.webdav_url;
    config.webdav_username = payload.webdav_username;
    config.webdav_password = payload.webdav_password;
    config.webdav_folder = payload.webdav_folder;
    config.encryption_active = payload.encryption_active;
    config.encryption_password = payload.encryption_password;
    config.profile_path = payload.profile_path;
    config.github_token = payload.github_token;
    config.github_gist_id = payload.github_gist_id;

    match save_config(&config) {
        Ok(_) => {
            add_log("Configuration settings updated.");
            StatusCode::OK.into_response()
        }
        Err(e) => {
            add_log(&format!("[ERROR] Failed to save settings: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
    }
}

// POST /api/sync
async fn post_sync_handler() -> impl IntoResponse {
    let mut config = load_config();
    add_log("Manual synchronization (Push) triggered...");
    
    match trigger_push(&mut config).await {
        Ok(_) => {
            add_log("Manual synchronization completed successfully.");
            StatusCode::OK.into_response()
        }
        Err(e) => {
            add_log(&format!("[ERROR] Manual synchronization error: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
    }
}

// POST /api/fix-drm
async fn post_fix_drm_handler() -> impl IntoResponse {
    add_log("Widevine DRM fixer triggered...");
    match fix_drm() {
        Ok(_) => {
            add_log("Widevine DRM libraries copied and integrated into Helium successfully!");
            StatusCode::OK.into_response()
        }
        Err(e) => {
            add_log(&format!("[ERROR] DRM fix error: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
    }
}

// GET /api/logs
async fn get_logs_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "logs": get_logs()
    }))
}


