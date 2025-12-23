//! Shared types for the PDF generation service.
//!
//! This module provides framework-agnostic types used across all integrations
//! (Actix-web, Rocket, Axum). These types define the API contract for PDF
//! generation endpoints.
//!
//! # Overview
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`PdfFromUrlRequest`] | Parameters for URL-to-PDF conversion |
//! | [`PdfFromHtmlRequest`] | Parameters for HTML-to-PDF conversion |
//! | [`PdfResponse`] | Successful PDF generation result |
//! | [`PdfServiceError`] | Error types with HTTP status mapping |
//! | [`ErrorResponse`] | JSON error response for API clients |
//! | [`PoolStatsResponse`] | Browser pool statistics |
//! | [`HealthResponse`] | Health check response |
//!
//! # Usage
//!
//! These types are used internally by framework integrations, but you can also
//! use them directly for custom handlers:
//!
//! ```rust,ignore
//! use html2pdf_api::service::{PdfFromUrlRequest, generate_pdf_from_url};
//!
//! let request = PdfFromUrlRequest {
//!     url: "https://example.com".to_string(),
//!     filename: Some("report.pdf".to_string()),
//!     landscape: Some(true),
//!     ..Default::default()
//! };
//!
//! // In a blocking context
//! let response = generate_pdf_from_url(&pool, &request)?;
//! println!("Generated {} bytes", response.data.len());
//! ```
//!
//! # Error Handling
//!
//! All errors are represented by [`PdfServiceError`], which provides:
//! - Human-readable error messages via [`Display`](std::fmt::Display)
//! - HTTP status codes via [`status_code()`](PdfServiceError::status_code)
//! - Machine-readable error codes via [`error_code()`](PdfServiceError::error_code)
//!
//! ```rust,ignore
//! use html2pdf_api::service::{PdfServiceError, ErrorResponse};
//!
//! fn handle_error(err: PdfServiceError) -> (u16, ErrorResponse) {
//!     let status = err.status_code();
//!     let body = ErrorResponse::from(err);
//!     (status, body)
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ============================================================================
// Request Types
// ============================================================================

/// Request parameters for converting a URL to PDF.
///
/// This struct represents the query parameters or request body for the
/// URL-to-PDF endpoint. All fields except `url` are optional with sensible
/// defaults.
///
/// # Required Fields
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | `url` | `String` | The URL to convert (must be a valid HTTP/HTTPS URL) |
///
/// # Optional Fields
///
/// | Field | Type | Default | Description |
/// |-------|------|---------|-------------|
/// | `filename` | `Option<String>` | `"document.pdf"` | Output filename for Content-Disposition header |
/// | `waitsecs` | `Option<u64>` | `5` | Seconds to wait for JavaScript execution |
/// | `landscape` | `Option<bool>` | `false` | Use landscape page orientation |
/// | `download` | `Option<bool>` | `false` | Force download vs inline display |
/// | `print_background` | `Option<bool>` | `true` | Include background colors/images |
///
/// # JavaScript Wait Behavior
///
/// The `waitsecs` parameter controls how long to wait for JavaScript to complete.
/// The service polls for `window.isPageDone === true` every 200ms. If your page
/// sets this flag, rendering completes immediately; otherwise, it waits the full
/// duration.
///
/// ```javascript
/// // In your web page, signal when rendering is complete:
/// window.isPageDone = true;
/// ```
///
/// # Examples
///
/// ## Basic URL conversion
///
/// ```rust
/// use html2pdf_api::service::PdfFromUrlRequest;
///
/// let request = PdfFromUrlRequest {
///     url: "https://example.com".to_string(),
///     ..Default::default()
/// };
///
/// assert_eq!(request.filename_or_default(), "document.pdf");
/// assert_eq!(request.wait_duration().as_secs(), 5);
/// ```
///
/// ## Landscape PDF with custom filename
///
/// ```rust
/// use html2pdf_api::service::PdfFromUrlRequest;
///
/// let request = PdfFromUrlRequest {
///     url: "https://example.com/report".to_string(),
///     filename: Some("quarterly-report.pdf".to_string()),
///     landscape: Some(true),
///     waitsecs: Some(10), // Complex charts need more time
///     ..Default::default()
/// };
///
/// assert!(request.is_landscape());
/// assert_eq!(request.wait_duration().as_secs(), 10);
/// ```
///
/// ## Force download (vs inline display)
///
/// ```rust
/// use html2pdf_api::service::PdfFromUrlRequest;
///
/// let request = PdfFromUrlRequest {
///     url: "https://example.com/invoice".to_string(),
///     filename: Some("invoice-2024.pdf".to_string()),
///     download: Some(true), // Forces "Content-Disposition: attachment"
///     ..Default::default()
/// };
///
/// assert!(request.is_download());
/// ```
///
/// # HTTP API Usage
///
/// ## As Query Parameters (GET request)
///
/// ```text
/// GET /pdf?url=https://example.com&filename=report.pdf&landscape=true&waitsecs=10
/// ```
///
/// ## As JSON Body (POST request, if supported)
///
/// ```json
/// {
///     "url": "https://example.com",
///     "filename": "report.pdf",
///     "landscape": true,
///     "waitsecs": 10,
///     "download": false,
///     "print_background": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PdfFromUrlRequest {
    /// The URL to convert to PDF.
    ///
    /// Must be a valid HTTP or HTTPS URL. Relative URLs are not supported.
    ///
    /// # Validation
    ///
    /// The URL is validated using the `url` crate before processing.
    /// Invalid URLs result in a [`PdfServiceError::InvalidUrl`] error.
    ///
    /// # Examples
    ///
    /// Valid URLs:
    /// - `https://example.com`
    /// - `https://example.com/path?query=value`
    /// - `http://localhost:3000/report`
    ///
    /// Invalid URLs:
    /// - `example.com` (missing scheme)
    /// - `/path/to/page` (relative URL)
    /// - `` (empty string)
    pub url: String,

    /// Output filename for the generated PDF.
    ///
    /// This value is used in the `Content-Disposition` header. If not provided,
    /// defaults to `"document.pdf"`.
    ///
    /// # Notes
    ///
    /// - The filename should include the `.pdf` extension
    /// - Special characters are not escaped; ensure valid filename characters
    /// - The browser may modify the filename based on its own rules
    #[serde(default)]
    pub filename: Option<String>,

    /// Seconds to wait for JavaScript execution before generating the PDF.
    ///
    /// Many modern web pages rely heavily on JavaScript for rendering content.
    /// This parameter controls the maximum wait time for the page to become ready.
    ///
    /// # Behavior
    ///
    /// 1. After navigation completes, the service waits up to `waitsecs` seconds
    /// 2. Every 200ms, it checks if `window.isPageDone === true`
    /// 3. If the flag is set, PDF generation begins immediately
    /// 4. If timeout is reached, PDF generation proceeds anyway
    ///
    /// # Default
    ///
    /// `5` seconds - suitable for most pages with moderate JavaScript.
    ///
    /// # Recommendations
    ///
    /// | Page Type | Recommended Value |
    /// |-----------|-------------------|
    /// | Static HTML | `1-2` |
    /// | Light JavaScript | `3-5` |
    /// | Heavy SPA (React, Vue) | `5-10` |
    /// | Complex charts/graphs | `10-15` |
    #[serde(default)]
    pub waitsecs: Option<u64>,

    /// Use landscape page orientation.
    ///
    /// When `true`, the PDF is generated in landscape mode (wider than tall).
    /// When `false` or not specified, portrait mode is used.
    ///
    /// # Default
    ///
    /// `false` (portrait orientation)
    ///
    /// # Use Cases
    ///
    /// - Wide tables or spreadsheets
    /// - Horizontal charts and graphs
    /// - Timeline visualizations
    /// - Presentation slides
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landscape: Option<bool>,

    /// Force download instead of inline display.
    ///
    /// Controls the `Content-Disposition` header behavior:
    ///
    /// | Value | Header | Browser Behavior |
    /// |-------|--------|------------------|
    /// | `false` (default) | `inline; filename="..."` | Display in browser |
    /// | `true` | `attachment; filename="..."` | Force download dialog |
    ///
    /// # Default
    ///
    /// `false` - PDF displays inline in the browser's PDF viewer.
    ///
    /// # Notes
    ///
    /// Browser behavior may vary. Some browsers always download PDFs
    /// regardless of this setting, depending on user preferences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<bool>,

    /// Include background colors and images in the PDF.
    ///
    /// When `true`, CSS background colors, background images, and other
    /// background graphics are included in the PDF output.
    ///
    /// # Default
    ///
    /// `true` - backgrounds are included by default.
    ///
    /// # Notes
    ///
    /// Setting this to `false` can reduce file size and is useful for
    /// print-friendly output where backgrounds are not desired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_background: Option<bool>,
}

