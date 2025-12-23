//! Convenient imports for common usage patterns.
//!
//! This module re-exports the most commonly used types from `html2pdf-api`,
//! allowing you to quickly get started with a single import.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//! ```
//!
//! This single line gives you access to all the core types needed to create
//! and use a browser pool for PDF generation.
//!
//! # What's Included
//!
//! ## Core Types (Always Available)
//!
//! | Type | Description |
//! |------|-------------|
//! | [`BrowserPool`] | Main pool for managing browser instances |
//! | [`BrowserPoolBuilder`] | Builder for creating configured pools |
//! | [`BrowserPoolConfig`] | Configuration settings for the pool |
//! | [`BrowserPoolConfigBuilder`] | Builder for creating configurations |
//! | [`BrowserPoolError`] | Error type for pool operations |
//! | [`Result`] | Type alias for `Result<T, BrowserPoolError>` |
//! | [`BrowserHandle`] | RAII handle for checked-out browsers |
//! | [`PoolStats`] | Real-time pool statistics |
//! | [`BrowserFactory`] | Trait for browser creation strategies |
//! | [`ChromeBrowserFactory`] | Default Chrome/Chromium factory |
//! | [`Healthcheck`] | Trait for browser health checking |
//! | [`SharedBrowserPool`] | Type alias for `Arc<Mutex<BrowserPool>>` |
//!
//! ## Standard Library Re-exports
//!
//! For convenience, commonly needed types are re-exported:
//!
//! | Type | Description |
//! |------|-------------|
//! | [`Arc`] | Thread-safe reference counting |
//! | [`Mutex`] | Mutual exclusion lock |
//!
//! ## Feature-Gated Exports
//!
//! Additional exports are available with feature flags:
//!
//! ### `env-config` Feature
//!
//! | Export | Description |
//! |--------|-------------|
//! | [`init_browser_pool`] | Initialize pool from environment variables |
//! | [`from_env`] | Load configuration from environment |
//! | [`chrome_path_from_env`] | Get Chrome path from `CHROME_PATH` env var |
//!
//! ### `actix-integration` Feature
//!
//! | Export | Description |
//! |--------|-------------|
//! | [`actix`] module | Actix-web handlers and helpers |
//!
//! ### `rocket-integration` Feature
//!
//! | Export | Description |
//! |--------|-------------|
//! | [`rocket`] module | Rocket handlers and helpers |
//!
//! ### `axum-integration` Feature
//!
//! | Export | Description |
//! |--------|-------------|
//! | [`axum`] module | Axum handlers and router |
//!
//! ### Any Integration Feature
//!
//! When any integration feature is enabled, service types are also available:
//!
//! | Type | Description |
//! |------|-------------|
//! | [`PdfFromUrlRequest`] | Request parameters for URL-to-PDF |
//! | [`PdfFromHtmlRequest`] | Request parameters for HTML-to-PDF |
//! | [`PdfResponse`] | Successful PDF generation result |
//! | [`PdfServiceError`] | Service-level errors |
//! | [`ErrorResponse`] | JSON error response format |
//! | [`PoolStatsResponse`] | Pool statistics response |
//! | [`HealthResponse`] | Health check response |
//!
//! # Usage Examples
//!
//! ## Basic Usage
//!
//! Create a pool with manual configuration:
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Build configuration
//!     let config = BrowserPoolConfigBuilder::new()
//!         .max_pool_size(5)
//!         .warmup_count(3)
//!         .browser_ttl(Duration::from_secs(3600))
//!         .build()?;
//!
//!     // Create pool
//!     let pool = BrowserPool::builder()
//!         .config(config)
//!         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!         .build()?;
//!
//!     // Warmup for production readiness
//!     pool.warmup().await?;
//!
//!     // Use a browser (automatically returned to pool when dropped)
//!     {
//!         let browser = pool.get()?;
//!         let tab = browser.new_tab()?;
//!         tab.navigate_to("https://example.com")?;
//!         tab.wait_until_navigated()?;
//!         let pdf = tab.print_to_pdf(None)?;
//!         std::fs::write("output.pdf", pdf)?;
//!     }
//!
//!     // Graceful shutdown
//!     pool.shutdown();
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Using Environment Configuration
//!
//! With the `env-config` feature, initialization is simpler:
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Reads BROWSER_POOL_SIZE, BROWSER_TTL_SECONDS, etc.
//!     let pool = init_browser_pool().await?;
//!
//!     // Pool is Arc<Mutex<BrowserPool>>, ready for sharing
//!     let guard = pool.lock().unwrap();
//!     let browser = guard.get()?;
//!     // ...
//!
//!     Ok(())
//! }
//! ```
//!
//! ## With Actix-web (Pre-built Routes)
//!
//! The fastest way to set up a PDF API:
//!
//! ```rust,ignore
//! use actix_web::{App, HttpServer, web};
//! use html2pdf_api::prelude::*;
//! use html2pdf_api::integrations::actix::configure_routes;
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize pool");
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .app_data(web::Data::new(pool.clone()))
//!             .configure(configure_routes) // Adds /pdf, /pdf/html, /health, etc.
//!     })
//!     .bind("127.0.0.1:8080")?
//!     .run()
//!     .await
//! }
//! ```
//!
//! ## With Actix-web (Custom Handler)
//!
//! For more control, use service functions directly:
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
//! ## Checking Pool Statistics
//!
//! Monitor pool health in your application:
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//!
//! fn log_pool_status(pool: &SharedBrowserPool) {
//!     if let Ok(guard) = pool.lock() {
//!         let stats = guard.stats();
//!         println!("Pool Status:");
//!         println!("  Available: {}", stats.available);
//!         println!("  Active: {}", stats.active);
//!         println!("  Total: {}", stats.total);
//!         
//!         // Check capacity
//!         if stats.available == 0 {
//!             println!("  ⚠️ Warning: No idle browsers available");
//!         }
//!     }
//! }
//! ```
//!
//! ## Error Handling
//!
//! Handle pool errors appropriately:
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//!
//! fn generate_pdf(pool: &SharedBrowserPool, url: &str) -> Result<Vec<u8>> {
//!     let guard = pool.lock()
//!         .map_err(|_| BrowserPoolError::PoolLock)?;
//!
//!     let browser = guard.get()?;  // Returns BrowserPoolError
//!     let tab = browser.new_tab()
//!         .map_err(|e| BrowserPoolError::BrowserCreation(e.to_string()))?;
//!
//!     tab.navigate_to(url)
//!         .map_err(|e| BrowserPoolError::BrowserCreation(e.to_string()))?;
//!     tab.wait_until_navigated()
//!         .map_err(|e| BrowserPoolError::BrowserCreation(e.to_string()))?;
//!
//!     let pdf = tab.print_to_pdf(None)
//!         .map_err(|e| BrowserPoolError::BrowserCreation(e.to_string()))?;
//!
//!     Ok(pdf)
//! }
//! ```
//!
//! # Feature Flag Reference
//!
//! | Feature | Adds to Prelude |
//! |---------|-----------------|
//! | (none) | Core types only |
//! | `env-config` | `init_browser_pool`, `from_env`, `chrome_path_from_env` |
//! | `actix-integration` | `actix` module + service types |
//! | `rocket-integration` | `rocket` module + service types |
//! | `axum-integration` | `axum` module + service types |
//! | `test-utils` | `MockBrowserFactory` (in `factory::mock`) |
//!
//! # See Also
//!
//! - [`crate::pool`] - Browser pool implementation details
//! - [`crate::config`] - Configuration options
//! - [`crate::service`] - Core PDF generation service
//! - [`crate::integrations`] - Web framework integrations

