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
        .layer(CorsLayer::permissive());

    // Define bind address
    let addr = SocketAddr::from(([127, 0, 0, 1], 8384));
    add_log(&format!("REST API daemon server starting: http://{}", addr));

    // Bind TcpListener
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[CRITICAL] Server failed to bind to address (port {} may be in use): {}", addr, e);
            std::process::exit(1);
        }
    };

    // Run Axum server with graceful shutdown on SIGTERM
    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.ok();
        add_log("Shutdown signal received. Running final sync...");

        // Perform a final push sync before exiting
        let mut config = config::load_config();
        if config.provider != "none" {
            match sync::trigger_push(&mut config).await {
                Ok(_) => add_log("Shutdown sync completed successfully."),
                Err(e) => add_log(&format!("[ERROR] Shutdown sync failed: {}", e)),
            }
        }
        add_log("Helium Sync Daemon shutting down. Goodbye!");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap_or_else(|e| eprintln!("[CRITICAL] Server error: {}", e));
}