impl PdfFromUrlRequest {
    /// Returns the filename, using `"document.pdf"` as the default.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfFromUrlRequest;
    ///
    /// let request = PdfFromUrlRequest::default();
    /// assert_eq!(request.filename_or_default(), "document.pdf");
    ///
    /// let request = PdfFromUrlRequest {
    ///     filename: Some("report.pdf".to_string()),
    ///     ..Default::default()
    /// };
    /// assert_eq!(request.filename_or_default(), "report.pdf");
    /// ```
    pub fn filename_or_default(&self) -> String {
        self.filename
            .clone()
            .unwrap_or_else(|| "document.pdf".to_string())
    }

    /// Returns the JavaScript wait duration.
    ///
    /// Defaults to 5 seconds if not specified.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfFromUrlRequest;
    /// use std::time::Duration;
    ///
    /// let request = PdfFromUrlRequest::default();
    /// assert_eq!(request.wait_duration(), Duration::from_secs(5));
    ///
    /// let request = PdfFromUrlRequest {
    ///     waitsecs: Some(10),
    ///     ..Default::default()
    /// };
    /// assert_eq!(request.wait_duration(), Duration::from_secs(10));
    /// ```
    pub fn wait_duration(&self) -> Duration {
        Duration::from_secs(self.waitsecs.unwrap_or(5))
    }

    /// Returns whether download mode is enabled.
    ///
    /// When `true`, the response includes `Content-Disposition: attachment`
    /// to force a download. When `false`, uses `inline` for in-browser display.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfFromUrlRequest;
    ///
    /// let request = PdfFromUrlRequest::default();
    /// assert!(!request.is_download()); // Default is inline
    ///
    /// let request = PdfFromUrlRequest {
    ///     download: Some(true),
    ///     ..Default::default()
    /// };
    /// assert!(request.is_download());
    /// ```
    pub fn is_download(&self) -> bool {
        self.download.unwrap_or(false)
    }

    /// Returns whether landscape orientation is enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfFromUrlRequest;
    ///
    /// let request = PdfFromUrlRequest::default();
    /// assert!(!request.is_landscape()); // Default is portrait
    ///
    /// let request = PdfFromUrlRequest {
    ///     landscape: Some(true),
    ///     ..Default::default()
    /// };
    /// assert!(request.is_landscape());
    /// ```
    pub fn is_landscape(&self) -> bool {
        self.landscape.unwrap_or(false)
    }

    /// Returns whether background printing is enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfFromUrlRequest;
    ///
    /// let request = PdfFromUrlRequest::default();
    /// assert!(request.print_background()); // Default is true
    ///
    /// let request = PdfFromUrlRequest {
    ///     print_background: Some(false),
    ///     ..Default::default()
    /// };
    /// assert!(!request.print_background());
    /// ```
    pub fn print_background(&self) -> bool {
        self.print_background.unwrap_or(true)
    }
}

