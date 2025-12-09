//! # html2pdf-api
//!
//! Thread-safe browser pool for headless Chrome automation with web framework integration.
//!
//! This crate provides a production-ready browser pool that manages headless Chrome/Chromium
//! instances with automatic lifecycle management, health monitoring, and graceful shutdown.
//!
//! ## Features
//!
//! - **Connection Pooling**: Reuses browser instances to avoid expensive startup costs
//! - **Health Monitoring**: Background thread continuously checks browser health
//! - **TTL Management**: Automatically retires old browsers and creates replacements
//! - **Race-Free Design**: Careful lock ordering prevents deadlocks
//! - **Graceful Shutdown**: Clean termination of all background tasks
//! - **RAII Pattern**: Automatic return of browsers to pool via Drop
//! - **Web Framework Integration**: Optional support for Actix-web, Rocket, and Axum
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │         Your Web Application                │
//! │      (Actix-web / Rocket / Axum)            │
//! └─────────────────┬───────────────────────────┘
//!                   │
//!                   ▼
//! ┌─────────────────────────────────────────────┐
//! │              BrowserPool                    │
//! │ ┌─────────────────────────────────────────┐ │
//! │ │   Available Pool (idle browsers)        │ │
//! │ │   [Browser1] [Browser2] [Browser3]      │ │
//! │ └─────────────────────────────────────────┘ │
//! │ ┌─────────────────────────────────────────┐ │
//! │ │   Active Tracking (in-use browsers)     │ │
//! │ │   {id → Browser}                        │ │
//! │ └─────────────────────────────────────────┘ │
//! │ ┌─────────────────────────────────────────┐ │
//! │ │   Keep-Alive Thread                     │ │
//! │ │   (health checks + TTL enforcement)     │ │
//! │ └─────────────────────────────────────────┘ │
//! └─────────────────────────────────────────────┘
//!                   │
//!                   ▼
//! ┌─────────────────────────────────────────────┐
//! │        Headless Chrome Browsers             │
//! │     (managed by headless_chrome crate)      │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create pool with custom configuration
//!     let pool = BrowserPool::builder()
//!         .config(
//!             BrowserPoolConfigBuilder::new()
//!                 .max_pool_size(5)
//!                 .warmup_count(3)
//!                 .browser_ttl(Duration::from_secs(3600))
//!                 .build()?
//!         )
//!         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!         .build()?;
//!
//!     // Warmup the pool (recommended for production)
//!     pool.warmup().await?;
//!
//!     // Use browsers
//!     {
//!         let browser = pool.get()?;
//!         let tab = browser.new_tab()?;
//!         tab.navigate_to("https://example.com")?;
//!         // ... generate PDF, take screenshot, etc.
//!     } // Browser automatically returned to pool
//!
//!     // Graceful shutdown
//!     pool.shutdown_async().await;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Environment Configuration
//!
//! When the `env-config` feature is enabled, you can initialize the pool
//! from environment variables (loaded from `app.env` file or system environment):
//!
//! ```rust,no_run
//! use html2pdf_api::init_browser_pool;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let pool = init_browser_pool().await?;
//!     // pool is Arc<Mutex<BrowserPool>>, ready for web handlers
//!     Ok(())
//! }
//! ```
//!
//! ### Environment File
//!
//! Create an `app.env` file in your project root (not `.env` for better
//! cross-platform visibility):
//!
//! ```text
//! BROWSER_POOL_SIZE=5
//! BROWSER_WARMUP_COUNT=3
//! BROWSER_TTL_SECONDS=3600
//! ```
//!
//! ### Environment Variables
//!
//! | Variable | Type | Default | Description |
//! |----------|------|---------|-------------|
//! | `BROWSER_POOL_SIZE` | usize | 5 | Maximum browsers in pool |
//! | `BROWSER_WARMUP_COUNT` | usize | 3 | Browsers to pre-create |
//! | `BROWSER_TTL_SECONDS` | u64 | 3600 | Browser lifetime (seconds) |
//! | `BROWSER_WARMUP_TIMEOUT_SECONDS` | u64 | 60 | Warmup timeout |
//! | `BROWSER_PING_INTERVAL_SECONDS` | u64 | 15 | Health check interval |
//! | `BROWSER_MAX_PING_FAILURES` | u32 | 3 | Failures before removal |
//! | `CHROME_PATH` | String | auto | Custom Chrome binary path |
//!
//! ## Feature Flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `env-config` | Enable environment-based configuration |
//! | `actix-integration` | Actix-web framework integration |
//! | `rocket-integration` | Rocket framework integration |
//! | `axum-integration` | Axum framework integration |
//! | `test-utils` | Enable mock factory for testing |
//!
//! ## Web Framework Integration
//!
//! ### Actix-web
//!
//! ```rust,ignore
//! use actix_web::{web, App, HttpServer};
//! use html2pdf_api::prelude::*;
//!
//! async fn generate_pdf(
//!     pool: web::Data<Arc<Mutex<BrowserPool>>>,
//! ) -> impl Responder {
//!     let pool = pool.lock().unwrap();
//!     let browser = pool.get()?;
//!     // ... use browser
//! }
//! ```
//!
//! ### Rocket
//!
//! ```rust,ignore
//! use rocket::State;
//! use html2pdf_api::prelude::*;
//!
//! #[get("/pdf")]
//! async fn generate_pdf(
//!     pool: &State<Arc<Mutex<BrowserPool>>>,
//! ) -> Result<Vec<u8>, Status> {
//!     let pool = pool.lock().unwrap();
//!     let browser = pool.get()?;
//!     // ... use browser
//! }
//! ```
//!
//! ### Axum
//!
//! ```rust,ignore
//! use axum::{Extension, response::IntoResponse};
//! use html2pdf_api::prelude::*;
//!
//! async fn generate_pdf(
//!     Extension(pool): Extension<Arc<Mutex<BrowserPool>>>,
//! ) -> impl IntoResponse {
//!     let pool = pool.lock().unwrap();
//!     let browser = pool.get()?;
//!     // ... use browser
//! }
//! ```
//!
//! ## Error Handling
//!
//! All fallible operations return [`Result<T, BrowserPoolError>`](Result).
//! The error type provides context about what went wrong:
//!
//! ```rust,ignore
//! use html2pdf_api::{BrowserPool, BrowserPoolError};
//!
//! match pool.get() {
//!     Ok(browser) => {
//!         // Use browser
//!     }
//!     Err(BrowserPoolError::ShuttingDown) => {
//!         // Pool is shutting down, handle gracefully
//!     }
//!     Err(BrowserPoolError::BrowserCreation(msg)) => {
//!         // Chrome failed to launch
//!         eprintln!("Browser creation failed: {}", msg);
//!     }
//!     Err(e) => {
//!         // Other errors
//!         eprintln!("Pool error: {}", e);
//!     }
//! }
//! ```
//!
//! ## Testing
//!
//! For testing without Chrome, enable the `test-utils` feature and use
//! [`MockBrowserFactory`](factory::mock::MockBrowserFactory):
//!
//! ```rust,ignore
//! use html2pdf_api::factory::mock::MockBrowserFactory;
//!
//! let factory = MockBrowserFactory::always_fails("Test error");
//! let pool = BrowserPool::builder()
//!     .factory(Box::new(factory))
//!     .enable_keep_alive(false)
//!     .build()?;
//! ```

