//! Axum integration example.
//!
//! This example demonstrates multiple ways to use html2pdf-api with Axum:
//!
//! 1. **Pre-built routes** - Zero configuration, just works
//! 2. **Custom handlers with service functions** - Full control with reusable logic
//! 3. **Manual browser control** - Direct browser operations
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example axum_example --features axum-integration
//! ```
//!
//! # Endpoints
//!
//! ## Pre-built (from configure_routes)
//! - `GET  /pdf?url=https://example.com` - Convert URL to PDF
//! - `POST /pdf/html` - Convert HTML to PDF
//! - `GET  /pool/stats` - Pool statistics
//! - `GET  /health` - Health check
//! - `GET  /ready` - Readiness check
//!
//! ## Custom Examples
//! - `GET  /custom/pdf?url=...` - Custom handler using service function
//! - `GET  /manual/pdf` - Manual browser control (original approach)
//!
//! # Testing
//!
//! ```bash
//! # Pre-built URL to PDF
//! curl "http://localhost:3000/pdf?url=https://example.com" --output example.pdf
//!
//! # Pre-built HTML to PDF
//! curl -X POST http://localhost:3000/pdf/html \
//!   -H "Content-Type: application/json" \
//!   -d '{"html": "<h1>Hello World</h1>"}' \
//!   --output hello.pdf
//!
//! # Pool statistics
//! curl http://localhost:3000/pool/stats
//!
//! # Health check
//! curl http://localhost:3000/health
//!
//! # Custom handler
//! curl "http://localhost:3000/custom/pdf?url=https://example.com" --output custom.pdf
//!
//! # Manual handler
//! curl http://localhost:3000/manual/pdf --output manual.pdf
//! ```

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use html2pdf_api::integrations::axum::configure_routes;
use html2pdf_api::service::{
    PdfFromHtmlRequest, PdfFromUrlRequest, generate_pdf_from_html, generate_pdf_from_url,
};
use html2pdf_api::{
    BrowserPool, BrowserPoolConfigBuilder, ChromeBrowserFactory, SharedBrowserPool,
};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;

// ============================================================================
// Custom Query Parameters
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CustomPdfQuery {
    url: String,
    filename: Option<String>,
}

// ============================================================================
// Custom Handler Example: Using Service Functions
// ============================================================================

/// Custom handler demonstrating how to use service functions directly.
///
/// This approach gives you full control while reusing the core PDF generation logic.
/// You can add custom authentication, rate limiting, logging, etc.
async fn custom_pdf_handler(
    State(pool): State<SharedBrowserPool>,
    Query(query): Query<CustomPdfQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // -------------------------------------------------------------------------
    // Custom pre-processing (add your logic here)
    // -------------------------------------------------------------------------
    log::info!("Custom handler called for URL: {}", query.url);

    // Example: Custom validation
    if query.url.contains("blocked-domain.com") {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "This domain is blocked",
                "code": "DOMAIN_BLOCKED"
            })),
        ));
    }

    // Example: Custom logging with request ID
    let request_id = uuid::Uuid::new_v4().to_string();
    log::info!(
        "[{}] Starting PDF generation for: {}",
        request_id,
        query.url
    );

    // -------------------------------------------------------------------------
    // Call the service function in a blocking context
    // -------------------------------------------------------------------------
    let request = PdfFromUrlRequest {
        url: query.url,
        filename: query.filename,
        ..Default::default()
    };

    let result = tokio::task::spawn_blocking(move || generate_pdf_from_url(&pool, &request)).await;

    // -------------------------------------------------------------------------
    // Custom post-processing and response building
    // -------------------------------------------------------------------------
    match result {
        Ok(Ok(pdf_response)) => {
            log::info!(
                "[{}] PDF generated successfully: {} bytes",
                request_id,
                pdf_response.size()
            );

            Ok((
                [
                    (header::CONTENT_TYPE, "application/pdf".to_string()),
                    (
                        header::CONTENT_DISPOSITION,
                        pdf_response.content_disposition(),
                    ),
                    (header::HeaderName::from_static("x-request-id"), request_id),
                    (
                        header::HeaderName::from_static("x-pdf-size"),
                        pdf_response.size().to_string(),
                    ),
                ],
                pdf_response.data,
            ))
        }
        Ok(Err(service_error)) => {
            log::error!("[{}] Service error: {}", request_id, service_error);

            let status = StatusCode::from_u16(service_error.status_code())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            Err((
                status,
                Json(serde_json::json!({
                    "error": service_error.to_string(),
                    "code": service_error.error_code(),
                    "retryable": service_error.is_retryable()
                })),
            ))
        }
        Err(join_error) => {
            log::error!("[{}] Blocking error: {}", request_id, join_error);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Internal server error",
                    "code": "BLOCKING_ERROR"
                })),
            ))
        }
    }
}