/// Request parameters for converting HTML content to PDF.
///
/// This struct represents the request body for the HTML-to-PDF endpoint.
/// The HTML content is loaded via a data URL, so no external server is needed.
///
/// # Required Fields
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | `html` | `String` | Complete HTML document or fragment to convert |
///
/// # Optional Fields
///
/// | Field | Type | Default | Description |
/// |-------|------|---------|-------------|
/// | `filename` | `Option<String>` | `"document.pdf"` | Output filename |
/// | `waitsecs` | `Option<u64>` | `2` | Seconds to wait for JavaScript |
/// | `landscape` | `Option<bool>` | `false` | Use landscape orientation |
/// | `download` | `Option<bool>` | `false` | Force download vs inline |
/// | `print_background` | `Option<bool>` | `true` | Include backgrounds |
/// | `base_url` | `Option<String>` | `None` | Base URL for relative links (not yet implemented) |
///
/// # HTML Content Guidelines
///
/// ## Complete Document (Recommended)
///
/// For best results, provide a complete HTML document:
///
/// ```html
/// <!DOCTYPE html>
/// <html>
/// <head>
///     <meta charset="UTF-8">
///     <style>
///         body { font-family: Arial, sans-serif; }
///         /* Your styles here */
///     </style>
/// </head>
/// <body>
///     <h1>Your Content</h1>
///     <p>Your content here...</p>
/// </body>
/// </html>
/// ```
///
/// ## Fragment
///
/// HTML fragments are wrapped automatically, but styling may be limited:
///
/// ```html
/// <h1>Hello World</h1>
/// <p>This is a paragraph.</p>
/// ```
///
/// # External Resources
///
/// Since HTML is loaded via data URL, external resources have limitations:
///
/// | Resource Type | Behavior |
/// |---------------|----------|
/// | Inline styles | ✅ Works |
/// | Inline images (base64) | ✅ Works |
/// | External CSS (`<link>`) | ⚠️ May work if absolute URL |
/// | External images | ⚠️ May work if absolute URL |
/// | Relative URLs | ❌ Will not resolve |
/// | External fonts | ⚠️ May work if absolute URL |
///
/// For reliable results, embed all resources inline or use absolute URLs.
///
/// # Examples
///
/// ## Simple HTML conversion
///
/// ```rust
/// use html2pdf_api::service::PdfFromHtmlRequest;
///
/// let request = PdfFromHtmlRequest {
///     html: "<h1>Invoice #12345</h1><p>Amount: $99.99</p>".to_string(),
///     filename: Some("invoice.pdf".to_string()),
///     ..Default::default()
/// };
/// ```
///
/// ## Complete document with styling
///
/// ```rust
/// use html2pdf_api::service::PdfFromHtmlRequest;
///
/// let html = r#"
/// <!DOCTYPE html>
/// <html>
/// <head>
///     <style>
///         body { font-family: 'Helvetica', sans-serif; padding: 20px; }
///         h1 { color: #333; border-bottom: 2px solid #007bff; }
///         table { width: 100%; border-collapse: collapse; }
///         th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
///     </style>
/// </head>
/// <body>
///     <h1>Monthly Report</h1>
///     <table>
///         <tr><th>Item</th><th>Value</th></tr>
///         <tr><td>Revenue</td><td>$10,000</td></tr>
///     </table>
/// </body>
/// </html>
/// "#;
///
/// let request = PdfFromHtmlRequest {
///     html: html.to_string(),
///     filename: Some("report.pdf".to_string()),
///     landscape: Some(true),
///     ..Default::default()
/// };
/// ```
///
/// # HTTP API Usage
///
/// ```text
/// POST /pdf/html
/// Content-Type: application/json
///
/// {
///     "html": "<!DOCTYPE html><html>...</html>",
///     "filename": "document.pdf",
///     "landscape": false,
///     "download": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PdfFromHtmlRequest {
    /// HTML content to convert to PDF.
    ///
    /// Can be a complete HTML document or a fragment. Complete documents
    /// with proper `<!DOCTYPE>`, `<html>`, `<head>`, and `<body>` tags
    /// are recommended for consistent rendering.
    ///
    /// # Size Limits
    ///
    /// While there's no hard limit, very large HTML documents may:
    /// - Increase processing time
    /// - Consume more memory
    /// - Hit URL length limits (data URLs have browser-specific limits)
    ///
    /// For documents over 1MB, consider hosting the HTML and using
    /// [`PdfFromUrlRequest`] instead.
    pub html: String,

    /// Output filename for the generated PDF.
    ///
    /// Defaults to `"document.pdf"` if not specified.
    /// See [`PdfFromUrlRequest::filename`] for details.
    #[serde(default)]
    pub filename: Option<String>,

    /// Seconds to wait for JavaScript execution.
    ///
    /// Defaults to `2` seconds for HTML content (shorter than URL conversion
    /// since the content is already loaded).
    ///
    /// Increase this value if your HTML includes JavaScript that modifies
    /// the DOM after initial load.
    #[serde(default)]
    pub waitsecs: Option<u64>,

    /// Use landscape page orientation.
    ///
    /// See [`PdfFromUrlRequest::landscape`] for details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landscape: Option<bool>,

    /// Force download instead of inline display.
    ///
    /// See [`PdfFromUrlRequest::download`] for details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<bool>,

    /// Include background colors and images.
    ///
    /// See [`PdfFromUrlRequest::print_background`] for details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_background: Option<bool>,

    /// Base URL for resolving relative links.
    ///
    /// **Note:** This feature is not yet implemented. Relative URLs in
    /// HTML content will not resolve correctly. Use absolute URLs for
    /// external resources.
    ///
    /// # Future Behavior
    ///
    /// When implemented, this will allow relative URLs in the HTML to
    /// resolve against the specified base:
    ///
    /// ```json
    /// {
    ///     "html": "<img src=\"/images/logo.png\">",
    ///     "base_url": "https://example.com"
    /// }
    /// ```
    ///
    /// Would resolve the image to `https://example.com/images/logo.png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl PdfFromHtmlRequest {
    /// Returns the filename, using `"document.pdf"` as the default.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfFromHtmlRequest;
    ///
    /// let request = PdfFromHtmlRequest::default();
    /// assert_eq!(request.filename_or_default(), "document.pdf");
    /// ```
    pub fn filename_or_default(&self) -> String {
        self.filename
            .clone()
            .unwrap_or_else(|| "document.pdf".to_string())
    }

    /// Returns the JavaScript wait duration.
    ///
    /// Defaults to 2 seconds for HTML content (shorter than URL conversion).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfFromHtmlRequest;
    /// use std::time::Duration;
    ///
    /// let request = PdfFromHtmlRequest::default();
    /// assert_eq!(request.wait_duration(), Duration::from_secs(2));
    /// ```
    pub fn wait_duration(&self) -> Duration {
        Duration::from_secs(self.waitsecs.unwrap_or(2))
    }

    /// Returns whether download mode is enabled.
    ///
    /// See [`PdfFromUrlRequest::is_download`] for details.
    pub fn is_download(&self) -> bool {
        self.download.unwrap_or(false)
    }

    /// Returns whether landscape orientation is enabled.
    ///
    /// See [`PdfFromUrlRequest::is_landscape`] for details.
    pub fn is_landscape(&self) -> bool {
        self.landscape.unwrap_or(false)
    }

    /// Returns whether background printing is enabled.
    ///
    /// See [`PdfFromUrlRequest::print_background`] for details.
    pub fn print_background(&self) -> bool {
        self.print_background.unwrap_or(true)
    }
}

