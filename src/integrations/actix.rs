//! Actix-web framework integration.
//!
//! This module provides helpers and pre-built handlers for using `BrowserPool`
//! with Actix-web. You can choose between using the pre-built handlers for
//! quick setup, or writing custom handlers for full control.
//!
//! # Quick Start
//!
//! ## Option 1: Pre-built Routes (Fastest Setup)
//!
//! Use [`configure_routes`] to add all PDF endpoints with a single line:
//!
//! ```rust,ignore
//! use actix_web::{App, HttpServer, web};
//! use html2pdf_api::prelude::*;
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .app_data(web::Data::new(pool.clone()))
//!             .configure(html2pdf_api::integrations::actix::configure_routes)
//!     })
//!     .bind("127.0.0.1:8080")?
//!     .run()
//!     .await
//! }
//! ```
//!
//! This gives you the following endpoints:
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/pdf?url=...` | Convert URL to PDF |
//! | POST | `/pdf/html` | Convert HTML to PDF |
//! | GET | `/pool/stats` | Pool statistics |
//! | GET | `/health` | Health check |
//! | GET | `/ready` | Readiness check |
//!
//! ## Option 2: Mix Pre-built and Custom Handlers
//!
//! Use individual pre-built handlers alongside your own:
//!
//! ```rust,ignore
//! use actix_web::{App, HttpServer, web};
//! use html2pdf_api::prelude::*;
//! use html2pdf_api::integrations::actix::{pdf_from_url, health_check};
//!
//! async fn my_custom_handler() -> impl Responder {
//!     // Your custom logic
//! }
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let pool = init_browser_pool().await?;
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .app_data(web::Data::new(pool.clone()))
//!             // Pre-built handlers
//!             .route("/pdf", web::get().to(pdf_from_url))
//!             .route("/health", web::get().to(health_check))
//!             // Custom handler
//!             .route("/custom", web::get().to(my_custom_handler))
//!     })
//!     .bind("127.0.0.1:8080")?
//!     .run()
//!     .await
//! }
//! ```
//!
//! ## Option 3: Custom Handlers with Service Functions
//!
//! For full control, use the service functions directly:
//!
//! ```rust,ignore
//! use actix_web::{web, HttpResponse, Responder};
//! use html2pdf_api::prelude::*;
//! use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};
//!
//! async fn my_pdf_handler(
//!     pool: web::Data<SharedBrowserPool>,
//!     query: web::Query<PdfFromUrlRequest>,
//! ) -> impl Responder {
//!     // Custom pre-processing: auth, rate limiting, logging, etc.
//!     log::info!("Custom handler: {}", query.url);
//!
//!     let pool = pool.into_inner();
//!     let request = query.into_inner();
//!
//!     // Call service in blocking context
//!     let result = web::block(move || {
//!         generate_pdf_from_url(&pool, &request)
//!     }).await;
//!
//!     match result {
//!         Ok(Ok(pdf)) => {
//!             // Custom post-processing
//!             HttpResponse::Ok()
//!                 .content_type("application/pdf")
//!                 .insert_header(("X-Custom-Header", "value"))
//!                 .body(pdf.data)
//!         }
//!         Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
//!         Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
//!     }
//! }
//! ```
//!
//! ## Option 4: Full Manual Control (Original Approach)
//!
//! For complete control over browser operations:
//!
//! ```rust,ignore
//! use actix_web::{web, HttpResponse, Responder};
//! use html2pdf_api::prelude::*;
//!
//! async fn manual_pdf_handler(
//!     pool: web::Data<SharedBrowserPool>,
//! ) -> impl Responder {
//!     let pool_guard = match pool.lock() {
//!         Ok(guard) => guard,
//!         Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
//!     };
//!
//!     let browser = match pool_guard.get() {
//!         Ok(b) => b,
//!         Err(e) => return HttpResponse::ServiceUnavailable().body(e.to_string()),
//!     };
//!
//!     let tab = browser.new_tab().unwrap();
//!     tab.navigate_to("https://example.com").unwrap();
//!     tab.wait_until_navigated().unwrap();
//!
//!     let pdf_data = tab.print_to_pdf(None).unwrap();
//!
//!     HttpResponse::Ok()
//!         .content_type("application/pdf")
//!         .body(pdf_data)
//! }
//! ```
//!
//! # Setup
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! html2pdf-api = { version = "0.2", features = ["actix-integration"] }
//! actix-web = "4"
//! ```
//!
//! # Graceful Shutdown
//!
//! For proper cleanup, shutdown the pool when the server stops:
//!
//! ```rust,ignore
//! use actix_web::{App, HttpServer, web};
//! use html2pdf_api::prelude::*;
//! use std::sync::Arc;
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     // Keep a reference for shutdown
//!     let shutdown_pool = pool.clone();
//!
//!     let server = HttpServer::new(move || {
//!         App::new()
//!             .app_data(web::Data::new(pool.clone()))
//!             .configure(html2pdf_api::integrations::actix::configure_routes)
//!     })
//!     .bind("127.0.0.1:8080")?
//!     .run();
//!
//!     // Run server
//!     let result = server.await;
//!
//!     // Cleanup pool after server stops
//!     if let Ok(mut pool) = shutdown_pool.lock() {
//!         pool.shutdown();
//!     }
//!
//!     result
//! }
//! ```
//!
//! # API Reference
//!
//! ## Pre-built Handlers
//!
//! | Handler | Method | Default Path | Description |
//! |---------|--------|--------------|-------------|
//! | [`pdf_from_url`] | GET | `/pdf` | Convert URL to PDF |
//! | [`pdf_from_html`] | POST | `/pdf/html` | Convert HTML to PDF |
//! | [`pool_stats`] | GET | `/pool/stats` | Pool statistics |
//! | [`health_check`] | GET | `/health` | Health check (always 200) |
//! | [`readiness_check`] | GET | `/ready` | Readiness check (checks pool) |
//!
//! ## Type Aliases
//!
//! | Type | Description |
//! |------|-------------|
//! | [`SharedPool`] | `Arc<Mutex<BrowserPool>>` - for service functions |
//! | [`BrowserPoolData`] | `web::Data<SharedBrowserPool>` - for handler parameters |
//!
//! ## Helper Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | [`configure_routes`] | Configure all pre-built routes |
//! | [`create_pool_data`] | Wrap `SharedBrowserPool` in `web::Data` |
//! | [`create_pool_data_from_arc`] | Wrap `Arc<Mutex<BrowserPool>>` in `web::Data` |
//!
//! ## Extension Traits
//!
//! | Trait | Description |
//! |-------|-------------|
//! | [`BrowserPoolActixExt`] | Adds `into_actix_data()` to `BrowserPool` |

