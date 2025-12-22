//! Core PDF generation service (framework-agnostic).
//!
//! This module contains the core PDF generation logic that is shared across
//! all web framework integrations. The functions here are **synchronous/blocking**
//! and should be called from within a blocking context (e.g., `tokio::task::spawn_blocking`,
//! `actix_web::web::block`, etc.).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Framework Integration                        │
//! │              (Actix-web / Rocket / Axum)                        │
//! └─────────────────────────┬───────────────────────────────────────┘
//!                           │ async context
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │              spawn_blocking / web::block                        │
//! └─────────────────────────┬───────────────────────────────────────┘
//!                           │ blocking context
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                  This Module (pdf.rs)                           │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
//! │  │generate_pdf_    │  │generate_pdf_    │  │get_pool_stats   │  │
//! │  │from_url         │  │from_html        │  │                 │  │
//! │  └────────┬────────┘  └────────┬────────┘  └─────────────────┘  │
//! │           │                    │                                │
//! │           └──────────┬─────────┘                                │
//! │                      ▼                                          │
//! │           ┌─────────────────────┐                               │
//! │           │generate_pdf_internal│                               │
//! │           └──────────┬──────────┘                               │
//! └──────────────────────┼──────────────────────────────────────────┘
//!                        │
//!                        ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    BrowserPool                                  │
//! │                 (headless_chrome)                               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Thread Safety
//!
//! All functions in this module are designed to be called from multiple threads
//! concurrently. The browser pool is protected by a `Mutex`, and each PDF
//! generation operation acquires a browser, uses it, and returns it to the pool
//! automatically via RAII.
//!
//! # Blocking Behavior
//!
//! **Important:** These functions block the calling thread. In an async context,
//! always wrap calls in a blocking task:
//!
//! ```rust,ignore
//! // ✅ Correct: Using spawn_blocking
//! let result = tokio::task::spawn_blocking(move || {
//!     generate_pdf_from_url(&pool, &request)
//! }).await?;
//!
//! // ❌ Wrong: Calling directly in async context
//! // This will block the async runtime!
//! let result = generate_pdf_from_url(&pool, &request);
//! ```
//!
//! # Usage Examples
//!
//! ## Basic URL to PDF Conversion
//!
//! ```rust,ignore
//! use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};
//! use std::sync::Mutex;
//!
//! // Assuming `pool` is a Mutex<BrowserPool>
//! let request = PdfFromUrlRequest {
//!     url: "https://example.com".to_string(),
//!     ..Default::default()
//! };
//!
//! // In a blocking context:
//! let response = generate_pdf_from_url(&pool, &request)?;
//! println!("Generated PDF: {} bytes", response.data.len());
//! ```
//!
//! ## HTML to PDF Conversion
//!
//! ```rust,ignore
//! use html2pdf_api::service::{generate_pdf_from_html, PdfFromHtmlRequest};
//!
//! let request = PdfFromHtmlRequest {
//!     html: "<html><body><h1>Hello World</h1></body></html>".to_string(),
//!     filename: Some("hello.pdf".to_string()),
//!     ..Default::default()
//! };
//!
//! let response = generate_pdf_from_html(&pool, &request)?;
//! std::fs::write("hello.pdf", &response.data)?;
//! ```
//!
//! ## With Async Web Framework
//!
//! ```rust,ignore
//! use actix_web::{web, HttpResponse};
//! use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};
//!
//! async fn handler(
//!     pool: web::Data<SharedPool>,
//!     query: web::Query<PdfFromUrlRequest>,
//! ) -> HttpResponse {
//!     let pool = pool.into_inner();
//!     let request = query.into_inner();
//!
//!     let result = web::block(move || {
//!         generate_pdf_from_url(&pool, &request)
//!     }).await;
//!
//!     match result {
//!         Ok(Ok(pdf)) => HttpResponse::Ok()
//!             .content_type("application/pdf")
//!             .body(pdf.data),
//!         Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
//!         Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
//!     }
//! }
//! ```
//!
//! # Performance Considerations
//!
//! | Operation | Typical Duration | Notes |
//! |-----------|------------------|-------|
//! | Pool lock acquisition | < 1ms | Fast, non-blocking |
//! | Browser checkout | < 1ms | If browser available |
//! | Browser creation | 500ms - 2s | If pool needs to create new browser |
//! | Page navigation | 100ms - 10s | Depends on target page |
//! | JavaScript wait | 0 - 15s | Configurable via `waitsecs` |
//! | PDF generation | 100ms - 5s | Depends on page complexity |
//! | Tab cleanup | < 100ms | Best effort, non-blocking |
//!
//! # Error Handling
//!
//! All functions return `Result<T, PdfServiceError>`. Errors are categorized
//! and include appropriate HTTP status codes. See [`PdfServiceError`] for
//! the complete error taxonomy.
//!
//! [`PdfServiceError`]: crate::service::PdfServiceError

use headless_chrome::types::PrintToPdfOptions;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::handle::BrowserHandle;
use crate::pool::BrowserPool;
use crate::service::types::*;

// ============================================================================
// Constants
// ============================================================================