// ============================================================================
// Core Types (Always Available)
// ============================================================================

/// The main browser pool type for managing browser instances.
///
/// See [`crate::pool::BrowserPool`] for full documentation.
pub use crate::pool::BrowserPool;

/// Builder for creating configured [`BrowserPool`] instances.
///
/// See [`crate::pool::BrowserPoolBuilder`] for full documentation.
pub use crate::pool::BrowserPoolBuilder;

/// Configuration settings for the browser pool.
///
/// See [`crate::config::BrowserPoolConfig`] for full documentation.
pub use crate::config::BrowserPoolConfig;

/// Builder for creating [`BrowserPoolConfig`] instances.
///
/// See [`crate::config::BrowserPoolConfigBuilder`] for full documentation.
pub use crate::config::BrowserPoolConfigBuilder;

/// Error type for browser pool operations.
///
/// See [`crate::error::BrowserPoolError`] for full documentation.
pub use crate::error::BrowserPoolError;

/// Result type alias using [`BrowserPoolError`].
///
/// Equivalent to `std::result::Result<T, BrowserPoolError>`.
pub use crate::error::Result;

/// RAII handle for a browser checked out from the pool.
///
/// When dropped, the browser is automatically returned to the pool.
/// See [`crate::handle::BrowserHandle`] for full documentation.
pub use crate::handle::BrowserHandle;