use actix_web::{HttpResponse, Responder, http::header, web};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::SharedBrowserPool;
use crate::pool::BrowserPool;
use crate::service::{
    self, DEFAULT_TIMEOUT_SECS, ErrorResponse, HealthResponse, PdfFromHtmlRequest,
    PdfFromUrlRequest, PdfServiceError,
};

// ============================================================================
// Type Aliases
// ============================================================================

/// Type alias for shared browser pool.
///
/// This is the standard pool type used by the service functions.
/// It's an `Arc<Mutex<BrowserPool>>` which allows safe sharing across
/// threads and handlers.
///
/// # Usage
///
/// ```rust,ignore
/// use html2pdf_api::integrations::actix::SharedPool;
///
/// fn my_function(pool: &SharedPool) {
///     let guard = pool.lock().unwrap();
///     let browser = guard.get().unwrap();
///     // ...
/// }
/// ```
pub type SharedPool = Arc<Mutex<BrowserPool>>;

/// Type alias for Actix-web `Data` wrapper around the shared pool.
///
/// Use this type in your handler parameters for automatic extraction:
///
/// ```rust,ignore
/// use html2pdf_api::integrations::actix::BrowserPoolData;
///
/// async fn handler(pool: BrowserPoolData) -> impl Responder {
///     let pool_guard = pool.lock().unwrap();
///     let browser = pool_guard.get()?;
///     // ...
/// }
/// ```
///
/// # Note
///
/// `BrowserPoolData` and `web::Data<SharedPool>` are interchangeable.
/// Use whichever is more convenient for your code.
pub type BrowserPoolData = web::Data<SharedBrowserPool>;

// ============================================================================
// Pre-built Handlers
// ============================================================================