/// Default timeout for the entire PDF generation operation in seconds.
///
/// This timeout encompasses the complete operation including:
/// - Browser acquisition from pool
/// - Page navigation
/// - JavaScript execution wait
/// - PDF rendering
/// - Tab cleanup
///
/// If the operation exceeds this duration, a [`PdfServiceError::Timeout`]
/// error is returned.
///
/// # Default Value
///
/// `60` seconds - sufficient for most web pages, including those with
/// heavy JavaScript and external resources.
///
/// # Customization
///
/// This constant is used by framework integrations for their timeout wrappers.
/// To customize, create your own timeout wrapper around the service functions.
///
/// ```rust,ignore
/// use std::time::Duration;
/// use tokio::time::timeout;
///
/// let custom_timeout = Duration::from_secs(120); // 2 minutes
///
/// let result = timeout(custom_timeout, async {
///     tokio::task::spawn_blocking(move || {
///         generate_pdf_from_url(&pool, &request)
///     }).await
/// }).await;
/// ```
pub const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Default wait time for JavaScript execution in seconds.
///
/// After page navigation completes, the service waits for JavaScript to finish
/// rendering dynamic content. This constant defines the default wait time when
/// not specified in the request.
///
/// # Behavior
///
/// During the wait period, the service polls every 200ms for `window.isPageDone === true`.
/// If the page sets this flag, PDF generation proceeds immediately. Otherwise,
/// the full wait duration elapses before generating the PDF.
///
/// # Default Value
///
/// `5` seconds - balances between allowing time for JavaScript execution
/// and not waiting unnecessarily for simple pages.
///
/// # Recommendations
///
/// | Page Type | Recommended Wait |
/// |-----------|------------------|
/// | Static HTML | 1-2 seconds |
/// | Light JavaScript (vanilla JS, jQuery) | 3-5 seconds |
/// | Heavy SPA (React, Vue, Angular) | 5-10 seconds |
/// | Complex visualizations (D3, charts) | 10-15 seconds |
/// | Real-time data loading | 10-20 seconds |
pub const DEFAULT_WAIT_SECS: u64 = 5;

/// Polling interval for JavaScript completion check in milliseconds.
///
/// When waiting for JavaScript to complete, the service checks for
/// `window.isPageDone === true` at this interval.
///
/// # Trade-offs
///
/// - **Shorter interval**: More responsive but higher CPU usage
/// - **Longer interval**: Lower CPU usage but may overshoot ready state
///
/// # Default Value
///
/// `200` milliseconds - provides good responsiveness without excessive polling.
const JS_POLL_INTERVAL_MS: u64 = 200;

// ============================================================================
// Public API - Core PDF Generation Functions
// ============================================================================

/// Generate a PDF from a URL.
///
/// Navigates to the specified URL using a browser from the pool, waits for
/// JavaScript execution, and generates a PDF of the rendered page.
///
/// # Thread Safety
///
/// This function is thread-safe and can be called concurrently from multiple
/// threads. The browser pool mutex ensures safe access to shared resources.
///
/// # Blocking Behavior
///
/// **This function blocks the calling thread.** In async contexts, wrap it
/// in `tokio::task::spawn_blocking`, `actix_web::web::block`, or similar.
///
/// # Arguments
///
/// * `pool` - Reference to the mutex-wrapped browser pool. The mutex is held
///   only briefly during browser checkout; PDF generation occurs outside the lock.
/// * `request` - PDF generation parameters. See [`PdfFromUrlRequest`] for details.
///
/// # Returns
///
/// * `Ok(PdfResponse)` - Successfully generated PDF with binary data and metadata
/// * `Err(PdfServiceError)` - Error with details about what went wrong
///
/// # Errors
///
/// | Error | Cause | Resolution |
/// |-------|-------|------------|
/// | [`InvalidUrl`] | URL is empty or malformed | Provide valid HTTP/HTTPS URL |
/// | [`PoolLockFailed`] | Mutex poisoned | Restart service |
/// | [`BrowserUnavailable`] | Pool exhausted | Retry or increase pool size |
/// | [`TabCreationFailed`] | Browser issue | Automatic recovery |
/// | [`NavigationFailed`] | URL unreachable | Check URL accessibility |
/// | [`NavigationTimeout`] | Page too slow | Increase timeout or optimize page |
/// | [`PdfGenerationFailed`] | Rendering issue | Simplify page or check content |
///
/// [`InvalidUrl`]: PdfServiceError::InvalidUrl
/// [`PoolLockFailed`]: PdfServiceError::PoolLockFailed
/// [`BrowserUnavailable`]: PdfServiceError::BrowserUnavailable
/// [`TabCreationFailed`]: PdfServiceError::TabCreationFailed
/// [`NavigationFailed`]: PdfServiceError::NavigationFailed
/// [`NavigationTimeout`]: PdfServiceError::NavigationTimeout
/// [`PdfGenerationFailed`]: PdfServiceError::PdfGenerationFailed
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,ignore
/// use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};
///
/// let request = PdfFromUrlRequest {
///     url: "https://example.com".to_string(),
///     ..Default::default()
/// };
///
/// let response = generate_pdf_from_url(&pool, &request)?;
/// assert!(response.data.starts_with(b"%PDF-")); // Valid PDF header
/// ```
///
/// ## With Custom Options
///
/// ```rust,ignore
/// let request = PdfFromUrlRequest {
///     url: "https://example.com/report".to_string(),
///     filename: Some("quarterly-report.pdf".to_string()),
///     landscape: Some(true),      // Wide tables
///     waitsecs: Some(10),         // Complex charts
///     download: Some(true),       // Force download
///     print_background: Some(true),
/// };
///
/// let response = generate_pdf_from_url(&pool, &request)?;
/// println!("Generated {} with {} bytes", response.filename, response.size());
/// ```
///
/// ## Error Handling
///
/// ```rust,ignore
/// match generate_pdf_from_url(&pool, &request) {
///     Ok(pdf) => {
///         // Success - use pdf.data
///     }
///     Err(PdfServiceError::InvalidUrl(msg)) => {
///         // Client error - return 400
///         eprintln!("Bad URL: {}", msg);
///     }
///     Err(PdfServiceError::BrowserUnavailable(_)) => {
///         // Transient error - retry
///         std::thread::sleep(Duration::from_secs(1));
///     }
///     Err(e) => {
///         // Other error
///         eprintln!("PDF generation failed: {}", e);
///     }
/// }
/// ```
///
/// # Performance
///
/// Typical execution time breakdown for a moderately complex page:
///
/// ```text
/// ┌────────────────────────────────────────────────────────────────┐
/// │ Pool lock + browser checkout                          ~1ms    │
/// │ ├─────────────────────────────────────────────────────────────┤
/// │ Tab creation                                          ~50ms   │
/// │ ├─────────────────────────────────────────────────────────────┤
/// │ Navigation + page load                                ~500ms  │
/// │ ├─────────────────────────────────────────────────────────────┤
/// │ JavaScript wait (configurable)                        ~5000ms │
/// │ ├─────────────────────────────────────────────────────────────┤
/// │ PDF rendering                                         ~200ms  │
/// │ ├─────────────────────────────────────────────────────────────┤
/// │ Tab cleanup                                           ~50ms   │
/// └────────────────────────────────────────────────────────────────┘
/// Total: ~5.8 seconds (dominated by JS wait)
/// ```
pub fn generate_pdf_from_url(
    pool: &Mutex<BrowserPool>,
    request: &PdfFromUrlRequest,
) -> Result<PdfResponse, PdfServiceError> {
    // Validate URL before acquiring browser
    let url = validate_url(&request.url)?;

    log::debug!(
        "Generating PDF from URL: {} (landscape={}, wait={}s)",
        url,
        request.is_landscape(),
        request.wait_duration().as_secs()
    );

    // Acquire browser from pool (lock held briefly)
    let browser = acquire_browser(pool)?;

    // Generate PDF (lock released, browser returned via RAII on completion/error)
    let pdf_data = generate_pdf_internal(
        &browser,
        &url,
        request.wait_duration(),
        request.is_landscape(),
        request.print_background(),
    )?;

    log::info!(
        "✅ PDF generated successfully from URL: {} ({} bytes)",
        url,
        pdf_data.len()
    );

    Ok(PdfResponse::new(
        pdf_data,
        request.filename_or_default(),
        request.is_download(),
    ))
}