// ============================================================================
// Response Types
// ============================================================================

/// Successful PDF generation result.
///
/// Contains the generated PDF binary data along with metadata for building
/// the HTTP response. This type is returned by the core service functions
/// and converted to framework-specific responses by the integrations.
///
/// # Fields
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | `data` | `Vec<u8>` | Raw PDF binary data |
/// | `filename` | `String` | Suggested filename for download |
/// | `force_download` | `bool` | Whether to force download vs inline display |
///
/// # HTTP Response Headers
///
/// When converted to an HTTP response, this generates:
///
/// ```text
/// Content-Type: application/pdf
/// Content-Disposition: inline; filename="document.pdf"  (or attachment if force_download)
/// Cache-Control: no-cache
/// ```
///
/// # Examples
///
/// ```rust
/// use html2pdf_api::service::PdfResponse;
///
/// let response = PdfResponse::new(
///     vec![0x25, 0x50, 0x44, 0x46], // PDF magic bytes
///     "report.pdf".to_string(),
///     false, // inline display
/// );
///
/// assert_eq!(response.content_disposition(), "inline; filename=\"report.pdf\"");
/// ```
#[derive(Debug, Clone)]
pub struct PdfResponse {
    /// The generated PDF as raw binary data.
    ///
    /// This is the complete PDF file content, ready to be sent as the
    /// HTTP response body or written to a file.
    ///
    /// # PDF Structure
    ///
    /// Valid PDF data always starts with `%PDF-` (bytes `25 50 44 46 2D`).
    /// You can verify the data is valid by checking this header:
    ///
    /// ```rust
    /// fn is_valid_pdf(data: &[u8]) -> bool {
    ///     data.starts_with(b"%PDF-")
    /// }
    /// ```
    pub data: Vec<u8>,

    /// Suggested filename for the PDF download.
    ///
    /// This is used in the `Content-Disposition` header. The actual
    /// filename used by the browser may differ based on user settings
    /// or browser behavior.
    pub filename: String,

    /// Whether to force download instead of inline display.
    ///
    /// - `true`: Uses `Content-Disposition: attachment` (forces download)
    /// - `false`: Uses `Content-Disposition: inline` (displays in browser)
    pub force_download: bool,
}

impl PdfResponse {
    /// Creates a new PDF response.
    ///
    /// # Arguments
    ///
    /// * `data` - The raw PDF binary data
    /// * `filename` - Suggested filename for the download
    /// * `force_download` - Whether to force download vs inline display
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfResponse;
    ///
    /// // For inline display
    /// let response = PdfResponse::new(
    ///     pdf_bytes,
    ///     "invoice.pdf".to_string(),
    ///     false,
    /// );
    ///
    /// // For forced download
    /// let response = PdfResponse::new(
    ///     pdf_bytes,
    ///     "confidential-report.pdf".to_string(),
    ///     true,
    /// );
    /// ```
    pub fn new(data: Vec<u8>, filename: String, force_download: bool) -> Self {
        Self {
            data,
            filename,
            force_download,
        }
    }

    /// Generates the `Content-Disposition` header value.
    ///
    /// Returns a properly formatted header value based on the
    /// `force_download` setting.
    ///
    /// # Returns
    ///
    /// - `"attachment; filename=\"{filename}\""` if `force_download` is `true`
    /// - `"inline; filename=\"{filename}\""` if `force_download` is `false`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfResponse;
    ///
    /// let inline = PdfResponse::new(vec![], "doc.pdf".to_string(), false);
    /// assert_eq!(inline.content_disposition(), "inline; filename=\"doc.pdf\"");
    ///
    /// let download = PdfResponse::new(vec![], "doc.pdf".to_string(), true);
    /// assert_eq!(download.content_disposition(), "attachment; filename=\"doc.pdf\"");
    /// ```
    pub fn content_disposition(&self) -> String {
        let disposition_type = if self.force_download {
            "attachment"
        } else {
            "inline"
        };
        format!("{}; filename=\"{}\"", disposition_type, self.filename)
    }

    /// Returns the size of the PDF data in bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfResponse;
    ///
    /// let response = PdfResponse::new(vec![0; 1024], "doc.pdf".to_string(), false);
    /// assert_eq!(response.size(), 1024);
    /// ```
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// Browser pool statistics response.
///
/// Provides real-time metrics about the browser pool state. Useful for
/// monitoring, debugging, and capacity planning.
///
/// # Fields
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | `available` | `usize` | Browsers ready to handle requests |
/// | `active` | `usize` | Browsers currently in use |
/// | `total` | `usize` | Total browsers (available + active) |
///
/// # Understanding the Metrics
///
/// ```text
/// total = available + active
///
/// ┌─────────────────────────────────────┐
/// │           Browser Pool              │
/// │  ┌─────────────┬─────────────────┐  │
/// │  │  Available  │     Active      │  │
/// │  │   (idle)    │   (in use)      │  │
/// │  │  [B1] [B2]  │  [B3] [B4] [B5] │  │
/// │  └─────────────┴─────────────────┘  │
/// └─────────────────────────────────────┘
///
/// available = 2, active = 3, total = 5
/// ```
///
/// # Health Indicators
///
/// | Condition | Meaning |
/// |-----------|---------|
/// | `available > 0` | Pool can handle new requests immediately |
/// | `available == 0 && active < max` | New requests will create browsers |
/// | `available == 0 && active == max` | Pool at capacity, requests may queue |
///
/// # HTTP API Usage
///
/// ```text
/// GET /pool/stats
///
/// Response:
/// {
///     "available": 3,
///     "active": 2,
///     "total": 5
/// }
/// ```
///
/// # Examples
///
/// ```rust
/// use html2pdf_api::service::PoolStatsResponse;
///
/// let stats = PoolStatsResponse {
///     available: 3,
///     active: 2,
///     total: 5,
/// };
///
/// // Check if pool has capacity
/// let has_capacity = stats.available > 0;
///
/// // Calculate utilization
/// let utilization = stats.active as f64 / stats.total as f64 * 100.0;
/// println!("Pool utilization: {:.1}%", utilization); // "Pool utilization: 40.0%"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatsResponse {
    /// Number of browsers available (idle) in the pool.
    ///
    /// These browsers are ready to handle requests immediately without
    /// the overhead of launching a new browser process.
    pub available: usize,

