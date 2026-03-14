//! Axum framework integration.
//!
//! This module provides helpers and pre-built handlers for using `BrowserPool`
//! with Axum. You can choose between using the pre-built handlers for
//! quick setup, or writing custom handlers for full control.
//!
//! # Quick Start
//!
//! ## Option 1: Pre-built Routes (Fastest Setup)
//!
//! Use [`configure_routes`] to add all PDF endpoints with a single line:
//!
//! ```rust,ignore
//! use axum::Router;
//! use html2pdf_api::prelude::*;
//!
//! #[tokio::main]
//! async fn main() {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     let app = Router::new()
//!         .merge(html2pdf_api::integrations::axum::configure_routes())
//!         .with_state(pool);
//!
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
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
//! use axum::{Router, routing::get};
//! use html2pdf_api::prelude::*;
//! use html2pdf_api::integrations::axum::{pdf_from_url, health_check};
//!
//! async fn my_custom_handler() -> &'static str {
//!     "Custom response"
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let pool = init_browser_pool().await.unwrap();
//!
//!     let app = Router::new()
//!         .route("/pdf", get(pdf_from_url))
//!         .route("/health", get(health_check))
//!         .route("/custom", get(my_custom_handler))
//!         .with_state(pool);
//!
//!     // ... serve app
//! }
//! ```
//!
//! ## Option 3: Custom Handlers with Service Functions
//!
//! For full control, use the service functions directly:
//!
//! ```rust,ignore
//! use axum::{
//!     extract::{Query, State},
//!     http::StatusCode,
//!     response::IntoResponse,
//! };
//! use html2pdf_api::prelude::*;
//! use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};
//!
//! async fn my_pdf_handler(
//!     State(pool): State<SharedBrowserPool>,
//!     Query(request): Query<PdfFromUrlRequest>,
//! ) -> impl IntoResponse {
//!     // Call service in blocking context
//!     let result = tokio::task::spawn_blocking(move || {
//!         generate_pdf_from_url(&pool, &request)
//!     }).await;
//!
//!     match result {
//!         Ok(Ok(pdf)) => {
//!             // Custom post-processing
//!             (
//!                 [(axum::http::header::CONTENT_TYPE, "application/pdf")],
//!                 pdf.data,
//!             ).into_response()
//!         }
//!         Ok(Err(_)) => StatusCode::BAD_REQUEST.into_response(),
//!         Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
//!     }
//! }
//! ```
//!
//! ## Option 4: Full Manual Control (Original Approach)
//!
//! For complete control over browser operations:
//!
//! ```rust,ignore
//! use axum::{extract::State, http::StatusCode, response::IntoResponse};
//! use html2pdf_api::prelude::*;
//!
//! async fn manual_pdf_handler(
//!     State(pool): State<SharedBrowserPool>,
//! ) -> Result<impl IntoResponse, StatusCode> {
//!     let pool_guard = pool.lock()
//!         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!
//!     let browser = pool_guard.get()
//!         .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
//!
//!     let tab = browser.new_tab()
//!         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!     tab.navigate_to("https://example.com")
//!         .map_err(|_| StatusCode::BAD_GATEWAY)?;
//!     tab.wait_until_navigated()
//!         .map_err(|_| StatusCode::BAD_GATEWAY)?;
//!
//!     let pdf_data = tab.print_to_pdf(None)
//!         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!
//!     Ok((
//!         [(axum::http::header::CONTENT_TYPE, "application/pdf")],
//!         pdf_data,
//!     ))
//! }
//! ```
//!
//! # Setup
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! html2pdf-api = { version = "0.2", features = ["axum-integration"] }
//! axum = "0.8"
//! ```
//!
//! # Graceful Shutdown
//!
//! For proper cleanup with graceful shutdown:
//!
//! ```rust,ignore
//! use axum::Router;
//! use html2pdf_api::prelude::*;
//! use std::sync::Arc;
//! use tokio::signal;
//!
//! #[tokio::main]
//! async fn main() {
//!     let pool = init_browser_pool().await.unwrap();
//!     let shutdown_pool = Arc::clone(&pool);
//!
//!     let app = Router::new()
//!         .merge(html2pdf_api::integrations::axum::configure_routes())
//!         .with_state(pool);
//!
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
//!     
//!     axum::serve(listener, app)
//!         .with_graceful_shutdown(shutdown_signal(shutdown_pool))
//!         .await
//!         .unwrap();
//! }
//!
//! async fn shutdown_signal(pool: SharedBrowserPool) {
//!     let ctrl_c = async {
//!         signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
//!     };
//!
//!     #[cfg(unix)]
//!     let terminate = async {
//!         signal::unix::signal(signal::unix::SignalKind::terminate())
//!             .expect("Failed to install signal handler")
//!             .recv()
//!             .await;
//!     };
//!
//!     #[cfg(not(unix))]
//!     let terminate = std::future::pending::<()>();
//!
//!     tokio::select! {
//!         _ = ctrl_c => {},
//!         _ = terminate => {},
//!     }
//!
//!     println!("Shutting down...");
//!     if let Ok(mut pool) = pool.lock() {
//!         pool.shutdown();
//!     }
//! }
//! ```