/// Real-time statistics about the browser pool.
///
/// See [`crate::stats::PoolStats`] for full documentation.
pub use crate::stats::PoolStats;

/// Trait for browser creation strategies.
///
/// Implement this trait to customize how browsers are created.
/// See [`crate::factory::BrowserFactory`] for full documentation.
pub use crate::factory::BrowserFactory;

/// Default factory for creating Chrome/Chromium browsers.
///
/// See [`crate::factory::ChromeBrowserFactory`] for full documentation.
pub use crate::factory::ChromeBrowserFactory;

/// Trait for browser health checking.
///
/// See [`crate::traits::Healthcheck`] for full documentation.
pub use crate::traits::Healthcheck;

/// Type alias for a shared, thread-safe browser pool.
///
/// This is defined as `Arc<Mutex<BrowserPool>>` and is the standard
/// way to share a pool across threads and async tasks.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::prelude::*;
///
/// let pool: SharedBrowserPool = Arc::new(Mutex::new(
///     BrowserPool::builder()
///         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
///         .build()
///         .unwrap()
/// ));
///
/// // Or use the convenience method:
/// let pool: SharedBrowserPool = BrowserPool::builder()
///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
///     .build()
///     .unwrap()
///     .into_shared();
/// ```
pub use crate::SharedBrowserPool;

// ============================================================================
// Standard Library Re-exports
// ============================================================================

/// Thread-safe reference counting pointer.
///
/// Re-exported for convenience when working with [`SharedBrowserPool`].
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::prelude::*;
///
/// let pool = init_browser_pool().await?;
/// let pool_clone = Arc::clone(&pool);  // Clone for another thread/task
/// ```
pub use std::sync::Arc;

/// Mutual exclusion primitive.
///
/// Re-exported for convenience when working with [`SharedBrowserPool`].
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::prelude::*;
///
/// let pool = init_browser_pool().await?;
/// let guard = pool.lock().unwrap();  // Acquire lock
/// let browser = guard.get()?;
/// ```
pub use std::sync::Mutex;

// ============================================================================
// Environment Configuration (env-config feature)
// ============================================================================

/// Initialize a browser pool from environment variables.
///
/// This function reads configuration from environment variables and creates
/// a ready-to-use shared pool. It's the recommended way to initialize the
/// pool in production.
///
/// # Environment Variables
///
/// | Variable | Type | Default | Description |
/// |----------|------|---------|-------------|
/// | `BROWSER_POOL_SIZE` | usize | 5 | Maximum browsers in pool |
/// | `BROWSER_WARMUP_COUNT` | usize | 3 | Browsers to pre-create |
/// | `BROWSER_TTL_SECONDS` | u64 | 3600 | Browser lifetime |
/// | `BROWSER_WARMUP_TIMEOUT_SECONDS` | u64 | 60 | Warmup timeout |
/// | `BROWSER_PING_INTERVAL_SECONDS` | u64 | 15 | Health check interval |
/// | `BROWSER_MAX_PING_FAILURES` | u32 | 3 | Max failures before removal |
/// | `CHROME_PATH` | String | auto | Custom Chrome path |
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::prelude::*;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let pool = init_browser_pool().await?;
///     // Pool is ready to use!
///     Ok(())
/// }
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - Environment variables contain invalid values
/// - Chrome/Chromium cannot be found or launched
/// - Pool warmup fails
#[cfg(feature = "env-config")]
pub use crate::pool::init_browser_pool;

/// Load pool configuration from environment variables.
///
/// Use this when you need to customize pool creation but still want
/// environment-based configuration.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::prelude::*;
///
/// let config = from_env()?;
/// let pool = BrowserPool::builder()
///     .config(config)
///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
///     .enable_keep_alive(true)  // Custom option
///     .build()?;
/// ```
#[cfg(feature = "env-config")]
pub use crate::config::env::from_env;