    /// Number of browsers currently in use (checked out).
    ///
    /// These browsers are actively processing PDF generation requests.
    /// They will return to the available pool when the request completes.
    pub active: usize,

    /// Total number of browsers in the pool.
    ///
    /// This equals `available + active`. The maximum value is determined
    /// by the pool's `max_pool_size` configuration.
    pub total: usize,
}

/// Health check response.
///
/// Simple response indicating the service is running. Used by load balancers,
/// container orchestrators (Kubernetes), and monitoring systems.
///
/// # HTTP API Usage
///
/// ```text
/// GET /health
///
/// Response (200 OK):
/// {
///     "status": "healthy",
///     "service": "html2pdf-api"
/// }
/// ```
///
/// # Kubernetes Liveness Probe
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
/// # Examples
///
/// ```rust
/// use html2pdf_api::service::HealthResponse;
///
/// let response = HealthResponse::default();
/// assert_eq!(response.status, "healthy");
/// assert_eq!(response.service, "html2pdf-api");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Health status, always `"healthy"` when the endpoint responds.
    ///
    /// If the service is unhealthy, the endpoint won't respond at all
    /// (connection refused or timeout).
    pub status: String,

    /// Service name identifier.
    ///
    /// Useful when multiple services share a health check endpoint
    /// or for logging purposes.
    pub service: String,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            status: "healthy".to_string(),
            service: "html2pdf-api".to_string(),
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during PDF generation.
///
/// Each variant maps to a specific HTTP status code and error code for
/// consistent API responses. Use [`status_code()`](Self::status_code) and
/// [`error_code()`](Self::error_code) to build HTTP responses.
///
/// # HTTP Status Code Mapping
///
/// | Error Type | HTTP Status | Error Code |
/// |------------|-------------|------------|
/// | [`InvalidUrl`](Self::InvalidUrl) | 400 Bad Request | `INVALID_URL` |
/// | [`EmptyHtml`](Self::EmptyHtml) | 400 Bad Request | `EMPTY_HTML` |
/// | [`PoolLockFailed`](Self::PoolLockFailed) | 500 Internal Server Error | `POOL_LOCK_FAILED` |
/// | [`BrowserUnavailable`](Self::BrowserUnavailable) | 503 Service Unavailable | `BROWSER_UNAVAILABLE` |
/// | [`TabCreationFailed`](Self::TabCreationFailed) | 500 Internal Server Error | `TAB_CREATION_FAILED` |
/// | [`NavigationFailed`](Self::NavigationFailed) | 502 Bad Gateway | `NAVIGATION_FAILED` |
/// | [`NavigationTimeout`](Self::NavigationTimeout) | 504 Gateway Timeout | `NAVIGATION_TIMEOUT` |
/// | [`PdfGenerationFailed`](Self::PdfGenerationFailed) | 502 Bad Gateway | `PDF_GENERATION_FAILED` |
/// | [`Timeout`](Self::Timeout) | 504 Gateway Timeout | `TIMEOUT` |
/// | [`PoolShuttingDown`](Self::PoolShuttingDown) | 503 Service Unavailable | `POOL_SHUTTING_DOWN` |
/// | [`Internal`](Self::Internal) | 500 Internal Server Error | `INTERNAL_ERROR` |
///
/// # Error Categories
///
/// ## Client Errors (4xx)
///
/// These indicate problems with the request that the client can fix:
/// - [`InvalidUrl`](Self::InvalidUrl) - Malformed or missing URL
/// - [`EmptyHtml`](Self::EmptyHtml) - Empty HTML content
///
/// ## Server Errors (5xx)
///
/// These indicate problems on the server side:
/// - [`PoolLockFailed`](Self::PoolLockFailed) - Internal synchronization issue
/// - [`TabCreationFailed`](Self::TabCreationFailed) - Browser tab creation failed
/// - [`Internal`](Self::Internal) - Unexpected internal error
///
/// ## Upstream Errors (502/504)
///
/// These indicate problems with the target URL or browser:
/// - [`NavigationFailed`](Self::NavigationFailed) - Failed to load the URL
/// - [`NavigationTimeout`](Self::NavigationTimeout) - URL took too long to load
/// - [`PdfGenerationFailed`](Self::PdfGenerationFailed) - Browser failed to generate PDF
/// - [`Timeout`](Self::Timeout) - Overall operation timeout
///
/// ## Availability Errors (503)
///
/// These indicate the service is temporarily unavailable:
/// - [`BrowserUnavailable`](Self::BrowserUnavailable) - No browsers available in pool
/// - [`PoolShuttingDown`](Self::PoolShuttingDown) - Service is shutting down
///
/// # Examples
///
/// ## Error Handling
///
/// ```rust
/// use html2pdf_api::service::{PdfServiceError, ErrorResponse};
///
/// fn handle_result(result: Result<Vec<u8>, PdfServiceError>) -> (u16, String) {
///     match result {
///         Ok(pdf) => (200, format!("Generated {} bytes", pdf.len())),
///         Err(e) => {
///             let status = e.status_code();
///             let response = ErrorResponse::from(&e);
///             (status, serde_json::to_string(&response).unwrap())
///         }
///     }
/// }
/// ```
///
/// ## Retry Logic
///
/// ```rust
/// use html2pdf_api::service::PdfServiceError;
///
/// fn should_retry(error: &PdfServiceError) -> bool {
///     match error {
///         // Transient errors - worth retrying
///         PdfServiceError::BrowserUnavailable(_) => true,
///         PdfServiceError::NavigationTimeout(_) => true,
///         PdfServiceError::Timeout(_) => true,
///         
///         // Client errors - don't retry
///         PdfServiceError::InvalidUrl(_) => false,
///         PdfServiceError::EmptyHtml => false,
///         
///         // Server errors - maybe retry with backoff
///         PdfServiceError::PoolLockFailed(_) => true,
///         
///         // Fatal errors - don't retry
///         PdfServiceError::PoolShuttingDown => false,
///         
///         _ => false,
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub enum PdfServiceError {
    /// The provided URL is invalid or malformed.
    ///
    /// # Causes
    ///
    /// - Empty URL string
    /// - Missing URL scheme (e.g., `example.com` instead of `https://example.com`)
    /// - Invalid URL format
    ///
    /// # Resolution
    ///
    /// Provide a valid HTTP or HTTPS URL with proper formatting.
    ///
    /// # Example Response
    ///
    /// ```json
    /// {
    ///     "error": "Invalid URL: relative URL without a base",
    ///     "code": "INVALID_URL"
    /// }
    /// ```
    InvalidUrl(String),

    /// The HTML content is empty or contains only whitespace.
    ///
    /// # Causes
    ///
    /// - Empty `html` field in request
    /// - HTML field contains only whitespace
    ///
    /// # Resolution
    ///
    /// Provide non-empty HTML content.
    ///
    /// # Example Response
    ///
    /// ```json
    /// {
    ///     "error": "HTML content is required",
    ///     "code": "EMPTY_HTML"
    /// }
    /// ```
    EmptyHtml,

    /// Failed to acquire the browser pool lock.
    ///
    /// This is an internal error indicating a synchronization problem,
    /// typically caused by a poisoned mutex (previous panic while holding lock).
    ///
    /// # Causes
    ///
    /// - Mutex was poisoned by a previous panic
    /// - Deadlock condition (should not happen with correct implementation)
    ///
    /// # Resolution
    ///
    /// This is a server-side issue. Restarting the service may help.
    /// Check logs for previous panic messages.
    PoolLockFailed(String),

    /// No browser is available in the pool.
    ///
    /// All browsers are currently in use and the pool is at maximum capacity.
    ///
    /// # Causes
    ///
    /// - High request volume exceeding pool capacity
    /// - Slow PDF generation causing browser exhaustion
    /// - Browsers failing health checks faster than replacement
    ///
    /// # Resolution
    ///
    /// - Retry after a short delay
    /// - Increase `max_pool_size` configuration
    /// - Reduce `waitsecs` to speed up PDF generation
    BrowserUnavailable(String),

    /// Failed to create a new browser tab.
    ///
    /// The browser instance is available but couldn't create a new tab.
    ///
    /// # Causes
    ///
    /// - Browser process is unresponsive
    /// - System resource exhaustion (file descriptors, memory)
    /// - Browser crashed
    ///
    /// # Resolution
    ///
    /// The pool should automatically replace unhealthy browsers.
    /// If persistent, check system resources and browser logs.
    TabCreationFailed(String),

    /// Failed to navigate to the specified URL.
    ///
    /// The browser couldn't load the target URL.
    ///
    /// # Causes
    ///
    /// - URL doesn't exist (404)
    /// - Server error at target URL (5xx)
    /// - SSL/TLS certificate issues
    /// - Network connectivity problems
    /// - Target server refusing connections
    ///
    /// # Resolution
    ///
    /// - Verify the URL is accessible
    /// - Check if the target server is running
    /// - Verify SSL certificates if using HTTPS
    NavigationFailed(String),

    /// Navigation to the URL timed out.
    ///
    /// The browser started loading the URL but didn't complete within
    /// the allowed time.
    ///
    /// # Causes
    ///
    /// - Target server is slow to respond
    /// - Large page with many resources
    /// - Network latency issues
    /// - Target server overloaded
    ///
    /// # Resolution
    ///
    /// - Check target server performance
    /// - Increase timeout if needed (via configuration)
    /// - Optimize the target page
    NavigationTimeout(String),

    /// Failed to generate PDF from the loaded page.
    ///
    /// The page loaded successfully but PDF generation failed.
    ///
    /// # Causes
    ///
    /// - Complex page layout that can't be rendered
    /// - Browser rendering issues
    /// - Memory exhaustion during rendering
    /// - Invalid page content
    ///
    /// # Resolution
    ///
    /// - Simplify the page layout
    /// - Check for rendering errors in browser console
    /// - Ensure sufficient system memory
    PdfGenerationFailed(String),

    /// The overall operation timed out.
    ///
    /// The complete PDF generation operation (including queue time,
    /// navigation, and rendering) exceeded the maximum allowed duration.
    ///
    /// # Causes
    ///
    /// - High system load
    /// - Very large or complex pages
    /// - Slow target server
    /// - Insufficient `waitsecs` for JavaScript completion
    ///
    /// # Resolution
    ///
    /// - Retry the request
    /// - Increase timeout configuration
    /// - Reduce page complexity
    Timeout(String),

    /// The browser pool is shutting down.
    ///
    /// The service is in the process of graceful shutdown and not
    /// accepting new requests.
    ///
    /// # Causes
    ///
    /// - Service restart initiated
    /// - Graceful shutdown in progress
    /// - Container/pod termination
    ///
    /// # Resolution
    ///
    /// Wait for the service to restart and retry. Do not retry
    /// immediately as the service is intentionally stopping.
    PoolShuttingDown,

    /// An unexpected internal error occurred.
    ///
    /// Catch-all for errors that don't fit other categories.
    ///
    /// # Causes
    ///
    /// - Unexpected panic
    /// - Unhandled error condition
    /// - Bug in the application
    ///
    /// # Resolution
    ///
    /// Check server logs for details. Report persistent issues
    /// with reproduction steps.
    Internal(String),
}