/// Generate a PDF from HTML content.
///
/// Loads the provided HTML content into a browser tab using a data URL,
/// waits for any JavaScript execution, and generates a PDF.
///
/// # Thread Safety
///
/// This function is thread-safe and can be called concurrently from multiple
/// threads. See [`generate_pdf_from_url`] for details.
///
/// # Blocking Behavior
///
/// **This function blocks the calling thread.** See [`generate_pdf_from_url`]
/// for guidance on async usage.
///
/// # How It Works
///
/// The HTML content is converted to a data URL:
///
/// ```text
/// data:text/html;charset=utf-8,<encoded-html-content>
/// ```
///
/// This allows loading HTML directly without a web server. The browser
/// renders the HTML as if it were loaded from a regular URL.
///
/// # Arguments
///
/// * `pool` - Reference to the mutex-wrapped browser pool
/// * `request` - HTML content and generation parameters. See [`PdfFromHtmlRequest`].
///
/// # Returns
///
/// * `Ok(PdfResponse)` - Successfully generated PDF
/// * `Err(PdfServiceError)` - Error details
///
/// # Errors
///
/// | Error | Cause | Resolution |
/// |-------|-------|------------|
/// | [`EmptyHtml`] | HTML content is empty/whitespace | Provide HTML content |
/// | [`PoolLockFailed`] | Mutex poisoned | Restart service |
/// | [`BrowserUnavailable`] | Pool exhausted | Retry or increase pool size |
/// | [`NavigationFailed`] | HTML parsing issue | Check HTML validity |
/// | [`PdfGenerationFailed`] | Rendering issue | Simplify HTML |
///
/// [`EmptyHtml`]: PdfServiceError::EmptyHtml
/// [`PoolLockFailed`]: PdfServiceError::PoolLockFailed
/// [`BrowserUnavailable`]: PdfServiceError::BrowserUnavailable
/// [`NavigationFailed`]: PdfServiceError::NavigationFailed
/// [`PdfGenerationFailed`]: PdfServiceError::PdfGenerationFailed
///
/// # Limitations
///
/// ## External Resources
///
/// Since HTML is loaded via data URL, relative URLs don't work:
///
/// ```html
/// <!-- ❌ Won't work - relative URL -->
/// <img src="/images/logo.png">
///
/// <!-- ✅ Works - absolute URL -->
/// <img src="https://example.com/images/logo.png">
///
/// <!-- ✅ Works - inline base64 -->
/// <img src="data:image/png;base64,iVBORw0KGgo...">
/// ```
///
/// ## Size Limits
///
/// Data URLs have browser-specific size limits. For very large HTML documents
/// (> 1MB), consider:
/// - Hosting the HTML on a temporary server
/// - Using [`generate_pdf_from_url`] instead
/// - Splitting into multiple PDFs
///
/// # Examples
///
/// ## Simple HTML
///
/// ```rust,ignore
/// use html2pdf_api::service::{generate_pdf_from_html, PdfFromHtmlRequest};
///
/// let request = PdfFromHtmlRequest {
///     html: "<h1>Hello World</h1><p>This is a test.</p>".to_string(),
///     ..Default::default()
/// };
///
/// let response = generate_pdf_from_html(&pool, &request)?;
/// std::fs::write("output.pdf", &response.data)?;
/// ```
///
/// ## Complete Document with Styling
///
/// ```rust,ignore
/// let html = r#"
/// <!DOCTYPE html>
/// <html>
/// <head>
///     <meta charset="UTF-8">
///     <style>
///         body {
///             font-family: 'Arial', sans-serif;
///             margin: 40px;
///             color: #333;
///         }
///         h1 {
///             color: #0066cc;
///             border-bottom: 2px solid #0066cc;
///             padding-bottom: 10px;
///         }
///         table {
///             width: 100%;
///             border-collapse: collapse;
///             margin-top: 20px;
///         }
///         th, td {
///             border: 1px solid #ddd;
///             padding: 12px;
///             text-align: left;
///         }
///         th {
///             background-color: #f5f5f5;
///         }
///     </style>
/// </head>
/// <body>
///     <h1>Monthly Report</h1>
///     <p>Generated on: 2024-01-15</p>
///     <table>
///         <tr><th>Metric</th><th>Value</th></tr>
///         <tr><td>Revenue</td><td>$50,000</td></tr>
///         <tr><td>Users</td><td>1,234</td></tr>
///     </table>
/// </body>
/// </html>
/// "#;
///
/// let request = PdfFromHtmlRequest {
///     html: html.to_string(),
///     filename: Some("monthly-report.pdf".to_string()),
///     print_background: Some(true), // Include styled backgrounds
///     ..Default::default()
/// };
///
/// let response = generate_pdf_from_html(&pool, &request)?;
/// ```
///
/// ## With Embedded Images
///
/// ```rust,ignore
/// // Base64 encode an image
/// let image_base64 = base64::encode(std::fs::read("logo.png")?);
///
/// let html = format!(r#"
/// <!DOCTYPE html>
/// <html>
/// <body>
///     <img src="data:image/png;base64,{}" alt="Logo">
///     <h1>Company Report</h1>
/// </body>
/// </html>
/// "#, image_base64);
///
/// let request = PdfFromHtmlRequest {
///     html,
///     ..Default::default()
/// };
///
/// let response = generate_pdf_from_html(&pool, &request)?;
/// ```
pub fn generate_pdf_from_html(
    pool: &Mutex<BrowserPool>,
    request: &PdfFromHtmlRequest,
) -> Result<PdfResponse, PdfServiceError> {
    // Validate HTML content
    if request.html.trim().is_empty() {
        log::warn!("Empty HTML content provided");
        return Err(PdfServiceError::EmptyHtml);
    }

    log::debug!(
        "Generating PDF from HTML ({} bytes, landscape={}, wait={}s)",
        request.html.len(),
        request.is_landscape(),
        request.wait_duration().as_secs()
    );

    // Acquire browser from pool
    let browser = acquire_browser(pool)?;

    // Convert HTML to data URL
    // Using percent-encoding to handle special characters
    let data_url = format!(
        "data:text/html;charset=utf-8,{}",
        urlencoding::encode(&request.html)
    );

    log::trace!("Data URL length: {} bytes", data_url.len());

    // Generate PDF
    let pdf_data = generate_pdf_internal(
        &browser,
        &data_url,
        request.wait_duration(),
        request.is_landscape(),
        request.print_background(),
    )?;

    log::info!(
        "✅ PDF generated successfully from HTML ({} bytes input → {} bytes output)",
        request.html.len(),
        pdf_data.len()
    );

    Ok(PdfResponse::new(
        pdf_data,
        request.filename_or_default(),
        request.is_download(),
    ))
}