// ============================================================================
// Manual Handler Example: Direct Browser Control
// ============================================================================

/// Manual handler demonstrating direct browser control.
///
/// This is the original approach from the existing example.
/// Use this when you need complete control over browser operations.
async fn manual_pdf_handler(
    State(pool): State<SharedBrowserPool>,
) -> Result<impl IntoResponse, StatusCode> {
    // Get a browser from the pool (no lock needed)
    let browser = pool.get().map_err(|e| {
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
    tab.navigate_to("https://www.rust-lang.org").map_err(|e| {
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
                "attachment; filename=\"rust-lang.pdf\"",
            ),
        ],
        pdf_data,
    ))
}

// ============================================================================
// Advanced Handler Example: Print Profiling & Security
// ============================================================================

#[derive(serde::Deserialize)]
pub struct AdvancedPdfQuery {
    pub filename: Option<String>,
    pub landscape: Option<bool>,
    pub print_background: Option<bool>,
    pub waitsecs: Option<u64>,
    pub scale: Option<f64>,
    pub paper_width: Option<f64>,
    pub paper_height: Option<f64>,
    pub margin_top: Option<f64>,
    pub margin_bottom: Option<f64>,
    pub margin_left: Option<f64>,
    pub margin_right: Option<f64>,
    pub page_ranges: Option<String>,
    pub display_header_footer: Option<bool>,
    pub header_template: Option<String>,
    pub footer_template: Option<String>,
    pub prefer_css_page_size: Option<bool>,
    pub offline_mode: Option<bool>,
}

