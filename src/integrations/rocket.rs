//! Rocket framework integration.
//!
//! This module provides helpers and pre-built handlers for using `BrowserPool`
//! with Rocket. You can choose between using the pre-built handlers for
//! quick setup, or writing custom handlers for full control.
//!
//! # Quick Start
//!
//! ## Option 1: Pre-built Routes (Fastest Setup)
//!
//! Use [`configure_routes`] to add all PDF endpoints with a single line:
//!
//! ```rust,ignore
//! use rocket::launch;
//! use html2pdf_api::prelude::*;
//!
//! #[launch]
//! async fn rocket() -> _ {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     rocket::build()
//!         .manage(pool)
//!         .configure(html2pdf_api::integrations::rocket::configure_routes)
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
//! use rocket::{get, launch, routes};
//! use html2pdf_api::prelude::*;
//! use html2pdf_api::integrations::rocket::{pdf_from_url, health_check};
//!
//! #[get("/custom")]
//! fn my_custom_handler() -> &'static str {
//!     "Custom response"
//! }
//!
//! #[launch]
//! async fn rocket() -> _ {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     rocket::build()
//!         .manage(pool)
//!         .mount("/", routes![
//!             pdf_from_url,
//!             health_check,
//!             my_custom_handler
//!         ])
//! }
//! ```
//!
//! ## Option 3: Custom Handlers with Service Functions
//!
//! For full control, use the service functions directly:
//!
//! ```rust,ignore
//! use rocket::{get, http::Status, serde::json::Json, State};
//! use html2pdf_api::prelude::*;
//! use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};
//!
//! #[get("/my-pdf?<url>")]
//! async fn my_pdf_handler(
//!     pool: &State<SharedBrowserPool>,
//!     url: String,
//! ) -> Result<Vec<u8>, Status> {
//!     // Custom pre-processing: auth, rate limiting, logging, etc.
//!     log::info!("Custom handler: {}", url);
//!
//!     let pool = pool.inner().clone();
//!     let request = PdfFromUrlRequest {
//!         url,
//!         filename: Some("custom.pdf".to_string()),
//!         ..Default::default()
//!     };
//!
//!     // Call service in blocking context
//!     let result = tokio::task::spawn_blocking(move || {
//!         generate_pdf_from_url(&pool, &request)
//!     }).await;
//!
//!     match result {
//!         Ok(Ok(pdf)) => {
//!             // Custom post-processing
//!             Ok(pdf.data)
//!         }
//!         Ok(Err(_)) => Err(Status::BadRequest),
//!         Err(_) => Err(Status::InternalServerError),
//!     }
//! }
//! ```
//!
//! ## Option 4: Full Manual Control (Original Approach)
//!
//! For complete control over browser operations:
//!
//! ```rust,ignore
//! use rocket::{get, http::Status, State};
//! use html2pdf_api::prelude::*;
//!
//! #[get("/manual-pdf")]
//! fn manual_pdf_handler(
//!     pool: &State<SharedBrowserPool>,
//! ) -> Result<Vec<u8>, Status> {
//!     let pool_guard = pool.lock()
//!         .map_err(|_| Status::InternalServerError)?;
//!
//!     let browser = pool_guard.get()
//!         .map_err(|_| Status::ServiceUnavailable)?;
//!
//!     let tab = browser.new_tab()
//!         .map_err(|_| Status::InternalServerError)?;
//!     tab.navigate_to("https://example.com")
//!         .map_err(|_| Status::BadGateway)?;
//!     tab.wait_until_navigated()
//!         .map_err(|_| Status::BadGateway)?;
//!
//!     let pdf_data = tab.print_to_pdf(None)
//!         .map_err(|_| Status::InternalServerError)?;
//!
//!     Ok(pdf_data)
//! }
//! ```
//!
//! # Setup
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! html2pdf-api = { version = "0.2", features = ["rocket-integration"] }
//! rocket = { version = "0.5", features = ["json"] }
//! ```
//!
//! # Graceful Shutdown
//!
//! For proper cleanup, use Rocket's shutdown fairing:
//!
//! ```rust,ignore
//! use rocket::{fairing::{Fairing, Info, Kind}, launch, Orbit, Rocket};
//! use html2pdf_api::prelude::*;
//! use std::sync::Arc;
//!
//! struct ShutdownFairing {
//!     pool: SharedBrowserPool,
//! }
//!
//! #[rocket::async_trait]
//! impl Fairing for ShutdownFairing {
//!     fn info(&self) -> Info {
//!         Info {
//!             name: "Browser Pool Shutdown",
//!             kind: Kind::Shutdown,
//!         }
//!     }
//!
//!     async fn on_shutdown(&self, _rocket: &Rocket<Orbit>) {
//!         if let Ok(mut pool) = self.pool.lock() {
//!             pool.shutdown();
//!         }
//!     }
//! }
//!
//! #[launch]
//! async fn rocket() -> _ {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     let shutdown_pool = pool.clone();
//!
//!     rocket::build()
//!         .manage(pool)
//!         .attach(ShutdownFairing { pool: shutdown_pool })
//!         .configure(html2pdf_api::integrations::rocket::configure_routes)
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
//! | [`BrowserPoolState`] | `&State<SharedBrowserPool>` - for handler parameters |
//!
//! ## Helper Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | [`configure_routes`] | Configure all pre-built routes |
//! | [`routes()`] | Get all routes for manual mounting |
//! | [`create_pool_data`] | Wrap `SharedBrowserPool` for Rocket managed state |
//! | [`create_pool_data_from_arc`] | Wrap `Arc<Mutex<BrowserPool>>` for managed state |
//!
//! ## Extension Traits
//!
//! | Trait | Description |
//! |-------|-------------|
//! | [`BrowserPoolRocketExt`] | Adds `into_rocket_data()` to `BrowserPool` |

