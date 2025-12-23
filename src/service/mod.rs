//! PDF generation service module.
//!
//! This module provides the **framework-agnostic core** of the PDF generation
//! service. It contains shared types, error definitions, and the core PDF
//! generation logic that is reused across all web framework integrations.
//!
//! # Module Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                        html2pdf-api crate                               │
//! │                                                                         │
//! │  ┌───────────────────────────────────────────────────────────────────┐  │
//! │  │                    service module (this module)                   │  │
//! │  │                                                                   │  │
//! │  │  ┌─────────────────────────┐  ┌─────────────────────────────────┐ │  │
//! │  │  │      types.rs           │  │          pdf.rs                 │ │  │
//! │  │  │  ┌───────────────────┐  │  │  ┌───────────────────────────┐  │ │  │
//! │  │  │  │ PdfFromUrlRequest │  │  │  │ generate_pdf_from_url()   │  │ │  │
//! │  │  │  │ PdfFromHtmlRequest│  │  │  │ generate_pdf_from_html()  │  │ │  │
//! │  │  │  │ PdfResponse       │  │  │  │ get_pool_stats()          │  │ │  │
//! │  │  │  │ PdfServiceError   │  │  │  │ is_pool_ready()           │  │ │  │
//! │  │  │  │ ErrorResponse     │  │  │  └───────────────────────────┘  │ │  │
//! │  │  │  │ PoolStatsResponse │  │  │                                 │ │  │
//! │  │  │  │ HealthResponse    │  │  │                                 │ │  │
//! │  │  │  └───────────────────┘  │  │                                 │ │  │
//! │  │  └─────────────────────────┘  └─────────────────────────────────┘ │  │
//! │  └───────────────────────────────────────────────────────────────────┘  │
//! │                                    │                                    │
//! │                                    │ used by                            │
//! │                                    ▼                                    │
//! │  ┌───────────────────────────────────────────────────────────────────┐  │
//! │  │                    integrations module                            │  │
//! │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │  │
//! │  │  │  actix.rs   │  │  rocket.rs  │  │   axum.rs   │                │  │
//! │  │  │  (handlers) │  │  (handlers) │  │  (handlers) │                │  │
//! │  │  └─────────────┘  └─────────────┘  └─────────────┘                │  │
//! │  └───────────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Design Philosophy
//!
//! This module follows the **"thin handler, thick service"** pattern:
//!
//! | Layer | Responsibility | This Module? |
//! |-------|----------------|--------------|
//! | **Service** | Core business logic, validation, PDF generation | ✅ Yes |
//! | **Handler** | HTTP request/response mapping, framework glue | ❌ No (integrations) |
//!
//! Benefits of this design:
//! - **Single source of truth** for PDF generation logic
//! - **Easy testing** without HTTP overhead
//! - **Framework flexibility** - add new frameworks without duplicating logic
//! - **Type safety** - shared types ensure consistency across integrations
//!
//! # Public API Summary
//!
//! ## Request Types
//!
//! | Type | Purpose | Used By |
//! |------|---------|---------|
//! | `PdfFromUrlRequest` | Parameters for URL → PDF conversion | `GET /pdf` |
//! | `PdfFromHtmlRequest` | Parameters for HTML → PDF conversion | `POST /pdf/html` |
//!
//! ## Response Types
//!
//! | Type | Purpose | Used By |
//! |------|---------|---------|
//! | `PdfResponse` | Successful PDF generation result | PDF endpoints |
//! | `PoolStatsResponse` | Browser pool statistics | `GET /pool/stats` |
//! | `HealthResponse` | Health check response | `GET /health` |
//! | `ErrorResponse` | JSON error response | All endpoints (on error) |
//!
//! ## Error Types
//!
//! | Type | Purpose |
//! |------|---------|
//! | `PdfServiceError` | All possible service errors with HTTP status mapping |
//!
//! ## Core Functions
//!
//! | Function | Purpose | Blocking? |
//! |----------|---------|-----------|
//! | `generate_pdf_from_url` | Convert URL to PDF | ⚠️ Yes |
//! | `generate_pdf_from_html` | Convert HTML to PDF | ⚠️ Yes |
//! | `get_pool_stats` | Get pool statistics | ✅ Fast |
//! | `is_pool_ready` | Check pool readiness | ✅ Fast |
//!
//! ## Constants
//!
//! | Constant | Value | Purpose |
//! |----------|-------|---------|
//! | `DEFAULT_TIMEOUT_SECS` | 60 | Overall operation timeout |
//! | `DEFAULT_WAIT_SECS` | 5 | JavaScript wait time |
//!
//! # Usage Patterns
//!
//! ## Pattern 1: Use Pre-built Framework Integration (Recommended)
//!
//! The easiest way to use this library is via the pre-built integrations:
//!
//! ```rust,ignore
//! use actix_web::{App, HttpServer, web};
//! use html2pdf_api::prelude::*;
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let pool = init_browser_pool().await?;
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
//! ## Pattern 2: Custom Handlers with Service Functions
//!
//! For custom behavior, use the service functions directly:
//!
//! ```rust,ignore
//! use actix_web::{web, HttpResponse};
//! use html2pdf_api::service::{
//!     generate_pdf_from_url, PdfFromUrlRequest, PdfServiceError
//! };
//! use std::sync::{Arc, Mutex};
//!
//! async fn custom_pdf_handler(
//!     pool: web::Data<Arc<Mutex<BrowserPool>>>,
//!     query: web::Query<PdfFromUrlRequest>,
//! ) -> HttpResponse {
//!     // Add custom logic: authentication, rate limiting, logging, etc.
//!     log::info!("Custom handler called for: {}", query.url);
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
//!             // Add custom headers, transform response, etc.
//!             HttpResponse::Ok()
//!                 .content_type("application/pdf")
//!                 .insert_header(("X-Custom-Header", "value"))
//!                 .body(pdf.data)
//!         }
//!         Ok(Err(e)) => {
//!             // Custom error handling
//!             HttpResponse::build(http::StatusCode::from_u16(e.status_code()).unwrap())
//!                 .json(serde_json::json!({
//!                     "error": e.to_string(),
//!                     "code": e.error_code(),
//!                     "request_id": "custom-id-123"
//!                 }))
//!         }
//!         Err(e) => {
//!             HttpResponse::InternalServerError().body(e.to_string())
//!         }
//!     }
//! }
//! ```
//!
//! ## Pattern 3: Direct Service Usage (Non-HTTP)
//!
//! For CLI tools, batch processing, or testing:
//!
//! ```rust,ignore
//! use html2pdf_api::service::{
//!     generate_pdf_from_url, generate_pdf_from_html,
//!     PdfFromUrlRequest, PdfFromHtmlRequest,
//! };
//! use std::sync::Mutex;
//!
//! fn batch_convert(pool: &Mutex<BrowserPool>, urls: Vec<String>) -> Vec<Result<Vec<u8>, PdfServiceError>> {
//!     urls.into_iter()
//!         .map(|url| {
//!             let request = PdfFromUrlRequest {
//!                 url,
//!                 landscape: Some(true),
//!                 ..Default::default()
//!             };
//!             generate_pdf_from_url(pool, &request).map(|r| r.data)
//!         })
//!         .collect()
//! }
//!
//! fn generate_report(pool: &Mutex<BrowserPool>, html: String) -> Result<(), Box<dyn std::error::Error>> {
//!     let request = PdfFromHtmlRequest {
//!         html,
//!         filename: Some("report.pdf".to_string()),
//!         ..Default::default()
//!     };
//!
//!     let response = generate_pdf_from_html(pool, &request)?;
//!     std::fs::write("report.pdf", &response.data)?;
//!     println!("Generated report: {} bytes", response.size());
//!     Ok(())
//! }
//! ```
//!
//! # Blocking Behavior
//!
//! ⚠️ **Important:** The PDF generation functions (`generate_pdf_from_url` and
//! `generate_pdf_from_html`) are **blocking** and should never be called directly
//! from an async context.
//!
//! ## Correct Usage
//!
//! ```rust,ignore
//! // ✅ Actix-web: Use web::block
//! let result = web::block(move || {
//!     generate_pdf_from_url(&pool, &request)
//! }).await;
//!
//! // ✅ Tokio: Use spawn_blocking
//! let result = tokio::task::spawn_blocking(move || {
//!     generate_pdf_from_url(&pool, &request)
//! }).await;
//!
//! // ✅ Synchronous context: Call directly
//! let result = generate_pdf_from_url(&pool, &request);
//! ```
//!
//! ## Incorrect Usage
//!
//! ```rust,ignore
//! // ❌ WRONG: Blocking the async runtime
//! async fn bad_handler(pool: web::Data<SharedPool>) -> HttpResponse {
//!     // This blocks the entire async runtime thread!
//!     let result = generate_pdf_from_url(&pool, &request);
//!     // ...
//! }
//! ```
//!
//! # Error Handling
//!
//! All service functions return `Result<T, PdfServiceError>`. The error type
//! provides HTTP status codes and error codes for easy API response building:
//!
//! ```rust,ignore
//! use html2pdf_api::service::{PdfServiceError, ErrorResponse};
//!
//! fn handle_error(error: PdfServiceError) -> (u16, ErrorResponse) {
//!     let status = error.status_code();  // e.g., 400, 503, 504
//!     let response = ErrorResponse::from(&error);
//!     (status, response)
//! }
//!
//! // Check if error is worth retrying
//! if error.is_retryable() {
//!     // Wait and retry
//!     std::thread::sleep(Duration::from_secs(1));
//! }
//! ```
//!
//! # Testing
//!
//! The service functions can be tested without HTTP:
//!
//! ```rust,ignore
//! use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest, PdfServiceError};
//! use html2pdf_api::factory::mock::MockBrowserFactory;
//!
//! #[test]
//! fn test_invalid_url_returns_error() {
//!     let pool = create_test_pool();
//!     
//!     let request = PdfFromUrlRequest {
//!         url: "not-a-valid-url".to_string(),
//!         ..Default::default()
//!     };
//!     
//!     let result = generate_pdf_from_url(&pool, &request);
//!     
//!     assert!(matches!(result, Err(PdfServiceError::InvalidUrl(_))));
//! }
//!
//! #[test]
//! fn test_empty_html_returns_error() {
//!     let pool = create_test_pool();
//!     
//!     let request = PdfFromHtmlRequest {
//!         html: "   ".to_string(),  // whitespace only
//!         ..Default::default()
//!     };
//!     
//!     let result = generate_pdf_from_html(&pool, &request);
//!     
//!     assert!(matches!(result, Err(PdfServiceError::EmptyHtml)));
//! }
//! ```
//!
//! # Feature Flags
//!
//! This module is always available. However, the types include serde support
//! which is enabled by any integration feature:
//!
//! | Feature | Effect on this module |
//! |---------|----------------------|
//! | `actix-integration` | Enables `serde` for request/response types |
//! | `rocket-integration` | Enables `serde` for request/response types |
//! | `axum-integration` | Enables `serde` for request/response types |
//!
//! # See Also
//!
//! - [`crate::pool`] - Browser pool management
//! - [`crate::integrations`] - Framework-specific handlers
//! - [`crate::prelude`] - Convenient re-exports