/// Get Chrome path from the `CHROME_PATH` environment variable.
///
/// Returns `Some(path)` if the variable is set, `None` otherwise.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::prelude::*;
///
/// let factory = match chrome_path_from_env() {
///     Some(path) => ChromeBrowserFactory::with_path(path),
///     None => ChromeBrowserFactory::with_defaults(),
/// };
/// ```
#[cfg(feature = "env-config")]
pub use crate::config::env::chrome_path_from_env;

// ============================================================================
// Service Types (Any Integration Feature)
// ============================================================================

/// Request parameters for URL-to-PDF conversion.
///
/// See [`crate::service::PdfFromUrlRequest`] for full documentation.
#[cfg(any(
    feature = "actix-integration",
    feature = "rocket-integration",
    feature = "axum-integration"
))]
pub use crate::service::PdfFromUrlRequest;

/// Request parameters for HTML-to-PDF conversion.
///
/// See [`crate::service::PdfFromHtmlRequest`] for full documentation.
#[cfg(any(
    feature = "actix-integration",
    feature = "rocket-integration",
    feature = "axum-integration"
))]
pub use crate::service::PdfFromHtmlRequest;

/// Successful PDF generation result.
///
/// Contains the PDF binary data and metadata.
/// See [`crate::service::PdfResponse`] for full documentation.
#[cfg(any(
    feature = "actix-integration",
    feature = "rocket-integration",
    feature = "axum-integration"
))]
pub use crate::service::PdfResponse;

/// Errors that can occur during PDF generation.
///
/// Includes HTTP status code mapping for easy response building.
/// See [`crate::service::PdfServiceError`] for full documentation.
#[cfg(any(
    feature = "actix-integration",
    feature = "rocket-integration",
    feature = "axum-integration"
))]
pub use crate::service::PdfServiceError;

/// JSON error response format.
///
/// See [`crate::service::ErrorResponse`] for full documentation.
#[cfg(any(
    feature = "actix-integration",
    feature = "rocket-integration",
    feature = "axum-integration"
))]
pub use crate::service::ErrorResponse;

/// Pool statistics response for API endpoints.
///
/// See [`crate::service::PoolStatsResponse`] for full documentation.
#[cfg(any(
    feature = "actix-integration",
    feature = "rocket-integration",
    feature = "axum-integration"
))]
pub use crate::service::PoolStatsResponse;

/// Health check response for API endpoints.
///
/// See [`crate::service::HealthResponse`] for full documentation.
#[cfg(any(
    feature = "actix-integration",
    feature = "rocket-integration",
    feature = "axum-integration"
))]
pub use crate::service::HealthResponse;

// ============================================================================
// Framework-Specific Modules
// ============================================================================