/// Advanced handler demonstrating enterprise print features.
async fn advanced_pdf_handler(
    State(pool): State<SharedBrowserPool>,
    Query(query): Query<AdvancedPdfQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    log::info!("Advanced handler called: Generating secure enterprise report");

    let untrusted_html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { font-family: sans-serif; padding: 20px; }
                h1 { color: #2c3e50; }
                .confidential { color: #e74c3c; border: 1px solid #e74c3c; padding: 10px; margin-top: 20px; }
            </style>
        </head>
        <body>
            <h1>Quarterly Earnings Report</h1>
            <p>This report was generated securely via the Advanced Profiling API.</p>
            <div class="confidential">
                <strong>CONFIDENTIAL DATA</strong>
                <p>DO NOT DISTRIBUTE.</p>
            </div>
            <!-- This fetch will instantly fail because offline_mode is true -->
            <script>
                fetch('http://169.254.169.254/latest/meta-data/')
                    .then(r => console.log('Stolen metadata!', r))
                    .catch(e => console.error('Extraction prevented!', e));
            </script>
        </body>
        </html>
    "#.to_string();

    let request = PdfFromHtmlRequest {
        html: untrusted_html,
        filename: query.filename.clone().or(Some("secure_report.pdf".to_string())),
        paper_width: query.paper_width.or(Some(8.27)),
        paper_height: query.paper_height.or(Some(11.69)),
        margin_top: query.margin_top.or(Some(1.2)),
        margin_bottom: query.margin_bottom.or(Some(1.2)),
        margin_left: query.margin_left.or(Some(0.8)),
        margin_right: query.margin_right.or(Some(0.8)),
        display_header_footer: query.display_header_footer.or(Some(true)),
        header_template: query.header_template.clone().or_else(|| {
            Some(r#"<div style="font-size: 10px; color: #7f8c8d; text-align: right; width: 100%; border-bottom: 1px solid #bdc3c7; padding-bottom: 5px; margin: 0 40px;">Internal Organization Document | CONFIDENTIAL</div>"#.to_string())
        }),
        footer_template: query.footer_template.clone().or_else(|| {
            Some(r#"<div style="font-size: 10px; color: #7f8c8d; text-align: center; width: 100%;">Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>"#.to_string())
        }),
        offline_mode: query.offline_mode.or(Some(true)),
        waitsecs: query.waitsecs.or(Some(1)),
        landscape: query.landscape,
        print_background: query.print_background,
        scale: query.scale,
        page_ranges: query.page_ranges.clone(),
        prefer_css_page_size: query.prefer_css_page_size,
        ..Default::default()
    };

    let result = tokio::task::spawn_blocking(move || generate_pdf_from_html(&pool, &request)).await;

    match result {
        Ok(Ok(pdf_response)) => Ok((
            [
                (header::CONTENT_TYPE, "application/pdf".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    pdf_response.content_disposition(),
                ),
            ],
            pdf_response.data,
        )),
        Ok(Err(service_error)) => {
            log::error!("Service error: {}", service_error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
        Err(e) => {
            log::error!("Blocking error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============================================================================
// Legacy Handlers (Backward Compatibility)
// ============================================================================

/// Original generate_pdf handler from existing example.
async fn generate_pdf(
    State(pool): State<SharedBrowserPool>,
) -> Result<impl IntoResponse, StatusCode> {
    let browser = pool.get().map_err(|e| {
        log::error!("Failed to get browser: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    log::info!("Got browser {} from pool", browser.id());

    let tab = browser.new_tab().map_err(|e| {
        log::error!("Failed to create tab: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tab.navigate_to("https://google.com").map_err(|e| {
        log::error!("Failed to navigate: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tab.wait_until_navigated().map_err(|e| {
        log::error!("Navigation timeout: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let pdf_data = tab.print_to_pdf(None).map_err(|e| {
        log::error!("Failed to generate PDF: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    log::info!("Generated PDF: {} bytes", pdf_data.len());

    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"google.pdf\"",
            ),
        ],
        pdf_data,
    ))
}

/// Original pool_stats handler.
async fn legacy_pool_stats(
    State(pool): State<SharedBrowserPool>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let stats = pool.stats();

    Ok(Json(serde_json::json!({
        "available": stats.available,
        "active": stats.active,
        "total": stats.total
    })))
}

/// Original health handler.
async fn legacy_health() -> &'static str {
    "OK"
}

// ============================================================================
// Shutdown Integration
// ============================================================================

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
    if let Some(mut pool) = Arc::into_inner(pool) {
        pool.shutdown();
    }

    log::info!("Cleanup complete");
}

// ============================================================================
// Main Application
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Axum example...");
    log::info!("This example demonstrates multiple integration approaches:");
    log::info!("  1. Pre-built routes (configure_routes)");
    log::info!("  2. Custom handlers with service functions");
    log::info!("  3. Manual browser control");

    // -------------------------------------------------------------------------
    // Manual pool setup
    // -------------------------------------------------------------------------
    let pool = BrowserPool::builder()
        .config(
            BrowserPoolConfigBuilder::new()
                .max_pool_size(3)
                .warmup_count(2)
                .browser_ttl(Duration::from_secs(3600))
                .ping_interval(Duration::from_secs(15)) // <- seems CDP connection close after 15 seconds
                .build()
                .expect("Invalid configuration"),
        )
        .factory(Box::new(ChromeBrowserFactory::with_defaults()))
        .build()
        .expect("Failed to create browser pool");

    log::info!("Browser pool created, warming up...");
    pool.warmup().await.expect("Failed to warmup pool");
    log::info!("Pool warmed up successfully");

    // Convert to shared state (no Mutex needed)
    let shared_pool = Arc::new(pool);
    let shutdown_pool = Arc::clone(&shared_pool);

    log::info!("Starting server on http://localhost:3000");
    log::info!("");
    log::info!("Available endpoints:");
    log::info!("  Pre-built handlers (from configure_routes):");
    log::info!("    GET  http://localhost:3000/pdf?url=https://example.com");
    log::info!("    POST http://localhost:3000/pdf/html");
    log::info!("    GET  http://localhost:3000/pool/stats");
    log::info!("    GET  http://localhost:3000/health");
    log::info!("    GET  http://localhost:3000/ready");
    log::info!("");
    log::info!("  Custom handlers:");
    log::info!("    GET  http://localhost:3000/custom/pdf?url=https://example.com");
    log::info!("    GET  http://localhost:3000/manual/pdf");
    log::info!("    GET  http://localhost:3000/advanced/pdf");
    log::info!("");
    log::info!("  Legacy handlers (backward compatibility):");
    log::info!("    GET  http://localhost:3000/legacy/pdf");
    log::info!("    GET  http://localhost:3000/legacy/stats");
    log::info!("    GET  http://localhost:3000/legacy/health");

    // Build router
    let app = Router::new()
        // -----------------------------------------------------------------
        // Option 1: Pre-built routes (recommended)
        // -----------------------------------------------------------------
        // This single line adds: /pdf, /pdf/html, /pool/stats, /health, /ready
        .merge(configure_routes())
        //
        // -----------------------------------------------------------------
        // Option 2: Custom handlers using service functions
        // -----------------------------------------------------------------
        .route("/custom/pdf", get(custom_pdf_handler))
        // -----------------------------------------------------------------
        // Option 4: Enterprise profiling showcase
        // -----------------------------------------------------------------
        .route("/advanced/pdf", get(advanced_pdf_handler))
        // -----------------------------------------------------------------
        // Option 3: Manual browser control
        // -----------------------------------------------------------------
        .route("/manual/pdf", get(manual_pdf_handler))
        // -----------------------------------------------------------------
        // Legacy routes (backward compatibility with original example)
        // -----------------------------------------------------------------
        .route("/legacy/pdf", get(generate_pdf))
        .route("/legacy/stats", get(legacy_pool_stats))
        .route("/legacy/health", get(legacy_health))
        .with_state(shared_pool);

    // Start server with graceful shutdown
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_pool))
        .await
        .expect("Server error");
}