/// Generate PDF from a URL.
///
/// This handler converts a web page to PDF using the browser pool.
///
/// # Endpoint
///
/// ```text
/// GET /pdf?url=https://example.com&filename=output.pdf
/// ```
///
/// # Query Parameters
///
/// | Parameter | Type | Required | Default | Description |
/// |-----------|------|----------|---------|-------------|
/// | `url` | string | **Yes** | - | URL to convert (must be valid HTTP/HTTPS) |
/// | `filename` | string | No | `"document.pdf"` | Output filename |
/// | `waitsecs` | u64 | No | `5` | Seconds to wait for JavaScript |
/// | `landscape` | bool | No | `false` | Use landscape orientation |
/// | `download` | bool | No | `false` | Force download vs inline display |
/// | `print_background` | bool | No | `true` | Include background graphics |
///
/// # Response
///
/// ## Success (200 OK)
///
/// Returns PDF binary data with headers:
/// - `Content-Type: application/pdf`
/// - `Content-Disposition: inline; filename="document.pdf"` (or `attachment` if `download=true`)
/// - `Cache-Control: no-cache`
///
/// ## Errors
///
/// | Status | Code | Description |
/// |--------|------|-------------|
/// | 400 | `INVALID_URL` | URL is empty or malformed |
/// | 502 | `NAVIGATION_FAILED` | Failed to load the URL |
/// | 503 | `BROWSER_UNAVAILABLE` | No browsers available in pool |
/// | 504 | `TIMEOUT` | Operation timed out |
///
/// # Examples
///
/// ## Basic Request
///
/// ```text
/// GET /pdf?url=https://example.com
/// ```
///
/// ## With Options
///
/// ```text
/// GET /pdf?url=https://example.com/report&filename=report.pdf&landscape=true&waitsecs=10
/// ```
///
/// ## Force Download
///
/// ```text
/// GET /pdf?url=https://example.com&download=true&filename=download.pdf
/// ```
///
/// # Usage in App
///
/// ```rust,ignore
/// App::new()
///     .app_data(web::Data::new(pool.clone()))
///     .route("/pdf", web::get().to(pdf_from_url))
/// ```
pub async fn pdf_from_url(
    pool: web::Data<SharedPool>,
    query: web::Query<PdfFromUrlRequest>,
) -> impl Responder {
    let request = query.into_inner();
    let pool = pool.into_inner();

    log::debug!("PDF from URL request: {}", request.url);

    // Run blocking PDF generation with timeout
    let result = tokio::time::timeout(
        Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        web::block(move || service::generate_pdf_from_url(&pool, &request)),
    )
    .await;

    match result {
        Ok(Ok(Ok(response))) => build_pdf_response(response),
        Ok(Ok(Err(e))) => build_error_response(e),
        Ok(Err(blocking_err)) => {
            log::error!("Blocking task error: {}", blocking_err);
            build_error_response(PdfServiceError::Internal(blocking_err.to_string()))
        }
        Err(_timeout) => {
            log::error!(
                "PDF generation timed out after {} seconds",
                DEFAULT_TIMEOUT_SECS
            );
            build_error_response(PdfServiceError::Timeout(format!(
                "Operation timed out after {} seconds",
                DEFAULT_TIMEOUT_SECS
            )))
        }
    }
}