/// Actix-web integration module.
///
/// Provides pre-built handlers and helpers for Actix-web 4.x.
///
/// # Quick Start
///
/// ```rust,ignore
/// use actix_web::{App, HttpServer, web};
/// use html2pdf_api::prelude::*;
/// use html2pdf_api::integrations::actix::configure_routes;
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()> {
///     let pool = init_browser_pool().await.unwrap();
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
/// # Available Exports
///
/// | Export | Description |
/// |--------|-------------|
/// | `configure_routes` | Configure all pre-built routes |
/// | `pdf_from_url` | Handler for URL-to-PDF |
/// | `pdf_from_html` | Handler for HTML-to-PDF |
/// | `pool_stats` | Handler for pool statistics |
/// | `health_check` | Handler for health check |
/// | `readiness_check` | Handler for readiness check |
/// | `SharedPool` | Type alias for `Arc<Mutex<BrowserPool>>` |
/// | `BrowserPoolData` | Type alias for `web::Data<SharedBrowserPool>` |
/// | `BrowserPoolActixExt` | Extension trait for `BrowserPool` |
///
/// See [`crate::integrations::actix`] for full documentation.
#[cfg(feature = "actix-integration")]
pub mod actix {
    pub use crate::integrations::actix::*;
}

/// Rocket integration module.
///
/// Provides pre-built handlers and helpers for Rocket 0.5.x.
///
/// # Quick Start
///
/// ```rust,ignore
/// use html2pdf_api::prelude::*;
/// use html2pdf_api::integrations::rocket::routes;
///
/// #[rocket::launch]
/// async fn launch() -> _ {
///     let pool = init_browser_pool().await.unwrap();
///
///     rocket::build()
///         .manage(pool)
///         .mount("/", routes())
/// }
/// ```
///
/// # Available Exports
///
/// | Export | Description |
/// |--------|-------------|
/// | `routes` | Get all pre-built routes |
/// | `pdf_from_url` | Handler for URL-to-PDF |
/// | `pdf_from_html` | Handler for HTML-to-PDF |
/// | `pool_stats` | Handler for pool statistics |
/// | `health_check` | Handler for health check |
/// | `readiness_check` | Handler for readiness check |
/// | `SharedPool` | Type alias for `Arc<Mutex<BrowserPool>>` |
///
/// See [`crate::integrations::rocket`] for full documentation.
#[cfg(feature = "rocket-integration")]
pub mod rocket {
    pub use crate::integrations::rocket::*;
}

/// Axum integration module.
///
/// Provides pre-built handlers and router for Axum 0.7.x/0.8.x.
///
/// # Quick Start
///
/// ```rust,ignore
/// use html2pdf_api::prelude::*;
/// use html2pdf_api::integrations::axum::router;
///
/// #[tokio::main]
/// async fn main() {
///     let pool = init_browser_pool().await.unwrap();
///
///     let app = router().with_state(pool);
///
///     let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
///         .await
///         .unwrap();
///     axum::serve(listener, app).await.unwrap();
/// }
/// ```
///
/// # Available Exports
///
/// | Export | Description |
/// |--------|-------------|
/// | `router` | Create router with all pre-built routes |
/// | `pdf_from_url` | Handler for URL-to-PDF |
/// | `pdf_from_html` | Handler for HTML-to-PDF |
/// | `pool_stats` | Handler for pool statistics |
/// | `health_check` | Handler for health check |
/// | `readiness_check` | Handler for readiness check |
/// | `SharedPool` | Type alias for `Arc<Mutex<BrowserPool>>` |
///
/// See [`crate::integrations::axum`] for full documentation.
#[cfg(feature = "axum-integration")]
pub mod axum {
    pub use crate::integrations::axum::*;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify all core types are accessible.
    #[test]
    fn test_core_types_exported() {
        // These should all compile, proving the types are exported
        fn _accepts_config(_: BrowserPoolConfig) {}
        fn _accepts_error(_: BrowserPoolError) {}
        fn _accepts_stats(_: PoolStats) {}
        fn _returns_result() -> Result<()> {
            Ok(())
        }
    }

    /// Verify Arc and Mutex are re-exported.
    #[test]
    fn test_std_reexports() {
        let _: Arc<i32> = Arc::new(42);
        let _: Mutex<i32> = Mutex::new(42);
    }

    /// Verify SharedBrowserPool type alias works.
    #[test]
    fn test_shared_browser_pool_type() {
        fn _accepts_shared_pool(_: SharedBrowserPool) {}

        // Verify it's Arc<Mutex<BrowserPool>>
        fn _verify_type() {
            let pool = BrowserPool::builder()
                .factory(Box::new(crate::factory::mock::MockBrowserFactory::new()))
                .build()
                .unwrap();

            let shared: SharedBrowserPool = Arc::new(Mutex::new(pool));
            _accepts_shared_pool(shared);
        }
    }

    /// Verify env-config exports when feature is enabled.
    #[cfg(feature = "env-config")]
    #[test]
    fn test_env_config_exports() {
        // These should compile when env-config is enabled
        let _: Option<String> = chrome_path_from_env();
        fn _takes_from_env(_: fn() -> crate::error::Result<BrowserPoolConfig>) {}
        _takes_from_env(from_env);
    }

    /// Verify service types when any integration is enabled.
    #[cfg(any(
        feature = "actix-integration",
        feature = "rocket-integration",
        feature = "axum-integration"
    ))]
    #[test]
    fn test_service_types_exported() {
        let _: PdfFromUrlRequest = PdfFromUrlRequest::default();
        let _: PdfFromHtmlRequest = PdfFromHtmlRequest::default();
        let _: HealthResponse = HealthResponse::default();
    }
}