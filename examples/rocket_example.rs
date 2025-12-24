//! Rocket integration examples.
//!
//! This example demonstrates multiple ways to use html2pdf-api with Rocket:
//!
//! 1. **Pre-built routes** - Zero configuration, just works
//! 2. **Custom handlers with service functions** - Full control with reusable logic
//! 3. **Manual browser control** - Direct browser operations
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example rocket_example --features rocket-integration
//! ```
//!
//! # Endpoints
//!
//! ## Pre-built (from routes())
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
//! curl "http://localhost:8000/pdf?url=https://example.com" --output example.pdf
//!
//! # Pre-built HTML to PDF
//! curl -X POST http://localhost:8000/pdf/html \
//!   -H "Content-Type: application/json" \
//!   -d '{"html": "<h1>Hello World</h1>"}' \
//!   --output hello.pdf
//!
//! # Pool statistics
//! curl http://localhost:8000/pool/stats
//!
//! # Health check
//! curl http://localhost:8000/health
//!
//! # Custom handler
//! curl "http://localhost:8000/custom/pdf?url=https://example.com" --output custom.pdf
//!
//! # Manual handler
//! curl http://localhost:8000/manual/pdf --output manual.pdf
//! ```

// ============================================================================
// IMPORTANT: Import rocket types BEFORE the glob import to avoid conflicts
// ============================================================================

use rocket::serde::json::Json;
use rocket::{
    Build, Orbit, Request, Rocket, State,
    fairing::{Fairing, Info, Kind},
    form::FromForm,
    get,
    http::{ContentType, Header, Status},
    response::{self, Responder},
    routes,
};

// Now import prelude (the rocket module in prelude will be shadowed by above)
use html2pdf_api::SharedBrowserPool;
use html2pdf_api::config::BrowserPoolConfigBuilder;
use html2pdf_api::factory::ChromeBrowserFactory;
use html2pdf_api::integrations::rocket::SharedPool;
use html2pdf_api::pool::BrowserPool;
use html2pdf_api::service::{PdfFromUrlRequest, generate_pdf_from_url};

use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Custom Query Parameters
// ============================================================================

/// Query parameters for custom PDF handler.
#[derive(Debug, FromForm)]
struct CustomPdfQuery {
    /// URL to convert to PDF.
    url: String,
    /// Optional filename for the output.
    filename: Option<String>,
}

// ============================================================================
// Custom Response Types
// ============================================================================

/// Custom PDF response with additional headers.
///
/// Demonstrates how to create custom responders with extra metadata.
struct CustomPdfResponse {
    data: Vec<u8>,
    request_id: String,
    size: usize,
    content_disposition: String,
}

impl<'r> Responder<'r, 'static> for CustomPdfResponse {
    fn respond_to(self, _request: &'r Request<'_>) -> response::Result<'static> {
        response::Response::build()
            .header(ContentType::PDF)
            .header(Header::new("X-Request-ID", self.request_id))
            .header(Header::new("X-PDF-Size", self.size.to_string()))
            .header(Header::new("Content-Disposition", self.content_disposition))
            .sized_body(self.data.len(), std::io::Cursor::new(self.data))
            .ok()
    }
}

/// Custom error response with request tracking.
///
/// Demonstrates how to create custom error responders with metadata.
struct CustomErrorResponse {
    status: Status,
    request_id: String,
    error: String,
    code: String,
    retryable: bool,
}

impl<'r> Responder<'r, 'static> for CustomErrorResponse {
    fn respond_to(self, request: &'r Request<'_>) -> response::Result<'static> {
        let json = Json(serde_json::json!({
            "error": self.error,
            "code": self.code,
            "retryable": self.retryable
        }));

        response::Response::build_from(json.respond_to(request)?)
            .status(self.status)
            .header(Header::new("X-Request-ID", self.request_id))
            .ok()
    }
}

// ============================================================================
// Custom Handler Example: Using Service Functions
// ============================================================================

/// Custom handler demonstrating how to use service functions directly.
///
/// This approach gives you full control while reusing the core PDF generation logic.
/// You can add custom authentication, rate limiting, logging, etc.
#[get("/custom/pdf?<query..>")]
async fn custom_pdf_handler(
    pool: &State<SharedPool>,
    query: CustomPdfQuery,
) -> Result<CustomPdfResponse, CustomErrorResponse> {
    // -------------------------------------------------------------------------
    // Custom pre-processing (add your logic here)
    // -------------------------------------------------------------------------
    log::info!("Custom handler called for URL: {}", query.url);

    // Example: Custom validation
    if query.url.contains("blocked-domain.com") {
        return Err(CustomErrorResponse {
            status: Status::Forbidden,
            request_id: uuid::Uuid::new_v4().to_string(),
            error: "This domain is blocked".to_string(),
            code: "DOMAIN_BLOCKED".to_string(),
            retryable: false,
        });
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
    let pool = Arc::clone(pool.inner());
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

            // Calculate size and content_disposition BEFORE moving data
            let size = pdf_response.size();
            let content_disposition = pdf_response.content_disposition();

            Ok(CustomPdfResponse {
                data: pdf_response.data,
                request_id,
                size,
                content_disposition,
            })
        }
        Ok(Err(service_error)) => {
            log::error!("[{}] Service error: {}", request_id, service_error);

            let status = match service_error.status_code() {
                400 => Status::BadRequest,
                502 => Status::BadGateway,
                503 => Status::ServiceUnavailable,
                504 => Status::GatewayTimeout,
                _ => Status::InternalServerError,
            };

            Err(CustomErrorResponse {
                status,
                request_id,
                error: service_error.to_string(),
                code: service_error.error_code().to_string(),
                retryable: service_error.is_retryable(),
            })
        }
        Err(join_error) => {
            log::error!("[{}] Blocking error: {}", request_id, join_error);

            Err(CustomErrorResponse {
                status: Status::InternalServerError,
                request_id,
                error: "Internal server error".to_string(),
                code: "BLOCKING_ERROR".to_string(),
                retryable: true,
            })
        }
    }
}