use axum::{
    Router,
    extract::{Json, Query, State},
    http::{
        StatusCode,
        header::{self, HeaderValue},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::sync::Arc;
use std::time::Duration;

use crate::SharedBrowserPool;
use crate::pool::BrowserPool;
use crate::service::{
    self, DEFAULT_TIMEOUT_SECS, ErrorResponse, HealthResponse, PdfFromHtmlRequest,
    PdfFromUrlRequest, PdfResponse, PdfServiceError,
};

// ============================================================================
// Type Aliases
// ============================================================================

/// Type alias for shared browser pool.
///
/// This is the standard pool type used by the service functions.
pub type SharedPool = Arc<BrowserPool>;

/// Type alias for Axum `State` extractor with the shared pool.
///
/// Use this type in your handler parameters:
///
/// ```rust,ignore
/// async fn handler(
///     BrowserPoolState(pool): BrowserPoolState,
/// ) -> impl IntoResponse {
///     let browser = pool.get().unwrap();
///     // ...
/// }
/// ```
pub type BrowserPoolState = State<SharedBrowserPool>;

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
/// # Usage in App
///
/// ```rust,ignore
/// Router::new().route("/pdf", get(pdf_from_url)).with_state(pool)
/// ```
pub async fn pdf_from_url(
    State(pool): State<SharedPool>,
    Query(request): Query<PdfFromUrlRequest>,
) -> Response {
    let pool_arc = Arc::clone(&pool);

    log::debug!("PDF from URL request: {}", request.url);

    // Run blocking PDF generation with timeout
    let result = tokio::time::timeout(
        Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || service::generate_pdf_from_url(&pool_arc, &request)),
    )
    .await;

    match result {
        Ok(Ok(Ok(response))) => build_pdf_response(response),
        Ok(Ok(Err(e))) => build_error_response(e),
        Ok(Err(join_err)) => {
            log::error!("Blocking task error: {}", join_err);
            build_error_response(PdfServiceError::Internal(join_err.to_string()))
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
/// # Usage in App
///
/// ```rust,ignore
/// Router::new().route("/pdf/html", post(pdf_from_html)).with_state(pool)
/// ```
pub async fn pdf_from_html(
    State(pool): State<SharedPool>,
    Json(request): Json<PdfFromHtmlRequest>,
) -> Response {
    let pool_arc = Arc::clone(&pool);

    log::debug!("PDF from HTML request: {} bytes", request.html.len());

    let result = tokio::time::timeout(
        Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || service::generate_pdf_from_html(&pool_arc, &request)),
    )
    .await;

    match result {
        Ok(Ok(Ok(response))) => build_pdf_response(response),
        Ok(Ok(Err(e))) => build_error_response(e),
        Ok(Err(join_err)) => {
            log::error!("Blocking task error: {}", join_err);
            build_error_response(PdfServiceError::Internal(join_err.to_string()))
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
pub async fn pool_stats(State(pool): State<SharedPool>) -> Response {
    match service::get_pool_stats(&pool) {
        Ok(stats) => Json(stats).into_response(),
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
pub async fn health_check() -> Response {
    Json(HealthResponse::default()).into_response()
}

/// Readiness check endpoint.
///
/// Returns 200 OK if the pool has capacity to handle requests,
/// 503 Service Unavailable otherwise.
///
/// # Endpoint
///
/// ```text
/// GET /ready
/// ```
pub async fn readiness_check(State(pool): State<SharedPool>) -> Response {
    match service::is_pool_ready(&pool) {
        Ok(true) => Json(serde_json::json!({ "status": "ready" })).into_response(),
        Ok(false) => {
            let body = Json(serde_json::json!({
                "status": "not_ready",
                "reason": "no_available_capacity"
            }));
            (StatusCode::SERVICE_UNAVAILABLE, body).into_response()
        }
        Err(e) => {
            let body = Json(ErrorResponse::from(e));
            (StatusCode::SERVICE_UNAVAILABLE, body).into_response()
        }
    }
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Returns a router configured with all PDF routes.
///
/// Provides all pre-built handlers ready to be merged into a main router.
/// This is the easiest way to set up the PDF service in Axum.
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
/// use axum::Router;
/// use html2pdf_api::integrations::axum::configure_routes;
///
/// let app = Router::new()
///     .merge(configure_routes())
///     .with_state(pool);
/// ```
pub fn configure_routes() -> Router<SharedPool> {
    Router::new()
        .route("/pdf", get(pdf_from_url))
        .route("/pdf/html", post(pdf_from_html))
        .route("/pool/stats", get(pool_stats))
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
}

// ============================================================================
// Response Builders (Internal)
// ============================================================================

/// Build HTTP response for successful PDF generation.
fn build_pdf_response(response: PdfResponse) -> Response {
    log::info!(
        "PDF generated successfully: {} bytes, filename={}",
        response.size(),
        response.filename
    );

    let content_disposition = response.content_disposition();
    let mut res = response.data.into_response();

    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));

    if let Ok(val) = HeaderValue::from_str(&content_disposition) {
        res.headers_mut().insert(header::CONTENT_DISPOSITION, val);
    }

    res
}

/// Build HTTP response for errors.
fn build_error_response(error: PdfServiceError) -> Response {
    let status_code =
        StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = ErrorResponse::from(&error);

    log::warn!("PDF generation error: {} (HTTP {})", error, status_code);

    (status_code, Json(body)).into_response()
}

// ============================================================================
// Extension Traits
// ============================================================================

/// Extension trait for `BrowserPool` with Axum helpers.
///
/// Provides convenient methods for integrating with Axum.
pub trait BrowserPoolAxumExt {
    /// Convert the pool into a form suitable for Axum's `with_state()`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let state = pool.into_axum_state();
    /// Router::new().route("/pdf", get(generate_pdf)).with_state(state)
    /// ```
    fn into_axum_state(self) -> SharedBrowserPool;

    /// Convert the pool into an Extension layer.
    fn into_axum_extension(self) -> axum::Extension<SharedBrowserPool>;
}

impl BrowserPoolAxumExt for BrowserPool {
    fn into_axum_state(self) -> SharedBrowserPool {
        self.into_shared()
    }

    fn into_axum_extension(self) -> axum::Extension<SharedBrowserPool> {
        axum::Extension(self.into_shared())
    }
}

/// Create an Axum Extension from an existing shared pool.
pub fn create_extension(pool: SharedBrowserPool) -> axum::Extension<SharedBrowserPool> {
    axum::Extension(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_alias_compiles() {
        // This test just verifies the type alias is valid
        fn _accepts_pool_state(_: BrowserPoolState) {}
    }
}