mod pdf;
mod types;

// ============================================================================
// Re-exports: Types
// ============================================================================

pub use types::ErrorResponse;
pub use types::HealthResponse;
pub use types::PdfFromHtmlRequest;
pub use types::PdfFromUrlRequest;
pub use types::PdfResponse;
pub use types::PdfServiceError;
pub use types::PoolStatsResponse;

// ============================================================================
// Re-exports: Functions
// ============================================================================

pub use pdf::generate_pdf_from_html;
pub use pdf::generate_pdf_from_url;
pub use pdf::get_pool_stats;
pub use pdf::is_pool_ready;

// ============================================================================
// Re-exports: Constants
// ============================================================================

pub use pdf::DEFAULT_TIMEOUT_SECS;
pub use pdf::DEFAULT_WAIT_SECS;

// ============================================================================
// Module-level tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify all expected types are exported.
    #[test]
    fn test_type_exports() {
        // Request types
        let _: PdfFromUrlRequest = PdfFromUrlRequest::default();
        let _: PdfFromHtmlRequest = PdfFromHtmlRequest::default();

        // Response types
        let _: PdfResponse = PdfResponse::new(vec![], "test.pdf".to_string(), false);
        let _: PoolStatsResponse = PoolStatsResponse {
            available: 0,
            active: 0,
            total: 0,
        };
        let _: HealthResponse = HealthResponse::default();
        let _: ErrorResponse = ErrorResponse {
            error: "test".to_string(),
            code: "TEST".to_string(),
        };

        // Error types
        let _: PdfServiceError = PdfServiceError::EmptyHtml;
    }

    /// Verify all expected constants are exported.
    #[test]
    fn test_constant_exports() {
        assert!(DEFAULT_TIMEOUT_SECS > 0);
        assert!(DEFAULT_WAIT_SECS > 0);
        assert!(DEFAULT_TIMEOUT_SECS >= DEFAULT_WAIT_SECS);
    }

    /// Verify error type conversions work.
    #[test]
    fn test_error_to_response_conversion() {
        let error = PdfServiceError::InvalidUrl("test".to_string());
        let response: ErrorResponse = error.into();

        assert_eq!(response.code, "INVALID_URL");
        assert!(response.error.contains("Invalid URL"));
    }
}