#![doc(html_root_url = "https://docs.rs/html2pdf-api/0.1.0")]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

// ============================================================================
// Modules
// ============================================================================

pub mod config;
pub mod error;
pub mod factory;
pub mod handle;
pub mod pool;
pub mod prelude;
pub mod stats;
pub mod traits;

// Internal modules (not publicly exposed)
pub(crate) mod tracked;

// ============================================================================
// Feature-gated modules
// ============================================================================

/// Web framework integrations.
///
/// This module provides optional integrations with popular Rust web frameworks.
/// Enable the corresponding feature flag to use them:
///
/// - `actix-integration` for Actix-web
/// - `rocket-integration` for Rocket
/// - `axum-integration` for Axum
#[cfg(any(
    feature = "actix-integration",
    feature = "rocket-integration",
    feature = "axum-integration"
))]
pub mod integrations;

// ============================================================================
// Re-exports (Public API)
// ============================================================================

// Core types
pub use config::{BrowserPoolConfig, BrowserPoolConfigBuilder};
pub use error::{BrowserPoolError, Result};
pub use factory::{BrowserFactory, ChromeBrowserFactory, create_chrome_options};
pub use handle::BrowserHandle;
pub use pool::{BrowserPool, BrowserPoolBuilder};
pub use stats::PoolStats;
pub use traits::Healthcheck;

// Feature-gated re-exports
#[cfg(feature = "env-config")]
pub use config::env::{chrome_path_from_env, from_env};

#[cfg(feature = "env-config")]
pub use pool::init_browser_pool;

// ============================================================================
// Convenience type aliases
// ============================================================================

/// Shared browser pool type for web frameworks.
///
/// This is the recommended type for sharing a pool across web handlers.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::SharedBrowserPool;
///
/// let pool: SharedBrowserPool = browser_pool.into_shared();
/// ```
pub type SharedBrowserPool = std::sync::Arc<std::sync::Mutex<BrowserPool>>;