impl std::fmt::Display for PdfServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            Self::EmptyHtml => write!(f, "HTML content is required"),
            Self::PoolLockFailed(msg) => write!(f, "Failed to lock pool: {}", msg),
            Self::BrowserUnavailable(msg) => write!(f, "Browser unavailable: {}", msg),
            Self::TabCreationFailed(msg) => write!(f, "Failed to create tab: {}", msg),
            Self::NavigationFailed(msg) => write!(f, "Navigation failed: {}", msg),
            Self::NavigationTimeout(msg) => write!(f, "Navigation timeout: {}", msg),
            Self::PdfGenerationFailed(msg) => write!(f, "PDF generation failed: {}", msg),
            Self::Timeout(msg) => write!(f, "Operation timeout: {}", msg),
            Self::PoolShuttingDown => write!(f, "Pool is shutting down"),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for PdfServiceError {}

impl PdfServiceError {
    /// Returns the HTTP status code for this error.
    ///
    /// Maps each error type to an appropriate HTTP status code following
    /// REST conventions.
    ///
    /// # Status Code Categories
    ///
    /// | Range | Category | Meaning |
    /// |-------|----------|---------|
    /// | 400-499 | Client Error | Request problem (client can fix) |
    /// | 500-599 | Server Error | Server problem (client can't fix) |
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfServiceError;
    ///
    /// let error = PdfServiceError::InvalidUrl("missing scheme".to_string());
    /// assert_eq!(error.status_code(), 400);
    ///
    /// let error = PdfServiceError::BrowserUnavailable("pool exhausted".to_string());
    /// assert_eq!(error.status_code(), 503);
    ///
    /// let error = PdfServiceError::NavigationTimeout("30s exceeded".to_string());
    /// assert_eq!(error.status_code(), 504);
    /// ```
    pub fn status_code(&self) -> u16 {
        match self {
            // Client errors (4xx)
            Self::InvalidUrl(_) | Self::EmptyHtml => 400,

            // Server errors (5xx)
            Self::PoolLockFailed(_) | Self::TabCreationFailed(_) | Self::Internal(_) => 500,

            // Bad gateway (upstream errors)
            Self::NavigationFailed(_) | Self::PdfGenerationFailed(_) => 502,

            // Service unavailable
            Self::BrowserUnavailable(_) | Self::PoolShuttingDown => 503,

            // Gateway timeout
            Self::NavigationTimeout(_) | Self::Timeout(_) => 504,
        }
    }

