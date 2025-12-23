//! Actix-web integration examples.
//!
//! This example demonstrates multiple ways to use html2pdf-api with Actix-web:
//!
//! 1. **Pre-built routes** - Zero configuration, just works
//! 2. **Custom handlers with service functions** - Full control with reusable logic
//! 3. **Manual browser control** - Direct browser operations
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example actix_web_example --features actix-integration
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
//! curl "http://localhost:8080/pdf?url=https://example.com" --output example.pdf
//!
//! # Pre-built HTML to PDF
//! curl -X POST http://localhost:8080/pdf/html \
//!   -H "Content-Type: application/json" \
//!   -d '{"html": "<h1>Hello World</h1>"}' \
//!   --output hello.pdf
//!
//! # Pool statistics
//! curl http://localhost:8080/pool/stats
//!
//! # Health check
//! curl http://localhost:8080/health
//!
//! # Custom handler
//! curl "http://localhost:8080/custom/pdf?url=https://example.com" --output custom.pdf
//!
//! # Manual handler
//! curl http://localhost:8080/manual/pdf --output manual.pdf
//! ```

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use html2pdf_api::integrations::actix::{configure_routes, SharedPool};
use html2pdf_api::prelude::*;
use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Custom Handler Example: Using Service Functions
// ============================================================================

/// Custom handler demonstrating how to use service functions directly.
///
/// This approach gives you full control while reusing the core PDF generation logic.
/// You can add custom authentication, rate limiting, logging, etc.
async fn custom_pdf_handler(
    pool: web::Data<SharedPool>,
    query: web::Query<PdfFromUrlRequest>,
) -> impl Responder {
    // -------------------------------------------------------------------------
    // Custom pre-processing (add your logic here)
    // -------------------------------------------------------------------------
    log::info!("Custom handler called for URL: {}", query.url);

    // Example: Custom validation
    if query.url.contains("blocked-domain.com") {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": "This domain is blocked",
            "code": "DOMAIN_BLOCKED"
        }));
    }

    // Example: Custom logging with request ID
    let request_id = uuid::Uuid::new_v4().to_string();
    log::info!("[{}] Starting PDF generation for: {}", request_id, query.url);

    // -------------------------------------------------------------------------
    // Call the service function in a blocking context
    // -------------------------------------------------------------------------
    let pool = pool.into_inner();
    let request = query.into_inner();

    let result = web::block(move || generate_pdf_from_url(&pool, &request)).await;

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

            // Custom response with additional headers
            HttpResponse::Ok()
                .content_type("application/pdf")
                .insert_header(("X-Request-ID", request_id))
                .insert_header(("X-PDF-Size", pdf_response.size().to_string()))
                .insert_header((
                    "Content-Disposition",
                    pdf_response.content_disposition(),
                ))
                .body(pdf_response.data)
        }
        Ok(Err(service_error)) => {
            log::error!("[{}] Service error: {}", request_id, service_error);

            // Custom error response format
            HttpResponse::build(
                actix_web::http::StatusCode::from_u16(service_error.status_code())
                    .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR),
            )
            .insert_header(("X-Request-ID", request_id))
            .json(serde_json::json!({
                "error": service_error.to_string(),
                "code": service_error.error_code(),
                "retryable": service_error.is_retryable()
            }))
        }
        Err(blocking_error) => {
            log::error!("[{}] Blocking error: {}", request_id, blocking_error);

            HttpResponse::InternalServerError()
                .insert_header(("X-Request-ID", request_id))
                .json(serde_json::json!({
                    "error": "Internal server error",
                    "code": "BLOCKING_ERROR"
                }))
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
async fn manual_pdf_handler(pool: web::Data<SharedBrowserPool>) -> impl Responder {
    // Acquire lock on the pool
    let pool_guard = match pool.lock() {
        Ok(guard) => guard,
        Err(e) => {
            log::error!("Failed to lock pool: {}", e);
            return HttpResponse::InternalServerError().body("Pool lock failed");
        }
    };

    // Get a browser from the pool
    let browser = match pool_guard.get() {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to get browser: {}", e);
            return HttpResponse::ServiceUnavailable().body(format!("Browser unavailable: {}", e));
        }
    };

    log::info!("Got browser {} from pool", browser.id());

    // Create a new tab
    let tab = match browser.new_tab() {
        Ok(t) => t,
        Err(e) => {
            log::error!("Failed to create tab: {}", e);
            return HttpResponse::InternalServerError().body("Failed to create tab");
        }
    };

    // Navigate to the URL
    if let Err(e) = tab.navigate_to("https://www.rust-lang.org") {
        log::error!("Failed to navigate: {}", e);
        return HttpResponse::InternalServerError().body("Failed to navigate");
    }

    // Wait for navigation to complete
    if let Err(e) = tab.wait_until_navigated() {
        log::error!("Navigation timeout: {}", e);
        return HttpResponse::InternalServerError().body("Navigation timeout");
    }

    // Generate PDF
    let pdf_data = match tab.print_to_pdf(None) {
        Ok(data) => data,
        Err(e) => {
            log::error!("Failed to generate PDF: {}", e);
            return HttpResponse::InternalServerError().body("Failed to generate PDF");
        }
    };

    log::info!("Generated PDF: {} bytes", pdf_data.len());

    // Return PDF response
    HttpResponse::Ok()
        .content_type("application/pdf")
        .insert_header(("Content-Disposition", "attachment; filename=\"rust-lang.pdf\""))
        .body(pdf_data)
}

// ============================================================================
// Legacy Handlers (Backward Compatibility)
// ============================================================================