/// Generate PDF from HTML content.
///
/// This handler converts HTML content directly to PDF without requiring
/// a web server to host the HTML.
///
/// # Endpoint
///
/// ```text
/// POST /pdf/html
/// Content-Type: application/json
/// ```
///
/// # Request Body
///
/// ```json
/// {
///     "html": "<html><body><h1>Hello World</h1></body></html>",
///     "filename": "document.pdf",
///     "waitsecs": 2,
///     "landscape": false,
///     "download": false,
///     "print_background": true
/// }
/// ```
///
/// | Field | Type | Required | Default | Description |
/// |-------|------|----------|---------|-------------|
/// | `html` | string | **Yes** | - | HTML content to convert |
/// | `filename` | string | No | `"document.pdf"` | Output filename |
/// | `waitsecs` | u64 | No | `2` | Seconds to wait for JavaScript |
/// | `landscape` | bool | No | `false` | Use landscape orientation |
/// | `download` | bool | No | `false` | Force download vs inline display |
/// | `print_background` | bool | No | `true` | Include background graphics |
///
/// # Response
///
/// Same as [`pdf_from_url`].
///
/// # Errors
///
/// | Status | Code | Description |
/// |--------|------|-------------|
/// | 400 | `EMPTY_HTML` | HTML content is empty or whitespace |
/// | 502 | `PDF_GENERATION_FAILED` | Failed to generate PDF |
/// | 503 | `BROWSER_UNAVAILABLE` | No browsers available |
/// | 504 | `TIMEOUT` | Operation timed out |
///
/// # Example Request
///
/// ```bash
/// curl -X POST http://localhost:8080/pdf/html \
///   -H "Content-Type: application/json" \
///   -d '{"html": "<h1>Hello</h1>", "filename": "hello.pdf"}' \
///   --output hello.pdf
/// ```
///
/// # Usage in App
///
/// ```rust,ignore
/// App::new()
///     .app_data(web::Data::new(pool.clone()))
///     .route("/pdf/html", web::post().to(pdf_from_html))
/// ```
pub async fn pdf_from_html(
    pool: web::Data<SharedPool>,
    body: web::Json<PdfFromHtmlRequest>,
) -> impl Responder {
    let request = body.into_inner();
    let pool = pool.into_inner();

    log::debug!("PDF from HTML request: {} bytes", request.html.len());

    let result = tokio::time::timeout(
        Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        web::block(move || service::generate_pdf_from_html(&pool, &request)),
    )
    .await;

    match result {
        Ok(Ok(Ok(response))) => build_pdf_response(response),
        Ok(Ok(Err(e))) => build_error_response(e),
        Ok(Err(blocking_err)) => {
            log::error!("Blocking task error: {}", blocking_err);
            build_error_response(PdfServiceError::Internal(blocking_err.to_string()))
        }
        Err(_timeout) => {
            log::error!("PDF generation timed out");
            build_error_response(PdfServiceError::Timeout(format!(
                "Operation timed out after {} seconds",
                DEFAULT_TIMEOUT_SECS
            )))
        }
    }
}

/// Get browser pool statistics.
///
/// Returns real-time metrics about the browser pool including available
/// browsers, active browsers, and total count.
///
/// # Endpoint
///
/// ```text
/// GET /pool/stats
/// ```
///
/// # Response (200 OK)
///
/// ```json
/// {
///     "available": 3,
///     "active": 2,
///     "total": 5
/// }
/// ```
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | `available` | number | Browsers ready to handle requests |
/// | `active` | number | Browsers currently in use |
/// | `total` | number | Total browsers (available + active) |
///
/// # Errors
///
/// | Status | Code | Description |
/// |--------|------|-------------|
/// | 500 | `POOL_LOCK_FAILED` | Failed to acquire pool lock |
///
/// # Use Cases
///
/// - Monitoring dashboards
/// - Prometheus/Grafana metrics
/// - Capacity planning
/// - Debugging pool exhaustion
///
/// # Usage in App
///
/// ```rust,ignore
/// App::new()
///     .app_data(web::Data::new(pool.clone()))
///     .route("/pool/stats", web::get().to(pool_stats))
/// ```
pub async fn pool_stats(pool: web::Data<SharedPool>) -> impl Responder {
    match service::get_pool_stats(&pool) {
        Ok(stats) => HttpResponse::Ok().json(stats),
        Err(e) => build_error_response(e),
    }
}

/// Health check endpoint.
///
/// Simple endpoint that returns 200 OK if the service is running.
/// Does not check pool health - use [`readiness_check`] for that.
///
/// # Endpoint
///
/// ```text
/// GET /health
/// ```
///
/// # Response (200 OK)
///
/// ```json
/// {
///     "status": "healthy",
///     "service": "html2pdf-api"
/// }
/// ```
///
/// # Use Cases
///
/// - Kubernetes liveness probe
/// - Load balancer health check
/// - Uptime monitoring
///
/// # Kubernetes Example
///
/// ```yaml
/// livenessProbe:
///   httpGet:
///     path: /health
///     port: 8080
///   initialDelaySeconds: 10
///   periodSeconds: 30
/// ```
///
/// # Usage in App
///
/// ```rust,ignore
/// App::new()
///     .route("/health", web::get().to(health_check))
/// ```
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse::default())
}