    /// Returns a machine-readable error code.
    ///
    /// These codes are stable and can be used for programmatic error handling
    /// by API clients. They are returned in the `code` field of error responses.
    ///
    /// # Error Codes
    ///
    /// | Code | Error Type |
    /// |------|------------|
    /// | `INVALID_URL` | Invalid or malformed URL |
    /// | `EMPTY_HTML` | Empty HTML content |
    /// | `POOL_LOCK_FAILED` | Internal pool lock error |
    /// | `BROWSER_UNAVAILABLE` | No browsers available |
    /// | `TAB_CREATION_FAILED` | Failed to create browser tab |
    /// | `NAVIGATION_FAILED` | Failed to load URL |
    /// | `NAVIGATION_TIMEOUT` | URL load timeout |
    /// | `PDF_GENERATION_FAILED` | Failed to generate PDF |
    /// | `TIMEOUT` | Overall operation timeout |
    /// | `POOL_SHUTTING_DOWN` | Service shutting down |
    /// | `INTERNAL_ERROR` | Unexpected internal error |
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfServiceError;
    ///
    /// let error = PdfServiceError::InvalidUrl("test".to_string());
    /// assert_eq!(error.error_code(), "INVALID_URL");
    ///
    /// // Client-side handling
    /// match error.error_code() {
    ///     "INVALID_URL" | "EMPTY_HTML" => println!("Fix your request"),
    ///     "BROWSER_UNAVAILABLE" | "TIMEOUT" => println!("Retry later"),
    ///     _ => println!("Contact support"),
    /// }
    /// ```
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidUrl(_) => "INVALID_URL",
            Self::EmptyHtml => "EMPTY_HTML",
            Self::PoolLockFailed(_) => "POOL_LOCK_FAILED",
            Self::BrowserUnavailable(_) => "BROWSER_UNAVAILABLE",
            Self::TabCreationFailed(_) => "TAB_CREATION_FAILED",
            Self::NavigationFailed(_) => "NAVIGATION_FAILED",
            Self::NavigationTimeout(_) => "NAVIGATION_TIMEOUT",
            Self::PdfGenerationFailed(_) => "PDF_GENERATION_FAILED",
            Self::Timeout(_) => "TIMEOUT",
            Self::PoolShuttingDown => "POOL_SHUTTING_DOWN",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Returns `true` if this error is likely transient and worth retrying.
    ///
    /// Transient errors are typically caused by temporary conditions that
    /// may resolve on their own. Client errors (4xx) are never transient
    /// as they require the client to change the request.
    ///
    /// # Retryable Errors
    ///
    /// | Error | Retryable | Reason |
    /// |-------|-----------|--------|
    /// | `BrowserUnavailable` | ✅ | Pool may free up |
    /// | `NavigationTimeout` | ✅ | Network may recover |
    /// | `Timeout` | ✅ | Load may decrease |
    /// | `PoolLockFailed` | ✅ | Rare, may recover |
    /// | `InvalidUrl` | ❌ | Client must fix |
    /// | `EmptyHtml` | ❌ | Client must fix |
    /// | `PoolShuttingDown` | ❌ | Intentional shutdown |
    ///
    /// # Examples
    ///
    /// ```rust
    /// use html2pdf_api::service::PdfServiceError;
    ///
    /// let error = PdfServiceError::BrowserUnavailable("pool full".to_string());
    /// if error.is_retryable() {
    ///     // Wait and retry
    ///     tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    /// }
    ///
    /// let error = PdfServiceError::InvalidUrl("bad url".to_string());
    /// assert!(!error.is_retryable()); // Don't retry, fix the URL
    /// ```
    pub fn is_retryable(&self) -> bool {
        match self {
            // Transient - worth retrying
            Self::BrowserUnavailable(_)
            | Self::NavigationTimeout(_)
            | Self::Timeout(_)
            | Self::PoolLockFailed(_)
            | Self::TabCreationFailed(_) => true,

            // Client errors - must fix request
            Self::InvalidUrl(_) | Self::EmptyHtml => false,

            // Fatal - don't retry
            Self::PoolShuttingDown => false,

            // Upstream errors - maybe retry
            Self::NavigationFailed(_) | Self::PdfGenerationFailed(_) => true,

            // Unknown - conservative retry
            Self::Internal(_) => false,
        }
    }
}

