mod config;
mod drm;
mod sync;
mod watcher;
mod web;

use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use crate::watcher::add_log;

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Start browser process watcher in a background thread
    tokio::spawn(async {
        watcher::start_watcher().await;
    });

    // Create Axum Router
    let app = web::create_router()
        .layer(CorsLayer::permissive()); // Permit cross-origin requests for local development if needed

    // Define bind address
    let addr = SocketAddr::from(([127, 0, 0, 1], 8384));
    add_log(&format!("Web UI sunucusu başlatılıyor: http://{}", addr));

    // Bind TcpListener
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[CRITICAL] Sunucu adrese bağlanamadı (port {} dolu olabilir): {}", addr, e);
            std::process::exit(1);
        }
    };

    // Run Axum server
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[CRITICAL] Sunucu hatası: {}", e);
    }
}