/// Get current browser pool statistics.
///
/// Returns real-time metrics about the browser pool state including
/// available browsers, active browsers, and total count.
///
/// # Thread Safety
///
/// This function briefly acquires the pool lock to read statistics.
/// It's safe to call frequently for monitoring purposes.
///
/// # Blocking Behavior
///
/// This function blocks briefly (< 1ms typically) while holding the
/// pool lock. It's generally safe to call from async contexts directly,
/// but for consistency, you may still wrap it in a blocking task.
///
/// # Arguments
///
/// * `pool` - Reference to the mutex-wrapped browser pool
///
/// # Returns
///
/// * `Ok(PoolStatsResponse)` - Current pool statistics
/// * `Err(PdfServiceError::PoolLockFailed)` - If mutex is poisoned
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,ignore
/// use html2pdf_api::service::get_pool_stats;
///
/// let stats = get_pool_stats(&pool)?;
/// println!("Available: {}", stats.available);
/// println!("Active: {}", stats.active);
/// println!("Total: {}", stats.total);
/// ```
///
/// ## Monitoring Integration
///
/// ```rust,ignore
/// use prometheus::{Gauge, register_gauge};
///
/// lazy_static! {
///     static ref POOL_AVAILABLE: Gauge = register_gauge!(
///         "browser_pool_available",
///         "Number of available browsers in pool"
///     ).unwrap();
///     static ref POOL_ACTIVE: Gauge = register_gauge!(
///         "browser_pool_active",
///         "Number of active browsers in pool"
///     ).unwrap();
/// }
///
/// fn update_metrics(pool: &Mutex<BrowserPool>) {
///     if let Ok(stats) = get_pool_stats(pool) {
///         POOL_AVAILABLE.set(stats.available as f64);
///         POOL_ACTIVE.set(stats.active as f64);
///     }
/// }
/// ```
///
/// ## Capacity Check
///
/// ```rust,ignore
/// let stats = get_pool_stats(&pool)?;
///
/// if stats.available == 0 {
///     log::warn!("No browsers available, requests may be delayed");
/// }
///
/// let utilization = stats.active as f64 / stats.total.max(1) as f64;
/// if utilization > 0.8 {
///     log::warn!("Pool utilization at {:.0}%, consider scaling", utilization * 100.0);
/// }
/// ```
pub fn get_pool_stats(pool: &Mutex<BrowserPool>) -> Result<PoolStatsResponse, PdfServiceError> {
    let pool_guard = pool.lock().map_err(|e| {
        log::error!("Failed to lock browser pool for stats: {}", e);
        PdfServiceError::PoolLockFailed(e.to_string())
    })?;

    let stats = pool_guard.stats();

    Ok(PoolStatsResponse {
        available: stats.available,
        active: stats.active,
        total: stats.total,
    })
}