/// Readiness check endpoint.
///
/// Returns 200 OK if the pool has capacity to handle requests,
/// 503 Service Unavailable otherwise.
///
/// Unlike [`health_check`], this actually checks the pool state.
///
/// # Endpoint
///
/// ```text
/// GET /ready
/// ```
///
/// # Response
///
/// ## Ready (200 OK)
///
/// ```json
/// {
///     "status": "ready"
/// }
/// ```
///
/// ## Not Ready (503 Service Unavailable)
///
/// ```json
/// {
///     "status": "not_ready",
///     "reason": "no_available_capacity"
/// }
/// ```
///
/// # Readiness Criteria
///
/// The service is "ready" if either:
/// - There are idle browsers available (`available > 0`), OR
/// - There is capacity to create new browsers (`active < max_pool_size`)
///
/// # Use Cases
///
/// - Kubernetes readiness probe
/// - Load balancer health check (remove from rotation when busy)
/// - Auto-scaling triggers
///
/// # Kubernetes Example
///
/// ```yaml
/// readinessProbe:
///   httpGet:
///     path: /ready
///     port: 8080
///   initialDelaySeconds: 5
///   periodSeconds: 10
/// ```
///
/// # Usage in App
///
/// ```rust,ignore
/// App::new()
///     .app_data(web::Data::new(pool.clone()))
///     .route("/ready", web::get().to(readiness_check))
/// ```
pub async fn readiness_check(pool: web::Data<SharedPool>) -> impl Responder {
    match service::is_pool_ready(&pool) {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ready"
        })),
        Ok(false) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "reason": "no_available_capacity"
        })),
        Err(e) => HttpResponse::ServiceUnavailable().json(ErrorResponse::from(e)),
    }
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure all PDF routes.
///
/// Adds all pre-built handlers to the Actix-web app with default paths.
/// This is the easiest way to set up the PDF service.
///
/// # Routes Added
///
/// | Method | Path | Handler | Description |
/// |--------|------|---------|-------------|
/// | GET | `/pdf` | [`pdf_from_url`] | Convert URL to PDF |
/// | POST | `/pdf/html` | [`pdf_from_html`] | Convert HTML to PDF |
/// | GET | `/pool/stats` | [`pool_stats`] | Pool statistics |
/// | GET | `/health` | [`health_check`] | Health check |
/// | GET | `/ready` | [`readiness_check`] | Readiness check |
///
/// # Example
///
/// ```rust,ignore
/// use actix_web::{App, HttpServer, web};
/// use html2pdf_api::prelude::*;
/// use html2pdf_api::integrations::actix::configure_routes;
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()> {
///     let pool = init_browser_pool().await?;
///
///     HttpServer::new(move || {
///         App::new()
///             .app_data(web::Data::new(pool.clone()))
///             .configure(configure_routes)
///     })
///     .bind("127.0.0.1:8080")?
///     .run()
///     .await
/// }
/// ```
///
/// # Adding Custom Routes
///
/// You can combine `configure_routes` with additional routes:
///
/// ```rust,ignore
/// App::new()
///     .app_data(web::Data::new(pool.clone()))
///     .configure(configure_routes)  // Pre-built routes
///     .route("/custom", web::get().to(my_custom_handler))  // Your routes
/// ```
///
/// # Custom Path Prefix
///
/// To mount routes under a prefix, use `web::scope`:
///
/// ```rust,ignore
/// App::new()
///     .app_data(web::Data::new(pool.clone()))
///     .service(
///         web::scope("/api/v1")
///             .configure(configure_routes)
///     )
/// // Routes will be: /api/v1/pdf, /api/v1/health, etc.
/// ```
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/pdf", web::get().to(pdf_from_url))
        .route("/pdf/html", web::post().to(pdf_from_html))
        .route("/pool/stats", web::get().to(pool_stats))
        .route("/health", web::get().to(health_check))
        .route("/ready", web::get().to(readiness_check));
}

// ============================================================================
// Response Builders (Internal)
// ============================================================================

/// Build HTTP response for successful PDF generation.
fn build_pdf_response(response: crate::service::PdfResponse) -> HttpResponse {
    log::info!(
        "PDF generated successfully: {} bytes, filename={}",
        response.size(),
        response.filename
    );

    HttpResponse::Ok()
        .content_type("application/pdf")
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .insert_header((header::CONTENT_DISPOSITION, response.content_disposition()))
        .body(response.data)
}