use rocket::{
    Build, Request, Rocket, State,
    form::FromForm,
    get,
    http::{ContentType, Header, Status},
    post,
    response::{self, Responder},
    routes,
    serde::json::Json,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::SharedBrowserPool;
use crate::pool::BrowserPool;
use crate::service::{
    self, DEFAULT_TIMEOUT_SECS, ErrorResponse, HealthResponse, PdfFromHtmlRequest,
    PdfFromUrlRequest, PdfResponse, PdfServiceError, PoolStatsResponse,
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
/// use html2pdf_api::integrations::rocket::SharedPool;
///
/// fn my_function(pool: &SharedPool) {
///     let guard = pool.lock().unwrap();
///     let browser = guard.get().unwrap();
///     // ...
/// }
/// ```
pub type SharedPool = Arc<Mutex<BrowserPool>>;

/// Type alias for Rocket `State` wrapper around the shared pool.
///
/// Use this type in your handler parameters for automatic extraction:
///
/// ```rust,ignore
/// use rocket::State;
/// use html2pdf_api::integrations::rocket::BrowserPoolState;
///
/// #[get("/handler")]
/// fn handler(pool: BrowserPoolState<'_>) -> &'static str {
///     let pool_guard = pool.lock().unwrap();
///     let browser = pool_guard.get().unwrap();
///     // ...
///     "done"
/// }
/// ```
///
/// # Note
///
/// In Rocket, `State<T>` is accessed as a reference in handlers,
/// so this type alias represents the borrowed form.
pub type BrowserPoolState<'r> = &'r State<SharedBrowserPool>;

// ============================================================================
// Query Parameter Types (Rocket uses FromForm instead of serde::Deserialize)
// ============================================================================

/// Query parameters for PDF from URL endpoint.
///
/// This struct uses Rocket's `FromForm` trait for automatic deserialization
/// from query strings, similar to Actix-web's `web::Query<T>`.
///
/// # Example
///
/// ```text
/// GET /pdf?url=https://example.com&filename=doc.pdf&landscape=true
/// ```
#[derive(Debug, FromForm)]
pub struct PdfFromUrlQuery {
    /// URL to convert to PDF (required).
    pub url: String,
    /// Output filename (optional, defaults to "document.pdf").
    pub filename: Option<String>,
    /// Seconds to wait for JavaScript execution (optional, defaults to 5).
    pub waitsecs: Option<u64>,
    /// Use landscape orientation (optional, defaults to false).
    pub landscape: Option<bool>,
    /// Force download instead of inline display (optional, defaults to false).
    pub download: Option<bool>,
    /// Include background graphics (optional, defaults to true).
    pub print_background: Option<bool>,
}

impl From<PdfFromUrlQuery> for PdfFromUrlRequest {
    fn from(query: PdfFromUrlQuery) -> Self {
        Self {
            url: query.url,
            filename: query.filename,
            waitsecs: query.waitsecs,
            landscape: query.landscape,
            download: query.download,
            print_background: query.print_background,
        }
    }
}

// ============================================================================
// Custom Response Types
// ============================================================================

/// PDF response wrapper for Rocket.
///
/// This responder automatically sets the correct headers for PDF responses:
/// - `Content-Type: application/pdf`
/// - `Content-Disposition: inline` or `attachment` based on `force_download`
/// - `Cache-Control: no-cache`
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::rocket::PdfResponder;
///
/// fn create_pdf_response(data: Vec<u8>) -> PdfResponder {
///     PdfResponder {
///         data,
///         filename: "document.pdf".to_string(),
///         force_download: false,
///     }
/// }
/// ```
pub struct PdfResponder {
    /// The PDF binary data.
    pub data: Vec<u8>,
    /// The filename to suggest to the browser.
    pub filename: String,
    /// Whether to force download (attachment) or allow inline display.
    pub force_download: bool,
}

impl<'r> Responder<'r, 'static> for PdfResponder {
    fn respond_to(self, _request: &'r Request<'_>) -> response::Result<'static> {
        let disposition = if self.force_download {
            format!("attachment; filename=\"{}\"", self.filename)
        } else {
            format!("inline; filename=\"{}\"", self.filename)
        };

        response::Response::build()
            .header(ContentType::PDF)
            .header(Header::new("Cache-Control", "no-cache"))
            .header(Header::new("Content-Disposition", disposition))
            .sized_body(self.data.len(), std::io::Cursor::new(self.data))
            .ok()
    }
}