/// Check if the browser pool is ready to handle requests.
///
/// Returns `true` if the pool has available browsers or capacity to create
/// new ones. This is useful for readiness probes in container orchestration.
///
/// # Readiness Criteria
///
/// The pool is considered "ready" if either:
/// - There are idle browsers available (`available > 0`), OR
/// - There is capacity to create new browsers (`active < max_pool_size`)
///
/// The pool is "not ready" only when:
/// - All browsers are in use AND the pool is at maximum capacity
///
/// # Arguments
///
/// * `pool` - Reference to the mutex-wrapped browser pool
///
/// # Returns
///
/// * `Ok(true)` - Pool can accept new requests
/// * `Ok(false)` - Pool is at capacity, requests will queue
/// * `Err(PdfServiceError::PoolLockFailed)` - If mutex is poisoned
///
/// # Use Cases
///
/// ## Kubernetes Readiness Probe
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
/// ## Load Balancer Health Check
///
/// When `is_pool_ready` returns `false`, the endpoint should return
/// HTTP 503 Service Unavailable to remove the instance from rotation.
///
/// # Examples
///
/// ## Basic Check
///
/// ```rust,ignore
/// use html2pdf_api::service::is_pool_ready;
///
/// if is_pool_ready(&pool)? {
///     println!("Pool is ready to accept requests");
/// } else {
///     println!("Pool is at capacity");
/// }
/// ```
///
/// ## Request Gating
///
/// ```rust,ignore
/// async fn handle_request(pool: &Mutex<BrowserPool>, request: PdfFromUrlRequest) -> Result<PdfResponse, Error> {
///     // Quick capacity check before expensive operation
///     if !is_pool_ready(pool)? {
///         return Err(Error::ServiceUnavailable("Pool at capacity, try again later"));
///     }
///     
///     // Proceed with PDF generation
///     generate_pdf_from_url(pool, &request)
/// }
/// ```
pub fn is_pool_ready(pool: &Mutex<BrowserPool>) -> Result<bool, PdfServiceError> {
    let pool_guard = pool.lock().map_err(|e| {
        log::error!("Failed to lock browser pool for readiness check: {}", e);
        PdfServiceError::PoolLockFailed(e.to_string())
    })?;

    let stats = pool_guard.stats();
    let config = pool_guard.config();

    // Ready if we have available browsers OR we can create more
    let is_ready = stats.available > 0 || stats.active < config.max_pool_size;

    log::trace!(
        "Pool readiness check: available={}, active={}, max={}, ready={}",
        stats.available,
        stats.active,
        config.max_pool_size,
        is_ready
    );

    Ok(is_ready)
}

// ============================================================================
// Internal Helper Functions
// ============================================================================

/// Validate and normalize a URL string.
///
/// Parses the URL using the `url` crate and returns the normalized form.
/// This catches malformed URLs early, before acquiring a browser.
///
/// # Validation Rules
///
/// - URL must not be empty
/// - URL must be parseable by the `url` crate
/// - Scheme must be present (http/https/file/data)
///
/// # Arguments
///
/// * `url` - The URL string to validate
///
/// # Returns
///
/// * `Ok(String)` - The normalized URL
/// * `Err(PdfServiceError::InvalidUrl)` - If validation fails
///
/// # Examples
///
/// ```rust,ignore
/// assert!(validate_url("https://example.com").is_ok());
/// assert!(validate_url("").is_err());
/// assert!(validate_url("not-a-url").is_err());
/// ```
fn validate_url(url: &str) -> Result<String, PdfServiceError> {
    // Check for empty URL first (better error message)
    if url.trim().is_empty() {
        log::debug!("URL validation failed: empty URL");
        return Err(PdfServiceError::InvalidUrl("URL is required".to_string()));
    }

    // Parse and normalize the URL
    match url::Url::parse(url) {
        Ok(parsed) => {
            log::trace!("URL validated successfully: {}", parsed);
            Ok(parsed.to_string())
        }
        Err(e) => {
            log::debug!("URL validation failed for '{}': {}", url, e);
            Err(PdfServiceError::InvalidUrl(e.to_string()))
        }
    }
}

/// Acquire a browser from the pool.
///
/// Locks the pool mutex, retrieves a browser, and returns it. The lock is
/// released immediately after checkout, not held during PDF generation.
///
/// # Browser Lifecycle
///
/// The returned `BrowserHandle` uses RAII to automatically return the
/// browser to the pool when dropped:
///
/// ```text
/// ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
/// │  acquire_browser │ ──▶ │  BrowserHandle  │ ──▶ │  PDF Generation │
/// │  (lock, get)     │     │  (RAII guard)   │     │  (uses browser) │
/// └─────────────────┘     └─────────────────┘     └────────┬────────┘
///                                                          │
///                                                          ▼
///                         ┌─────────────────┐     ┌─────────────────┐
///                         │  Back to Pool   │ ◀── │  Drop Handle    │
///                         │  (automatic)    │     │  (RAII cleanup) │
///                         └─────────────────┘     └─────────────────┘
/// ```
///
/// # Arguments
///
/// * `pool` - Reference to the mutex-wrapped browser pool
///
/// # Returns
///
/// * `Ok(BrowserHandle)` - A browser ready for use
/// * `Err(PdfServiceError)` - If pool lock or browser acquisition fails
fn acquire_browser(pool: &Mutex<BrowserPool>) -> Result<BrowserHandle, PdfServiceError> {
    // Acquire lock on the pool
    let pool_guard = pool.lock().map_err(|e| {
        log::error!("❌ Failed to lock browser pool: {}", e);
        PdfServiceError::PoolLockFailed(e.to_string())
    })?;

    // Get a browser from the pool
    let browser = pool_guard.get().map_err(|e| {
        log::error!("❌ Failed to get browser from pool: {}", e);
        PdfServiceError::BrowserUnavailable(e.to_string())
    })?;

    log::debug!("Acquired browser {} from pool", browser.id());

    Ok(browser)
    // pool_guard (MutexGuard) is dropped here, releasing the lock
}

