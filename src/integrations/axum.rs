//! Axum framework integration.
//!
//! This module provides helpers for using `BrowserPool` with Axum.
//!
//! # Setup
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! html2pdf-api = { version = "0.1", features = ["axum-integration"] }
//! axum = "0.8"
//! tower = "0.5"
//! ```
//!
//! # Basic Usage with State
//!
//! ```rust,ignore
//! use axum::{
//!     Router,
//!     routing::get,
//!     extract::State,
//!     response::IntoResponse,
//!     http::StatusCode,
//! };
//! use html2pdf_api::prelude::*;
//! use std::sync::Arc;
//!
//! async fn generate_pdf(
//!     State(pool): State<SharedBrowserPool>,
//! ) -> Result<impl IntoResponse, StatusCode> {
//!     let pool_guard = pool.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!     let browser = pool_guard.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!
//!     let tab = browser.new_tab().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!     tab.navigate_to("https://example.com").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!
//!     // Generate PDF...
//!     let pdf_data = tab.print_to_pdf(None).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!
//!     Ok((
//!         [(axum::http::header::CONTENT_TYPE, "application/pdf")],
//!         pdf_data,
//!     ))
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create and warmup pool
//!     let pool = BrowserPool::builder()
//!         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!         .build()
//!         .expect("Failed to create pool");
//!
//!     pool.warmup().await.expect("Failed to warmup");
//!
//!     // Convert to shared state
//!     let shared_pool = pool.into_shared();
//!
//!     let app = Router::new()
//!         .route("/pdf", get(generate_pdf))
//!         .with_state(shared_pool);
//!
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
//! ```
//!
//! # Using Extension Layer
//!
//! Alternatively, use the Extension layer pattern:
//!
//! ```rust,ignore
//! use axum::{
//!     Router,
//!     routing::get,
//!     Extension,
//!     response::IntoResponse,
//! };
//! use html2pdf_api::prelude::*;
//! use std::sync::Arc;
//!
//! async fn generate_pdf(
//!     Extension(pool): Extension<SharedBrowserPool>,
//! ) -> impl IntoResponse {
//!     let pool_guard = pool.lock().unwrap();
//!     let browser = pool_guard.get().unwrap();
//!     // ...
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let pool = BrowserPool::builder()
//!         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!         .build()
//!         .expect("Failed to create pool");
//!
//!     pool.warmup().await.expect("Failed to warmup");
//!
//!     let shared_pool = pool.into_shared();
//!
//!     let app = Router::new()
//!         .route("/pdf", get(generate_pdf))
//!         .layer(Extension(shared_pool));
//!
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
//! ```
//!
//! # Using with `init_browser_pool`
//!
//! If you have the `env-config` feature enabled:
//!
//! ```rust,ignore
//! use axum::{Router, routing::get};
//! use html2pdf_api::init_browser_pool;
//!
//! #[tokio::main]
//! async fn main() {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     let app = Router::new()
//!         .route("/pdf", get(generate_pdf))
//!         .with_state(pool);
//!
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
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
//!     let pool = BrowserPool::builder()
//!         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!         .build()
//!         .expect("Failed to create pool");
//!
//!     pool.warmup().await.expect("Failed to warmup");
//!
//!     let shared_pool = Arc::new(std::sync::Mutex::new(pool));
//!     let shutdown_pool = Arc::clone(&shared_pool);
//!
//!     let app = Router::new()
//!         .with_state(shared_pool);
//!
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
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
//!         pool.shutdown_async().await;
//!     }
//! }
//! ```
//!
//! # Custom Extractor
//!
//! For cleaner handler signatures, create a custom extractor:
//!
//! ```rust,ignore
//! use axum::{
//!     async_trait,
//!     extract::{FromRequestParts, State},
//!     http::{request::Parts, StatusCode},
//! };
//! use html2pdf_api::prelude::*;
//!
//! pub struct Browser(pub BrowserHandle);
//!
//! #[async_trait]
//! impl FromRequestParts<SharedBrowserPool> for Browser {
//!     type Rejection = StatusCode;
//!
//!     async fn from_request_parts(
//!         _parts: &mut Parts,
//!         state: &SharedBrowserPool,
//!     ) -> Result<Self, Self::Rejection> {
//!         let pool = state.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//!         let browser = pool.get().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
//!         Ok(Browser(browser))
//!     }
//! }
//!
//! // Then use in handlers:
//! async fn generate_pdf(Browser(browser): Browser) -> impl IntoResponse {
//!     let tab = browser.new_tab().unwrap();
//!     // ...
//! }
//! ```

use axum::extract::State;

use crate::SharedBrowserPool;
use crate::pool::BrowserPool;

/// Type alias for Axum `State` extractor with the shared pool.
///
/// Use this type in your handler parameters:
///
/// ```rust,ignore
/// async fn handler(
///     BrowserPoolState(pool): BrowserPoolState,
/// ) -> impl IntoResponse {
///     let pool = pool.lock().unwrap();
///     let browser = pool.get()?;
///     // ...
/// }
/// ```
pub type BrowserPoolState = State<SharedBrowserPool>;

/// Extension trait for `BrowserPool` with Axum helpers.
///
/// Provides convenient methods for integrating with Axum.
pub trait BrowserPoolAxumExt {
    /// Convert the pool into a form suitable for Axum's `with_state()`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::integrations::axum::BrowserPoolAxumExt;
    ///
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    ///
    /// let state = pool.into_axum_state();
    ///
    /// Router::new()
    ///     .route("/pdf", get(generate_pdf))
    ///     .with_state(state)
    /// ```
    fn into_axum_state(self) -> SharedBrowserPool;

    /// Convert the pool into an Extension layer.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use axum::Extension;
    /// use html2pdf_api::integrations::axum::BrowserPoolAxumExt;
    ///
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    ///
    /// let extension = pool.into_axum_extension();
    ///
    /// Router::new()
    ///     .route("/pdf", get(generate_pdf))
    ///     .layer(extension)
    /// ```
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
///
/// Use this when you already have a `SharedBrowserPool` and want to
/// create an Extension layer.
///
/// # Parameters
///
/// * `pool` - The shared browser pool.
///
/// # Returns
///
/// `Extension<SharedBrowserPool>` ready for use with `Router::layer()`.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::integrations::axum::create_extension;
///
/// let shared_pool = pool.into_shared();
/// let extension = create_extension(shared_pool);
///
/// Router::new().layer(extension)
/// ```
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