/// Build HTTP response for errors.
fn build_error_response(error: PdfServiceError) -> HttpResponse {
    let status_code = error.status_code();
    let body = ErrorResponse::from(&error);

    log::warn!("PDF generation error: {} (HTTP {})", error, status_code);

    match status_code {
        400 => HttpResponse::BadRequest().json(body),
        502 => HttpResponse::BadGateway().json(body),
        503 => HttpResponse::ServiceUnavailable().json(body),
        504 => HttpResponse::GatewayTimeout().json(body),
        _ => HttpResponse::InternalServerError().json(body),
    }
}

// ============================================================================
// Extension Trait (Backward Compatibility)
// ============================================================================

/// Extension trait for `BrowserPool` with Actix-web helpers.
///
/// Provides convenient methods for integrating with Actix-web.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::actix::BrowserPoolActixExt;
///
/// let pool = BrowserPool::builder()
///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
///     .build()?;
///
/// pool.warmup().await?;
///
/// // Convert directly to Actix-web Data
/// let pool_data = pool.into_actix_data();
///
/// HttpServer::new(move || {
///     App::new()
///         .app_data(pool_data.clone())
///         .configure(configure_routes)
/// })
/// ```
pub trait BrowserPoolActixExt {
    /// Convert the pool into Actix-web `Data` wrapper.
    ///
    /// This is equivalent to calling `into_shared()` and then wrapping
    /// with `web::Data::new()`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::integrations::actix::BrowserPoolActixExt;
    ///
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    ///
    /// let pool_data = pool.into_actix_data();
    ///
    /// HttpServer::new(move || {
    ///     App::new()
    ///         .app_data(pool_data.clone())
    /// })
    /// ```
    fn into_actix_data(self) -> BrowserPoolData;
}

impl BrowserPoolActixExt for BrowserPool {
    fn into_actix_data(self) -> BrowserPoolData {
        web::Data::new(self.into_shared())
    }
}

// ============================================================================
// Helper Functions (Backward Compatibility)
// ============================================================================

/// Create Actix-web `Data` from an existing shared pool.
///
/// Use this when you already have a `SharedBrowserPool` and want to
/// wrap it for Actix-web.
///
/// # Parameters
///
/// * `pool` - The shared browser pool.
///
/// # Returns
///
/// `BrowserPoolData` ready for use with `App::app_data()`.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::actix::create_pool_data;
///
/// let shared_pool = pool.into_shared();
/// let pool_data = create_pool_data(shared_pool);
///
/// App::new().app_data(pool_data)
/// ```
pub fn create_pool_data(pool: SharedBrowserPool) -> BrowserPoolData {
    web::Data::new(pool)
}

/// Create Actix-web `Data` from an `Arc` reference.
///
/// Use this when you need to keep a reference to the pool for shutdown.
///
/// # Parameters
///
/// * `pool` - Arc reference to the shared browser pool.
///
/// # Returns
///
/// `BrowserPoolData` ready for use with `App::app_data()`.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::actix::create_pool_data_from_arc;
///
/// let shared_pool = pool.into_shared();
/// let pool_for_shutdown = Arc::clone(&shared_pool);
/// let pool_data = create_pool_data_from_arc(shared_pool);
///
/// // Use pool_data in App
/// // Use pool_for_shutdown for cleanup
/// ```
pub fn create_pool_data_from_arc(pool: Arc<std::sync::Mutex<BrowserPool>>) -> BrowserPoolData {
    web::Data::new(pool)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_alias_compiles() {
        // Verify the type alias is valid
        fn _accepts_pool_data(_: BrowserPoolData) {}
        fn _accepts_shared_pool(_: SharedPool) {}
    }

    #[tokio::test]
    async fn test_shared_pool_type_matches() {
        // SharedPool and SharedBrowserPool should be compatible
        fn _takes_shared_pool(_: SharedPool) {}
        fn _returns_shared_browser_pool() -> SharedBrowserPool {
            Arc::new(std::sync::Mutex::new(
                BrowserPool::builder()
                    .factory(Box::new(crate::factory::mock::MockBrowserFactory::new()))
                    .build()
                    .unwrap(),
            ))
        }

        // This should compile, proving type compatibility
        let pool: SharedBrowserPool = _returns_shared_browser_pool();
        let _: SharedPool = pool;
    }
}