/// Original generate_pdf handler from existing example.
///
/// Kept for backward compatibility - demonstrates the manual approach.
async fn generate_pdf(pool: web::Data<SharedBrowserPool>) -> impl Responder {
    let pool_guard = match pool.lock() {
        Ok(guard) => guard,
        Err(e) => {
            log::error!("Failed to lock pool: {}", e);
            return HttpResponse::InternalServerError().body("Pool lock failed");
        }
    };

    let browser = match pool_guard.get() {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to get browser: {}", e);
            return HttpResponse::ServiceUnavailable().body(format!("Browser unavailable: {}", e));
        }
    };

    log::info!("Got browser {} from pool", browser.id());

    let tab = match browser.new_tab() {
        Ok(t) => t,
        Err(e) => {
            log::error!("Failed to create tab: {}", e);
            return HttpResponse::InternalServerError().body("Failed to create tab");
        }
    };

    if let Err(e) = tab.navigate_to("https://google.com") {
        log::error!("Failed to navigate: {}", e);
        return HttpResponse::InternalServerError().body("Failed to navigate");
    }

    if let Err(e) = tab.wait_until_navigated() {
        log::error!("Navigation timeout: {}", e);
        return HttpResponse::InternalServerError().body("Navigation timeout");
    }

    let pdf_data = match tab.print_to_pdf(None) {
        Ok(data) => data,
        Err(e) => {
            log::error!("Failed to generate PDF: {}", e);
            return HttpResponse::InternalServerError().body("Failed to generate PDF");
        }
    };

    log::info!("Generated PDF: {} bytes", pdf_data.len());

    HttpResponse::Ok()
        .content_type("application/pdf")
        .insert_header(("Content-Disposition", "attachment; filename=\"google.pdf\""))
        .body(pdf_data)
}

/// Original pool_stats handler from existing example.
async fn legacy_pool_stats(pool: web::Data<SharedBrowserPool>) -> impl Responder {
    let pool_guard = match pool.lock() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("Pool lock failed"),
    };

    let stats = pool_guard.stats();

    HttpResponse::Ok().json(serde_json::json!({
        "available": stats.available,
        "active": stats.active,
        "total": stats.total
    }))
}

/// Original health handler from existing example.
async fn legacy_health() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

// ============================================================================
// Main Application
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Actix-web example...");
    log::info!("This example demonstrates multiple integration approaches:");
    log::info!("  1. Pre-built routes (configure_routes)");
    log::info!("  2. Custom handlers with service functions");
    log::info!("  3. Manual browser control");

    // -------------------------------------------------------------------------
    // Option 1: Using init_browser_pool (recommended for production)
    // -------------------------------------------------------------------------
    // Uncomment this and comment out the manual setup below:
    //
    // let pool = init_browser_pool().await
    //     .expect("Failed to initialize browser pool");

    // -------------------------------------------------------------------------
    // Option 2: Manual pool setup (for custom configuration)
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

    // Convert to shared state
    let shared_pool: SharedBrowserPool = Arc::new(std::sync::Mutex::new(pool));
    let shutdown_pool = Arc::clone(&shared_pool);

    log::info!("Starting server on http://localhost:8080");
    log::info!("");
    log::info!("Available endpoints:");
    log::info!("  Pre-built handlers (from configure_routes):");
    log::info!("    GET  http://localhost:8080/pdf?url=https://example.com");
    log::info!("    POST http://localhost:8080/pdf/html");
    log::info!("    GET  http://localhost:8080/pool/stats");
    log::info!("    GET  http://localhost:8080/health");
    log::info!("    GET  http://localhost:8080/ready");
    log::info!("");
    log::info!("  Custom handlers:");
    log::info!("    GET  http://localhost:8080/custom/pdf?url=https://example.com");
    log::info!("    GET  http://localhost:8080/manual/pdf");
    log::info!("");
    log::info!("  Legacy handlers (backward compatibility):");
    log::info!("    GET  http://localhost:8080/legacy/pdf");
    log::info!("    GET  http://localhost:8080/legacy/stats");
    log::info!("    GET  http://localhost:8080/legacy/health");

    // Start server
    let server = HttpServer::new(move || {
        App::new()
            // Share pool with all handlers
            .app_data(web::Data::new(Arc::clone(&shared_pool)))
            //
            // -----------------------------------------------------------------
            // Option 1: Pre-built routes (recommended)
            // -----------------------------------------------------------------
            // This single line adds: /pdf, /pdf/html, /pool/stats, /health, /ready
            .configure(configure_routes)
            //
            // -----------------------------------------------------------------
            // Option 2: Custom handlers using service functions
            // -----------------------------------------------------------------
            .route("/custom/pdf", web::get().to(custom_pdf_handler))
            //
            // -----------------------------------------------------------------
            // Option 3: Manual browser control
            // -----------------------------------------------------------------
            .route("/manual/pdf", web::get().to(manual_pdf_handler))
            //
            // -----------------------------------------------------------------
            // Legacy routes (backward compatibility with original example)
            // -----------------------------------------------------------------
            .service(
                web::scope("/legacy")
                    .route("/pdf", web::get().to(generate_pdf))
                    .route("/stats", web::get().to(legacy_pool_stats))
                    .route("/health", web::get().to(legacy_health)),
            )
    })
    .bind("127.0.0.1:8080")?
    .run();

    let result = server.await;

    // Cleanup pool
    log::info!("Server stopped, cleaning up browser pool...");
    if let Ok(mut pool) = shutdown_pool.lock() {
        pool.shutdown();
    }
    log::info!("Cleanup complete");

    result
}