/// Core PDF generation logic.
///
/// This function performs the actual work of:
/// 1. Creating a new browser tab
/// 2. Navigating to the URL
/// 3. Waiting for JavaScript completion
/// 4. Generating the PDF
/// 5. Cleaning up the tab
///
/// # Arguments
///
/// * `browser` - Browser handle from the pool
/// * `url` - URL to navigate to (can be http/https or data: URL)
/// * `wait_duration` - How long to wait for JavaScript
/// * `landscape` - Whether to use landscape orientation
/// * `print_background` - Whether to include background graphics
///
/// # Returns
///
/// * `Ok(Vec<u8>)` - The raw PDF binary data
/// * `Err(PdfServiceError)` - If any step fails
///
/// # Tab Lifecycle
///
/// A new tab is created for each PDF generation and closed afterward.
/// This ensures clean state and prevents memory leaks from accumulating
/// page resources.
///
/// ```text
/// Browser Instance
/// ├── Tab 1 (new) ◀── Created for this request
/// │   ├── Navigate to URL
/// │   ├── Wait for JS
/// │   ├── Generate PDF
/// │   └── Close tab ◀── Cleanup
/// └── (available for next request)
/// ```
fn generate_pdf_internal(
    browser: &BrowserHandle,
    url: &str,
    wait_duration: Duration,
    landscape: bool,
    print_background: bool,
) -> Result<Vec<u8>, PdfServiceError> {
    let start_time = Instant::now();

    // Create new tab
    log::trace!("Creating new browser tab");
    let tab = browser.new_tab().map_err(|e| {
        log::error!("❌ Failed to create tab: {}", e);
        PdfServiceError::TabCreationFailed(e.to_string())
    })?;

    // Configure PDF options
    let print_options = build_print_options(landscape, print_background);

    // Navigate to URL
    log::trace!("Navigating to URL: {}", truncate_url(url, 100));
    let nav_start = Instant::now();

    let page = tab
        .navigate_to(url)
        .map_err(|e| {
            log::error!("❌ Failed to navigate to URL: {}", e);
            PdfServiceError::NavigationFailed(e.to_string())
        })?
        .wait_until_navigated()
        .map_err(|e| {
            log::error!("❌ Navigation timeout: {}", e);
            PdfServiceError::NavigationTimeout(e.to_string())
        })?;

    log::debug!("Navigation completed in {:?}", nav_start.elapsed());

    // Wait for JavaScript execution
    wait_for_page_ready(&tab, wait_duration);

    // Generate PDF
    log::trace!("Generating PDF");
    let pdf_start = Instant::now();

    let pdf_data = page.print_to_pdf(print_options).map_err(|e| {
        log::error!("❌ Failed to generate PDF: {}", e);
        PdfServiceError::PdfGenerationFailed(e.to_string())
    })?;

    log::debug!(
        "PDF generated in {:?} ({} bytes)",
        pdf_start.elapsed(),
        pdf_data.len()
    );

    // Close tab (best effort - don't fail if this doesn't work)
    close_tab_safely(&tab);

    log::debug!("Total PDF generation time: {:?}", start_time.elapsed());

    Ok(pdf_data)
}

/// Build PDF print options.
///
/// Creates the `PrintToPdfOptions` struct with the specified settings
/// and sensible defaults for margins and other options.
///
/// # Default Settings
///
/// - **Margins**: All set to 0 (full page)
/// - **Header/Footer**: Disabled
/// - **Background**: Configurable (default: true)
/// - **Scale**: 1.0 (100%)
fn build_print_options(landscape: bool, print_background: bool) -> Option<PrintToPdfOptions> {
    Some(PrintToPdfOptions {
        landscape: Some(landscape),
        display_header_footer: Some(false),
        print_background: Some(print_background),
        // Zero margins for full-page output
        margin_top: Some(0.0),
        margin_bottom: Some(0.0),
        margin_left: Some(0.0),
        margin_right: Some(0.0),
        // Use defaults for everything else
        ..Default::default()
    })
}