/// JSON error response for API clients.
///
/// A standardized error response format returned by all PDF endpoints
/// when an error occurs. This structure makes it easy for API clients
/// to parse and handle errors programmatically.
///
/// # Fields
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | `error` | `String` | Human-readable error message |
/// | `code` | `String` | Machine-readable error code |
///
/// # Response Format
///
/// ```json
/// {
///     "error": "Invalid URL: relative URL without a base",
///     "code": "INVALID_URL"
/// }
/// ```
///
/// # Client-Side Handling
///
/// ```typescript
/// // TypeScript example
/// interface ErrorResponse {
///     error: string;
///     code: string;
/// }
///
/// async function convertToPdf(url: string): Promise<Blob> {
///     const response = await fetch(`/pdf?url=${encodeURIComponent(url)}`);
///     
///     if (!response.ok) {
///         const error: ErrorResponse = await response.json();
///         
///         switch (error.code) {
///             case 'INVALID_URL':
///                 throw new Error('Please provide a valid URL');
///             case 'BROWSER_UNAVAILABLE':
///                 // Retry after delay
///                 await sleep(1000);
///                 return convertToPdf(url);
///             default:
///                 throw new Error(error.error);
///         }
///     }
///     
///     return response.blob();
/// }
/// ```
///
/// # Examples
///
/// ```rust
/// use html2pdf_api::service::{PdfServiceError, ErrorResponse};
///
/// let error = PdfServiceError::InvalidUrl("missing scheme".to_string());
/// let response = ErrorResponse::from(error);
///
/// assert_eq!(response.code, "INVALID_URL");
/// assert!(response.error.contains("Invalid URL"));
///
/// // Serialize to JSON
/// let json = serde_json::to_string(&response).unwrap();
/// assert!(json.contains("INVALID_URL"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Human-readable error message.
    ///
    /// This message is intended for developers and logs. It may contain
    /// technical details about the error cause. For user-facing messages,
    /// consider mapping the `code` field to localized strings.
    pub error: String,

    /// Machine-readable error code.
    ///
    /// A stable, uppercase identifier for the error type. Use this for
    /// programmatic error handling rather than parsing the `error` message.
    ///
    /// See [`PdfServiceError::error_code()`] for the complete list of codes.
    pub code: String,
}

impl From<&PdfServiceError> for ErrorResponse {
    fn from(err: &PdfServiceError) -> Self {
        Self {
            error: err.to_string(),
            code: err.error_code().to_string(),
        }
    }
}

impl From<PdfServiceError> for ErrorResponse {
    fn from(err: PdfServiceError) -> Self {
        Self::from(&err)
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_from_url_request_defaults() {
        let request = PdfFromUrlRequest::default();

        assert_eq!(request.filename_or_default(), "document.pdf");
        assert_eq!(request.wait_duration(), Duration::from_secs(5));
        assert!(!request.is_download());
        assert!(!request.is_landscape());
        assert!(request.print_background());
    }

    #[test]
    fn test_pdf_from_url_request_custom_values() {
        let request = PdfFromUrlRequest {
            url: "https://example.com".to_string(),
            filename: Some("custom.pdf".to_string()),
            waitsecs: Some(10),
            landscape: Some(true),
            download: Some(true),
            print_background: Some(false),
        };

        assert_eq!(request.filename_or_default(), "custom.pdf");
        assert_eq!(request.wait_duration(), Duration::from_secs(10));
        assert!(request.is_download());
        assert!(request.is_landscape());
        assert!(!request.print_background());
    }

    #[test]
    fn test_pdf_from_html_request_defaults() {
        let request = PdfFromHtmlRequest::default();

        assert_eq!(request.filename_or_default(), "document.pdf");
        assert_eq!(request.wait_duration(), Duration::from_secs(2)); // Shorter default
        assert!(!request.is_download());
        assert!(!request.is_landscape());
        assert!(request.print_background());
    }

    #[test]
    fn test_pdf_response_content_disposition() {
        let inline = PdfResponse::new(vec![], "doc.pdf".to_string(), false);
        assert_eq!(inline.content_disposition(), "inline; filename=\"doc.pdf\"");

        let attachment = PdfResponse::new(vec![], "doc.pdf".to_string(), true);
        assert_eq!(
            attachment.content_disposition(),
            "attachment; filename=\"doc.pdf\""
        );
    }

    #[test]
    fn test_pdf_response_size() {
        let response = PdfResponse::new(vec![0; 1024], "doc.pdf".to_string(), false);
        assert_eq!(response.size(), 1024);
    }

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            PdfServiceError::InvalidUrl("".to_string()).status_code(),
            400
        );
        assert_eq!(PdfServiceError::EmptyHtml.status_code(), 400);
        assert_eq!(
            PdfServiceError::PoolLockFailed("".to_string()).status_code(),
            500
        );
        assert_eq!(
            PdfServiceError::BrowserUnavailable("".to_string()).status_code(),
            503
        );
        assert_eq!(
            PdfServiceError::NavigationFailed("".to_string()).status_code(),
            502
        );
        assert_eq!(
            PdfServiceError::NavigationTimeout("".to_string()).status_code(),
            504
        );
        assert_eq!(PdfServiceError::Timeout("".to_string()).status_code(), 504);
        assert_eq!(PdfServiceError::PoolShuttingDown.status_code(), 503);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            PdfServiceError::InvalidUrl("".to_string()).error_code(),
            "INVALID_URL"
        );
        assert_eq!(PdfServiceError::EmptyHtml.error_code(), "EMPTY_HTML");
        assert_eq!(
            PdfServiceError::PoolShuttingDown.error_code(),
            "POOL_SHUTTING_DOWN"
        );
    }

    #[test]
    fn test_error_retryable() {
        assert!(PdfServiceError::BrowserUnavailable("".to_string()).is_retryable());
        assert!(PdfServiceError::Timeout("".to_string()).is_retryable());
        assert!(!PdfServiceError::InvalidUrl("".to_string()).is_retryable());
        assert!(!PdfServiceError::EmptyHtml.is_retryable());
        assert!(!PdfServiceError::PoolShuttingDown.is_retryable());
    }

    #[test]
    fn test_error_response_from_error() {
        let error = PdfServiceError::InvalidUrl("test error".to_string());
        let response = ErrorResponse::from(error);

        assert_eq!(response.code, "INVALID_URL");
        assert!(response.error.contains("Invalid URL"));
    }

    #[test]
    fn test_health_response_default() {
        let response = HealthResponse::default();
        assert_eq!(response.status, "healthy");
        assert_eq!(response.service, "html2pdf-api");
    }
}