/// Error response wrapper for Rocket.
///
/// This responder automatically sets the correct HTTP status code based on
/// the error type and returns a JSON error body.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::rocket::ErrorResponder;
/// use html2pdf_api::service::ErrorResponse;
/// use rocket::http::Status;
///
/// fn create_error(msg: &str) -> ErrorResponder {
///     ErrorResponder {
///         status: Status::BadRequest,
///         body: ErrorResponse {
///             error: msg.to_string(),
///             code: "INVALID_REQUEST".to_string(),
///         },
///     }
/// }
/// ```
pub struct ErrorResponder {
    /// The HTTP status code.
    pub status: Status,
    /// The JSON error body.
    pub body: ErrorResponse,
}

impl<'r> Responder<'r, 'static> for ErrorResponder {
    fn respond_to(self, request: &'r Request<'_>) -> response::Result<'static> {
        response::Response::build_from(Json(self.body).respond_to(request)?)
            .status(self.status)
            .ok()
    }
}

/// Result type for Rocket handlers.
///
/// All pre-built handlers return this type, making it easy to use
/// `?` operator for error handling in custom code.
pub type HandlerResult<T> = Result<T, ErrorResponder>;

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
/// rocket::build()
///     .manage(pool)
///     .mount("/", routes![pdf_from_url])
/// ```
#[get("/pdf?<query..>")]
pub async fn pdf_from_url(
    pool: &State<SharedPool>,
    query: PdfFromUrlQuery,
) -> HandlerResult<PdfResponder> {
    let request: PdfFromUrlRequest = query.into();
    let pool = Arc::clone(pool.inner());

    log::debug!("PDF from URL request: {}", request.url);

    // Run blocking PDF generation with timeout
    let result = tokio::time::timeout(
        Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || service::generate_pdf_from_url(&pool, &request)),
    )
    .await;

    match result {
        Ok(Ok(Ok(response))) => Ok(build_pdf_response(response)),
        Ok(Ok(Err(e))) => Err(build_error_response(e)),
        Ok(Err(join_err)) => {
            log::error!("Blocking task error: {}", join_err);
            Err(build_error_response(PdfServiceError::Internal(
                join_err.to_string(),
            )))
        }
        Err(_timeout) => {
            log::error!(
                "PDF generation timed out after {} seconds",
                DEFAULT_TIMEOUT_SECS
            );
            Err(build_error_response(PdfServiceError::Timeout(format!(
                "Operation timed out after {} seconds",
                DEFAULT_TIMEOUT_SECS
            ))))
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
/// curl -X POST http://localhost:8000/pdf/html \
///   -H "Content-Type: application/json" \
///   -d '{"html": "<h1>Hello</h1>", "filename": "hello.pdf"}' \
///   --output hello.pdf
/// ```
///
/// # Usage in App
///
/// ```rust,ignore
/// rocket::build()
///     .manage(pool)
///     .mount("/", routes![pdf_from_html])
/// ```
#[post("/pdf/html", data = "<body>")]
pub async fn pdf_from_html(
    pool: &State<SharedPool>,
    body: Json<PdfFromHtmlRequest>,
) -> HandlerResult<PdfResponder> {
    let request = body.into_inner();
    let pool = Arc::clone(pool.inner());

    log::debug!("PDF from HTML request: {} bytes", request.html.len());

    // Run blocking PDF generation with timeout
    let result = tokio::time::timeout(
        Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || service::generate_pdf_from_html(&pool, &request)),
    )
    .await;

    match result {
        Ok(Ok(Ok(response))) => Ok(build_pdf_response(response)),
        Ok(Ok(Err(e))) => Err(build_error_response(e)),
        Ok(Err(join_err)) => {
            log::error!("Blocking task error: {}", join_err);
            Err(build_error_response(PdfServiceError::Internal(
                join_err.to_string(),
            )))
        }
        Err(_timeout) => {
            log::error!("PDF generation timed out");
            Err(build_error_response(PdfServiceError::Timeout(format!(
                "Operation timed out after {} seconds",
                DEFAULT_TIMEOUT_SECS
            ))))
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
/// rocket::build()
///     .manage(pool)
///     .mount("/", routes![pool_stats])
/// ```
#[get("/pool/stats")]
pub fn pool_stats(pool: &State<SharedPool>) -> HandlerResult<Json<PoolStatsResponse>> {
    service::get_pool_stats(pool.inner())
        .map(Json)
        .map_err(build_error_response)
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
///     port: 8000
///   initialDelaySeconds: 10
///   periodSeconds: 30
/// ```
///
/// # Usage in App
///
/// ```rust,ignore
/// rocket::build()
///     .mount("/", routes![health_check])
/// ```
#[get("/health")]
pub fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse::default())
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
///     port: 8000
///   initialDelaySeconds: 5
///   periodSeconds: 10
/// ```
///
/// # Usage in App
///
/// ```rust,ignore
/// rocket::build()
///     .manage(pool)
///     .mount("/", routes![readiness_check])
/// ```
#[get("/ready")]
pub fn readiness_check(
    pool: &State<SharedPool>,
) -> Result<Json<serde_json::Value>, ErrorResponder> {
    match service::is_pool_ready(pool.inner()) {
        Ok(true) => Ok(Json(serde_json::json!({
            "status": "ready"
        }))),
        Ok(false) => Err(ErrorResponder {
            status: Status::ServiceUnavailable,
            body: ErrorResponse {
                error: "No available capacity".to_string(),
                code: "NOT_READY".to_string(),
            },
        }),
        Err(e) => Err(build_error_response(e)),
    }
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure all PDF routes.
///
/// Adds all pre-built handlers to the Rocket instance with default paths.
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
/// use rocket::launch;
/// use html2pdf_api::prelude::*;
/// use html2pdf_api::integrations::rocket::configure_routes;
///
/// #[launch]
/// async fn rocket() -> _ {
///     let pool = init_browser_pool().await
///         .expect("Failed to initialize browser pool");
///
///     rocket::build()
///         .manage(pool)
///         .configure(configure_routes)
/// }
/// ```
///
/// # Adding Custom Routes
///
/// You can combine `configure_routes` with additional routes:
///
/// ```rust,ignore
/// use rocket::{get, routes};
///
/// #[get("/custom")]
/// fn my_custom_handler() -> &'static str { "custom" }
///
/// rocket::build()
///     .manage(pool)
///     .configure(configure_routes)  // Pre-built routes
///     .mount("/", routes![my_custom_handler])  // Your routes
/// ```
///
/// # Custom Path Prefix
///
/// To mount routes under a prefix, use [`routes()`] directly:
///
/// ```rust,ignore
/// use html2pdf_api::integrations::rocket::routes;
///
/// rocket::build()
///     .manage(pool)
///     .mount("/api/v1", routes())
/// // Routes will be: /api/v1/pdf, /api/v1/health, etc.
/// ```
pub fn configure_routes(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes())
}

/// Get all routes for manual mounting.
///
/// Returns a vector of all pre-built routes, allowing you to mount them
/// at a custom path prefix.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::rocket::routes;
///
/// // Mount at root
/// rocket::build().mount("/", routes())
///
/// // Mount at custom prefix
/// rocket::build().mount("/api/v1", routes())
/// ```
///
/// # Routes Returned
///
/// - `GET /pdf` - [`pdf_from_url`]
/// - `POST /pdf/html` - [`pdf_from_html`]
/// - `GET /pool/stats` - [`pool_stats`]
/// - `GET /health` - [`health_check`]
/// - `GET /ready` - [`readiness_check`]
pub fn routes() -> Vec<rocket::Route> {
    routes![
        pdf_from_url,
        pdf_from_html,
        pool_stats,
        health_check,
        readiness_check
    ]
}

// ============================================================================
// Response Builders (Internal)
// ============================================================================

/// Build PDF responder for successful PDF generation.
fn build_pdf_response(response: PdfResponse) -> PdfResponder {
    log::info!(
        "PDF generated successfully: {} bytes, filename={}",
        response.size(),
        response.filename
    );

    PdfResponder {
        data: response.data,
        filename: response.filename,
        force_download: response.force_download,
    }
}

/// Build error responder from service error.
fn build_error_response(error: PdfServiceError) -> ErrorResponder {
    let status = match error.status_code() {
        400 => Status::BadRequest,
        502 => Status::BadGateway,
        503 => Status::ServiceUnavailable,
        504 => Status::GatewayTimeout,
        _ => Status::InternalServerError,
    };

    log::warn!("PDF generation error: {} (HTTP {})", error, status.code);

    ErrorResponder {
        status,
        body: ErrorResponse::from(error),
    }
}

// ============================================================================
// Extension Trait
// ============================================================================

/// Extension trait for `BrowserPool` with Rocket helpers.
///
/// Provides convenient methods for integrating with Rocket's managed state.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::rocket::BrowserPoolRocketExt;
///
/// let pool = BrowserPool::builder()
///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
///     .build()?;
///
/// pool.warmup().await?;
///
/// // Convert directly to Rocket managed state
/// let shared_pool = pool.into_rocket_data();
///
/// rocket::build()
///     .manage(shared_pool)
///     .configure(configure_routes)
/// ```
pub trait BrowserPoolRocketExt {
    /// Convert the pool into a shared reference suitable for Rocket's managed state.
    ///
    /// This is equivalent to calling `into_shared()`, returning an
    /// `Arc<Mutex<BrowserPool>>` that can be passed to `rocket.manage()`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::integrations::rocket::BrowserPoolRocketExt;
    ///
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    ///
    /// let shared_pool = pool.into_rocket_data();
    ///
    /// rocket::build()
    ///     .manage(shared_pool)
    ///     .mount("/", routes())
    /// ```
    fn into_rocket_data(self) -> SharedBrowserPool;
}

impl BrowserPoolRocketExt for BrowserPool {
    fn into_rocket_data(self) -> SharedBrowserPool {
        self.into_shared()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create Rocket managed state from an existing shared pool.
///
/// Use this when you already have a `SharedBrowserPool` and want to
/// use it with Rocket's `manage()`.
///
/// # Parameters
///
/// * `pool` - The shared browser pool.
///
/// # Returns
///
/// `SharedBrowserPool` ready for use with `rocket.manage()`.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::rocket::create_pool_data;
///
/// let shared_pool = pool.into_shared();
/// let pool_data = create_pool_data(shared_pool);
///
/// rocket::build().manage(pool_data)
/// ```
pub fn create_pool_data(pool: SharedBrowserPool) -> SharedBrowserPool {
    pool
}

/// Create Rocket managed state from an `Arc` reference.
///
/// Use this when you need to keep a reference to the pool for shutdown.
///
/// # Parameters
///
/// * `pool` - Arc reference to the shared browser pool.
///
/// # Returns
///
/// Cloned `SharedBrowserPool` ready for use with `rocket.manage()`.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::rocket::create_pool_data_from_arc;
///
/// let shared_pool = pool.into_shared();
/// let pool_for_shutdown = Arc::clone(&shared_pool);
/// let pool_data = create_pool_data_from_arc(shared_pool);
///
/// // Use pool_data in rocket.manage()
/// // Use pool_for_shutdown for cleanup in shutdown fairing
/// ```
pub fn create_pool_data_from_arc(pool: Arc<Mutex<BrowserPool>>) -> SharedBrowserPool {
    pool
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
        fn _accepts_shared_pool(_: SharedPool) {}
    }

    #[test]
    fn test_error_responder_status_mapping() {
        let test_cases = vec![
            (
                PdfServiceError::InvalidUrl("".to_string()),
                Status::BadRequest,
            ),
            (
                PdfServiceError::NavigationFailed("".to_string()),
                Status::BadGateway,
            ),
            (
                PdfServiceError::BrowserUnavailable("".to_string()),
                Status::ServiceUnavailable,
            ),
            (
                PdfServiceError::Timeout("".to_string()),
                Status::GatewayTimeout,
            ),
            (
                PdfServiceError::Internal("".to_string()),
                Status::InternalServerError,
            ),
        ];

        for (error, expected_status) in test_cases {
            let responder = build_error_response(error);
            assert_eq!(responder.status, expected_status);
        }
    }

    #[test]
    fn test_pdf_from_url_query_conversion() {
        let query = PdfFromUrlQuery {
            url: "https://example.com".to_string(),
            filename: Some("test.pdf".to_string()),
            waitsecs: Some(10),
            landscape: Some(true),
            download: Some(false),
            print_background: Some(true),
        };

        let request: PdfFromUrlRequest = query.into();

        assert_eq!(request.url, "https://example.com");
        assert_eq!(request.filename, Some("test.pdf".to_string()));
        assert_eq!(request.waitsecs, Some(10));
        assert_eq!(request.landscape, Some(true));
        assert_eq!(request.download, Some(false));
        assert_eq!(request.print_background, Some(true));
    }

    #[tokio::test]
    async fn test_shared_pool_type_matches() {
        // SharedPool and SharedBrowserPool should be compatible
        fn _takes_shared_pool(_: SharedPool) {}
        fn _returns_shared_browser_pool() -> SharedBrowserPool {
            Arc::new(Mutex::new(
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

    #[test]
    fn test_routes_returns_all_endpoints() {
        let all_routes = routes();
        assert_eq!(all_routes.len(), 5);
    }
}