/// Wait for the page to signal it's ready for PDF generation.
///
/// This function implements a polling loop that checks for `window.isPageDone === true`.
/// This allows JavaScript-heavy pages to signal when they've finished rendering,
/// enabling early PDF generation without waiting the full timeout.
///
/// # Behavior Summary
///
/// | Page State | Result |
/// |------------|--------|
/// | `window.isPageDone = true` | Returns **immediately** (early exit) |
/// | `window.isPageDone = false` | Waits **full duration** |
/// | `window.isPageDone` not defined | Waits **full duration** |
/// | JavaScript error during check | Waits **full duration** |
///
/// # Default Behavior (No Flag Set)
///
/// **Important:** If the page does not set `window.isPageDone = true`, this function
/// waits the **full `max_wait` duration** before returning. This is intentional -
/// it gives JavaScript-heavy pages time to render even without explicit signaling.
///
/// For example, with the default `waitsecs = 5`:
/// - A page **with** the flag set immediately: ~0ms wait
/// - A page **without** the flag: full 5000ms wait
///
/// # How It Works
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │                    wait_for_page_ready                          │
/// │                                                                 │
/// │   ┌─────────┐     ┌──────────────┐     ┌─────────────────────┐  │
/// │   │  Start  │────▶│ Check flag   │────▶│ window.isPageDone?  │  │
/// │   └─────────┘     └──────────────┘     └──────────┬──────────┘  │
/// │                                                   │             │
/// │                          ┌────────────────────────┼─────────┐   │
/// │                          │                        │         │   │
/// │                          ▼                        ▼         │   │
/// │                   ┌────────────┐           ┌───────────┐    │   │
/// │                   │   true     │           │  false /  │    │   │
/// │                   │ (ready!)   │           │ undefined │    │   │
/// │                   └─────┬──────┘           └─────┬─────┘    │   │
/// │                         │                        │          │   │
/// │                         ▼                        ▼          │   │
/// │                   ┌───────────┐           ┌───────────┐     │   │
/// │                   │  Return   │           │ Sleep     │     │   │
/// │                   │  early    │           │ 200ms     │─────┘   │
/// │                   └───────────┘           └───────────┘         │
/// │                                                  │              │
/// │                                                  ▼              │
/// │                                           ┌───────────┐         │
/// │                                           │ Timeout?  │         │
/// │                                           └─────┬─────┘         │
/// │                                                 │               │
/// │                                    ┌────────────┴────────────┐  │
/// │                                    ▼                         ▼  │
/// │                             ┌───────────┐              ┌──────┐ │
/// │                             │   Yes     │              │  No  │ │
/// │                             │ (proceed) │              │(loop)│ │
/// │                             └───────────┘              └──────┘ │
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Polling Timeline
///
/// The function polls every 200ms (see `JS_POLL_INTERVAL_MS`):
///
/// ```text
/// Time:   0ms    200ms   400ms   600ms   800ms  ...  5000ms
///          │       │       │       │       │           │
///          ▼       ▼       ▼       ▼       ▼           ▼
///        Poll    Poll    Poll    Poll    Poll  ...   Timeout
///          │       │       │       │       │           │
///          └───────┴───────┴───────┴───────┴───────────┤
///                                                      ▼
///                                              Proceed to PDF
///
/// If window.isPageDone = true at any poll → Exit immediately
/// ```
///
/// Each poll executes this JavaScript:
///
/// ```javascript
/// window.isPageDone === true  // Returns true, false, or undefined
/// ```
///
/// - `true` → Function returns immediately
/// - `false` / `undefined` / error → Continue polling until timeout
///
/// # Page-Side Implementation (Optional)
///
/// To enable early completion and avoid unnecessary waiting, add this to your
/// page's JavaScript **after** all content is rendered:
///
/// ```javascript
/// // Signal that the page is ready for PDF generation
/// window.isPageDone = true;
/// ```
///
/// ## Framework Examples
///
/// **React:**
/// ```javascript
/// useEffect(() => {
///     fetchData().then((result) => {
///         setData(result);
///         // Signal ready after state update and re-render
///         setTimeout(() => { window.isPageDone = true; }, 0);
///     });
/// }, []);
/// ```
///
/// **Vue:**
/// ```javascript
/// mounted() {
///     this.loadData().then(() => {
///         this.$nextTick(() => {
///             window.isPageDone = true;
///         });
///     });
/// }
/// ```
///
/// **Vanilla JavaScript:**
/// ```javascript
/// document.addEventListener('DOMContentLoaded', async () => {
///     await loadDynamicContent();
///     await renderCharts();
///     window.isPageDone = true;  // All done!
/// });
/// ```
///
/// # When to Increase `waitsecs`
///
/// If you cannot modify the target page to set `window.isPageDone`, increase
/// `waitsecs` based on the page complexity:
///
/// | Page Type | Recommended `waitsecs` |
/// |-----------|------------------------|
/// | Static HTML (no JS) | 1 |
/// | Light JS (form validation, simple DOM) | 2-3 |
/// | Moderate JS (API calls, dynamic content) | 5 (default) |
/// | Heavy SPA (React, Vue, Angular) | 5-10 |
/// | Complex visualizations (D3, charts, maps) | 10-15 |
/// | Pages loading external resources | 10-20 |
///
/// # Performance Optimization
///
/// For high-throughput scenarios, implementing `window.isPageDone` on your
/// pages can significantly improve performance:
///
/// ```text
/// Without flag (5s default wait):
///     Request 1: ████████████████████ 5.2s
///     Request 2: ████████████████████ 5.1s
///     Request 3: ████████████████████ 5.3s
///     Average: 5.2s per PDF
///
/// With flag (page ready in 800ms):
///     Request 1: ████ 0.9s
///     Request 2: ████ 0.8s
///     Request 3: ████ 0.9s
///     Average: 0.87s per PDF (6x faster!)
/// ```
///
/// # Arguments
///
/// * `tab` - The browser tab to check. Must have completed navigation.
/// * `max_wait` - Maximum time to wait before proceeding with PDF generation.
///   This is the upper bound; the function may return earlier if the page
///   signals readiness.
///
/// # Returns
///
/// This function returns `()` (unit). It either:
/// - Returns early when `window.isPageDone === true` is detected
/// - Returns after `max_wait` duration has elapsed (timeout)
///
/// In both cases, PDF generation proceeds afterward. This function never fails -
/// timeout is a normal completion path, not an error.
///
/// # Thread Blocking
///
/// This function blocks the calling thread with `std::thread::sleep()`.
/// Always call from within a blocking context (e.g., `spawn_blocking`).
///
/// # Example
///
/// ```rust,ignore
/// // Navigate to page first
/// let page = tab.navigate_to(url)?.wait_until_navigated()?;
///
/// // Wait up to 10 seconds for JavaScript
/// wait_for_page_ready(&tab, Duration::from_secs(10));
///
/// // Now generate PDF - page is either ready or we've waited long enough
/// let pdf_data = page.print_to_pdf(options)?;
/// ```
fn wait_for_page_ready(tab: &headless_chrome::Tab, max_wait: Duration) {
    let start = Instant::now();
    let poll_interval = Duration::from_millis(JS_POLL_INTERVAL_MS);

    log::trace!(
        "Waiting up to {:?} for page to be ready (polling every {:?})",
        max_wait,
        poll_interval
    );

    while start.elapsed() < max_wait {
        // Check if page signals completion
        let is_done = tab
            .evaluate("window.isPageDone === true", false)
            .map(|result| {
                result
                    .value
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        if is_done {
            log::debug!(
                "Page signaled ready after {:?}",
                start.elapsed()
            );
            return;
        }

        // Sleep before next poll
        std::thread::sleep(poll_interval);
    }

    log::debug!(
        "Page wait completed after {:?} (timeout, proceeding anyway)",
        start.elapsed()
    );
}

/// Safely close a browser tab, ignoring errors.
///
/// Tab cleanup is best-effort. If it fails, we log a warning but don't
/// propagate the error since the PDF generation already succeeded.
///
/// # Why Best-Effort?
///
/// - The PDF data is already captured
/// - Tab resources will be cleaned up when the browser is recycled
/// - Failing here would discard a valid PDF
/// - Some errors (e.g., browser already closed) are expected
///
/// # Arguments
///
/// * `tab` - The browser tab to close
fn close_tab_safely(tab: &headless_chrome::Tab) {
    log::trace!("Closing browser tab");

    if let Err(e) = tab.close(true) {
        // Log but don't fail - PDF generation already succeeded
        log::warn!(
            "Failed to close tab (continuing anyway, resources will be cleaned up): {}",
            e
        );
    } else {
        log::trace!("Tab closed successfully");
    }
}

/// Truncate a URL for logging purposes.
///
/// Data URLs can be extremely long (containing entire HTML documents).
/// This function truncates them for readable log output.
///
/// # Arguments
///
/// * `url` - The URL to truncate
/// * `max_len` - Maximum length before truncation
///
/// # Returns
///
/// The URL, truncated with "..." if longer than `max_len`.
fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len])
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // URL Validation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_url_valid_https() {
        let result = validate_url("https://example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "https://example.com/");
    }

    #[test]
    fn test_validate_url_valid_http() {
        let result = validate_url("http://example.com/path?query=value");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_valid_with_port() {
        let result = validate_url("http://localhost:3000/api");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_empty() {
        let result = validate_url("");
        assert!(matches!(result, Err(PdfServiceError::InvalidUrl(_))));
    }

    #[test]
    fn test_validate_url_whitespace_only() {
        let result = validate_url("   ");
        assert!(matches!(result, Err(PdfServiceError::InvalidUrl(_))));
    }

    #[test]
    fn test_validate_url_no_scheme() {
        let result = validate_url("example.com");
        assert!(matches!(result, Err(PdfServiceError::InvalidUrl(_))));
    }

    #[test]
    fn test_validate_url_relative() {
        let result = validate_url("/path/to/page");
        assert!(matches!(result, Err(PdfServiceError::InvalidUrl(_))));
    }

    #[test]
    fn test_validate_url_data_url() {
        let result = validate_url("data:text/html,<h1>Hello</h1>");
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------------
    // Helper Function Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_truncate_url_short() {
        let url = "https://example.com";
        assert_eq!(truncate_url(url, 50), url);
    }

    #[test]
    fn test_truncate_url_long() {
        let url = "https://example.com/very/long/path/that/exceeds/the/maximum/length";
        let truncated = truncate_url(url, 30);
        assert_eq!(truncated.len(), 33); // 30 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_url_exact_length() {
        let url = "https://example.com";
        assert_eq!(truncate_url(url, url.len()), url);
    }

    #[test]
    fn test_build_print_options_landscape() {
        let options = build_print_options(true, true).unwrap();
        assert_eq!(options.landscape, Some(true));
        assert_eq!(options.print_background, Some(true));
    }

    #[test]
    fn test_build_print_options_portrait() {
        let options = build_print_options(false, false).unwrap();
        assert_eq!(options.landscape, Some(false));
        assert_eq!(options.print_background, Some(false));
    }

    #[test]
    fn test_build_print_options_zero_margins() {
        let options = build_print_options(false, true).unwrap();
        assert_eq!(options.margin_top, Some(0.0));
        assert_eq!(options.margin_bottom, Some(0.0));
        assert_eq!(options.margin_left, Some(0.0));
        assert_eq!(options.margin_right, Some(0.0));
    }

    #[test]
    fn test_build_print_options_no_header_footer() {
        let options = build_print_options(false, true).unwrap();
        assert_eq!(options.display_header_footer, Some(false));
    }

    // -------------------------------------------------------------------------
    // Constants Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_default_timeout_reasonable() {
        // Timeout should be at least 30 seconds for complex pages
        assert!(DEFAULT_TIMEOUT_SECS >= 30);
        // But not more than 5 minutes (would be too long)
        assert!(DEFAULT_TIMEOUT_SECS <= 300);
    }

    #[test]
    fn test_default_wait_reasonable() {
        // Wait should be at least 1 second for any JS
        assert!(DEFAULT_WAIT_SECS >= 1);
        // But not more than 30 seconds by default
        assert!(DEFAULT_WAIT_SECS <= 30);
    }

    #[test]
    fn test_poll_interval_reasonable() {
        // Poll interval should be at least 100ms (not too aggressive)
        assert!(JS_POLL_INTERVAL_MS >= 100);
        // But not more than 1 second (responsive enough)
        assert!(JS_POLL_INTERVAL_MS <= 1000);
    }
}