// ============================================================================
// Manual Handler Example: Direct Browser Control
// ============================================================================

/// Manual PDF response for simple cases.
struct ManualPdfResponse {
    data: Vec<u8>,
    filename: String,
}

impl<'r> Responder<'r, 'static> for ManualPdfResponse {
    fn respond_to(self, _request: &'r Request<'_>) -> response::Result<'static> {
        response::Response::build()
            .header(ContentType::PDF)
            .header(Header::new(
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", self.filename),
            ))
            .sized_body(self.data.len(), std::io::Cursor::new(self.data))
            .ok()
    }
}

/// Manual handler demonstrating direct browser control.
///
/// This is the original approach from the existing example.
/// Use this when you need complete control over browser operations.
#[get("/manual/pdf")]
fn manual_pdf_handler(
    pool: &State<SharedBrowserPool>,
) -> Result<ManualPdfResponse, (Status, String)> {
    // Acquire lock on the pool
    let pool_guard = pool.lock().map_err(|e| {
        log::error!("Failed to lock pool: {}", e);
        (Status::InternalServerError, "Pool lock failed".to_string())
    })?;

    // Get a browser from the pool
    let browser = pool_guard.get().map_err(|e| {
        log::error!("Failed to get browser: {}", e);
        (
            Status::ServiceUnavailable,
            format!("Browser unavailable: {}", e),
        )
    })?;

    log::info!("Got browser {} from pool", browser.id());

    // Create a new tab
    let tab = browser.new_tab().map_err(|e| {
        log::error!("Failed to create tab: {}", e);
        (
            Status::InternalServerError,
            "Failed to create tab".to_string(),
        )
    })?;

    // Navigate to the URL
    tab.navigate_to("https://www.rust-lang.org").map_err(|e| {
        log::error!("Failed to navigate: {}", e);
        (
            Status::InternalServerError,
            "Failed to navigate".to_string(),
        )
    })?;

    // Wait for navigation to complete
    tab.wait_until_navigated().map_err(|e| {
        log::error!("Navigation timeout: {}", e);
        (
            Status::InternalServerError,
            "Navigation timeout".to_string(),
        )
    })?;

    // Generate PDF
    let pdf_data = tab.print_to_pdf(None).map_err(|e| {
        log::error!("Failed to generate PDF: {}", e);
        (
            Status::InternalServerError,
            "Failed to generate PDF".to_string(),
        )
    })?;

    log::info!("Generated PDF: {} bytes", pdf_data.len());

    // Return PDF response
    Ok(ManualPdfResponse {
        data: pdf_data,
        filename: "rust-lang.pdf".to_string(),
    })
}

// ============================================================================
// Legacy Handlers (Backward Compatibility)
// ============================================================================

/// Original generate_pdf handler from existing example.
///
/// Kept for backward compatibility - demonstrates the manual approach.
#[get("/legacy/pdf")]
fn generate_pdf(pool: &State<SharedBrowserPool>) -> Result<ManualPdfResponse, (Status, String)> {
    let pool_guard = pool.lock().map_err(|e| {
        log::error!("Failed to lock pool: {}", e);
        (Status::InternalServerError, "Pool lock failed".to_string())
    })?;

    let browser = pool_guard.get().map_err(|e| {
        log::error!("Failed to get browser: {}", e);
        (
            Status::ServiceUnavailable,
            format!("Browser unavailable: {}", e),
        )
    })?;

    log::info!("Got browser {} from pool", browser.id());

    let tab = browser.new_tab().map_err(|e| {
        log::error!("Failed to create tab: {}", e);
        (
            Status::InternalServerError,
            "Failed to create tab".to_string(),
        )
    })?;

    tab.navigate_to("https://google.com").map_err(|e| {
        log::error!("Failed to navigate: {}", e);
        (
            Status::InternalServerError,
            "Failed to navigate".to_string(),
        )
    })?;

    tab.wait_until_navigated().map_err(|e| {
        log::error!("Navigation timeout: {}", e);
        (
            Status::InternalServerError,
            "Navigation timeout".to_string(),
        )
    })?;

    let pdf_data = tab.print_to_pdf(None).map_err(|e| {
        log::error!("Failed to generate PDF: {}", e);
        (
            Status::InternalServerError,
            "Failed to generate PDF".to_string(),
        )
    })?;

    log::info!("Generated PDF: {} bytes", pdf_data.len());

    Ok(ManualPdfResponse {
        data: pdf_data,
        filename: "google.pdf".to_string(),
    })
}

