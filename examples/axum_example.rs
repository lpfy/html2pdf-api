//! Axum integration example.
//!
//! Run with:
//! ```bash
//! cargo run --example axum_example --features axum-integration
//! ```
//!
//! Then visit: http://localhost:3000/pdf

use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use html2pdf_api::{
    BrowserPool, BrowserPoolConfigBuilder, ChromeBrowserFactory, SharedBrowserPool,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;

/// Handler that generates a PDF from a URL.
async fn generate_pdf(
    State(pool): State<SharedBrowserPool>,
) -> Result<impl IntoResponse, StatusCode> {
    // Acquire lock on the pool
    let pool_guard = pool.lock().map_err(|e| {
        log::error!("Failed to lock pool: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get a browser from the pool
    let browser = pool_guard.get().map_err(|e| {
        log::error!("Failed to get browser: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    log::info!("Got browser {} from pool", browser.id());

    // Create a new tab
    let tab = browser.new_tab().map_err(|e| {
        log::error!("Failed to create tab: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Navigate to the URL
    tab.navigate_to("https://example.com").map_err(|e| {
        log::error!("Failed to navigate: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Wait for navigation to complete
    tab.wait_until_navigated().map_err(|e| {
        log::error!("Navigation timeout: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Generate PDF
    let pdf_data = tab.print_to_pdf(None).map_err(|e| {
        log::error!("Failed to generate PDF: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    log::info!("Generated PDF: {} bytes", pdf_data.len());

    // Return PDF response
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"example.pdf\"",
            ),
        ],
        pdf_data,
    ))
}

/// Handler that returns pool statistics.
async fn pool_stats(
    State(pool): State<SharedBrowserPool>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool_guard = pool.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let stats = pool_guard.stats();

    Ok(Json(serde_json::json!({
        "available": stats.available,
        "active": stats.active,
        "total": stats.total
    })))
}

/// Health check endpoint.
async fn health() -> &'static str {
    "OK"
}

/// Shutdown signal handler.
async fn shutdown_signal(pool: SharedBrowserPool) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    log::info!("Shutdown signal received, cleaning up...");

    // Cleanup pool
    if let Ok(mut pool_guard) = pool.lock() {
        pool_guard.shutdown();
    }

    log::info!("Cleanup complete");
}

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Axum example...");

    // Create browser pool
    let pool = BrowserPool::builder()
        .config(
            BrowserPoolConfigBuilder::new()
                .max_pool_size(3)
                .warmup_count(2)
                .browser_ttl(Duration::from_secs(3600))
                .ping_interval(Duration::from_secs(30))
                .build()
                .expect("Invalid configuration"),
        )
        .factory(Box::new(ChromeBrowserFactory::with_defaults()))
        .build()
        .expect("Failed to create browser pool");

    log::info!("Browser pool created, warming up...");

    // Warmup the pool
    pool.warmup().await.expect("Failed to warmup pool");

    log::info!("Pool warmed up successfully");

    // Convert to shared state
    let shared_pool = Arc::new(std::sync::Mutex::new(pool));
    let shutdown_pool = Arc::clone(&shared_pool);

    // Build router
    let app = Router::new()
        .route("/pdf", get(generate_pdf))
        .route("/stats", get(pool_stats))
        .route("/health", get(health))
        .with_state(shared_pool);

    log::info!("Starting server on http://localhost:3000");

    // Start server with graceful shutdown
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_pool))
        .await
        .expect("Server error");
}