/// Original pool_stats handler from existing example.
#[get("/legacy/stats")]
fn legacy_pool_stats(pool: &State<SharedBrowserPool>) -> Result<Json<serde_json::Value>, Status> {
    let pool_guard = pool.lock().map_err(|_| Status::InternalServerError)?;

    let stats = pool_guard.stats();

    Ok(Json(serde_json::json!({
        "available": stats.available,
        "active": stats.active,
        "total": stats.total
    })))
}

/// Original health handler from existing example.
#[get("/legacy/health")]
fn legacy_health() -> &'static str {
    "OK"
}

// ============================================================================
// Shutdown Fairing
// ============================================================================

/// Fairing for graceful shutdown of the browser pool.
///
/// This ensures all browser processes are properly terminated
/// when the server stops.
struct ShutdownFairing {
    pool: SharedBrowserPool,
}

#[rocket::async_trait]
impl Fairing for ShutdownFairing {
    fn info(&self) -> Info {
        Info {
            name: "Browser Pool Shutdown",
            kind: Kind::Shutdown,
        }
    }

    async fn on_shutdown(&self, _rocket: &Rocket<Orbit>) {
        log::info!("Server stopping, cleaning up browser pool...");
        if let Ok(mut pool) = self.pool.lock() {
            pool.shutdown();
        }
        log::info!("Cleanup complete");
    }
}

// ============================================================================
// Rocket Configuration
// ============================================================================

/// Build the Rocket instance with all routes and managed state.
fn build_rocket(pool: SharedBrowserPool) -> Rocket<Build> {
    // Clone for shutdown fairing
    let shutdown_pool = Arc::clone(&pool);

    rocket::build()
        // -----------------------------------------------------------------
        // Manage shared pool state
        // -----------------------------------------------------------------
        .manage(pool)
        // -----------------------------------------------------------------
        // Attach shutdown fairing for cleanup
        // -----------------------------------------------------------------
        .attach(ShutdownFairing {
            pool: shutdown_pool,
        })
        // -----------------------------------------------------------------
        // Option 1: Pre-built routes (recommended)
        // Mount all pre-built routes from the integration module
        // -----------------------------------------------------------------
        .mount("/", html2pdf_api::integrations::rocket::routes())
        // -----------------------------------------------------------------
        // Option 2: Custom handlers using service functions
        // -----------------------------------------------------------------
        .mount("/", routes![custom_pdf_handler])
        // -----------------------------------------------------------------
        // Option 3: Manual browser control
        // -----------------------------------------------------------------
        .mount("/", routes![manual_pdf_handler])
        // -----------------------------------------------------------------
        // Legacy routes (backward compatibility with original example)
        // -----------------------------------------------------------------
        .mount("/", routes![generate_pdf, legacy_pool_stats, legacy_health])
}

// ============================================================================
// Main Application
// ============================================================================

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Rocket example...");
    log::info!("This example demonstrates multiple integration approaches:");
    log::info!("  1. Pre-built routes (routes())");
    log::info!("  2. Custom handlers with service functions");
    log::info!("  3. Manual browser control");

    // -------------------------------------------------------------------------
    // Option 1: Using init_browser_pool (recommended for production)
    // -------------------------------------------------------------------------
    // Uncomment this and comment out the manual setup below:
    //
    // use html2pdf_api::prelude::init_browser_pool;
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

    log::info!("Starting server on http://localhost:8000");
    log::info!("");
    log::info!("Available endpoints:");
    log::info!("  Pre-built handlers (from routes()):");
    log::info!("    GET  http://localhost:8000/pdf?url=https://example.com");
    log::info!("    POST http://localhost:8000/pdf/html");
    log::info!("    GET  http://localhost:8000/pool/stats");
    log::info!("    GET  http://localhost:8000/health");
    log::info!("    GET  http://localhost:8000/ready");
    log::info!("");
    log::info!("  Custom handlers:");
    log::info!("    GET  http://localhost:8000/custom/pdf?url=https://example.com");
    log::info!("    GET  http://localhost:8000/manual/pdf");
    log::info!("");
    log::info!("  Legacy handlers (backward compatibility):");
    log::info!("    GET  http://localhost:8000/legacy/pdf");
    log::info!("    GET  http://localhost:8000/legacy/stats");
    log::info!("    GET  http://localhost:8000/legacy/health");

    // Build and launch Rocket
    let _rocket = build_rocket(shared_pool).launch().await?;

    Ok(())
}
