//! Actix-web framework integration.
//!
//! This module provides helpers for using `BrowserPool` with Actix-web.
//!
//! # Setup
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! html2pdf-api = { version = "0.1", features = ["actix-integration"] }
//! actix-web = "4"
//! ```
//!
//! # Basic Usage
//!
//! ```rust,ignore
//! use actix_web::{web, App, HttpServer, HttpResponse, Responder};
//! use html2pdf_api::prelude::*;
//! use std::sync::Arc;
//!
//! async fn generate_pdf(
//!     pool: web::Data<SharedBrowserPool>,
//! ) -> impl Responder {
//!     let pool_guard = pool.lock().unwrap();
//!     let browser = match pool_guard.get() {
//!         Ok(b) => b,
//!         Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
//!     };
//!
//!     let tab = browser.new_tab().unwrap();
//!     tab.navigate_to("https://example.com").unwrap();
//!     
//!     // Generate PDF...
//!     let pdf_data = tab.print_to_pdf(None).unwrap();
//!
//!     HttpResponse::Ok()
//!         .content_type("application/pdf")
//!         .body(pdf_data)
//! }
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
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
//!     HttpServer::new(move || {
//!         App::new()
//!             .app_data(web::Data::new(Arc::clone(&shared_pool)))
//!             .route("/pdf", web::get().to(generate_pdf))
//!     })
//!     .bind("127.0.0.1:8080")?
//!     .run()
//!     .await
//! }
//! ```
//!
//! # Using with `init_browser_pool`
//!
//! If you have the `env-config` feature enabled:
//!
//! ```rust,ignore
//! use actix_web::{web, App, HttpServer};
//! use html2pdf_api::init_browser_pool;
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .app_data(web::Data::new(Arc::clone(&pool)))
//!             .configure(configure_routes)
//!     })
//!     .bind("127.0.0.1:8080")?
//!     .run()
//!     .await
//! }
//! ```
//!
//! # Graceful Shutdown
//!
//! For proper cleanup, shutdown the pool when the server stops:
//!
//! ```rust,ignore
//! use actix_web::{web, App, HttpServer};
//! use html2pdf_api::prelude::*;
//! use std::sync::Arc;
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
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
//!     let server = HttpServer::new(move || {
//!         App::new()
//!             .app_data(web::Data::new(Arc::clone(&shared_pool)))
//!     })
//!     .bind("127.0.0.1:8080")?
//!     .run();
//!
//!     let server_handle = server.handle();
//!
//!     // Run server
//!     let result = server.await;
//!
//!     // Cleanup pool after server stops
//!     if let Ok(mut pool) = shutdown_pool.lock() {
//!         pool.shutdown_async().await;
//!     }
//!
//!     result
//! }
//! ```

use actix_web::web;
use std::sync::Arc;

use crate::pool::BrowserPool;
use crate::SharedBrowserPool;

/// Type alias for Actix-web `Data` wrapper around the shared pool.
///
/// Use this type in your handler parameters:
///
/// ```rust,ignore
/// async fn handler(pool: BrowserPoolData) -> impl Responder {
///     let pool = pool.lock().unwrap();
///     let browser = pool.get()?;
///     // ...
/// }
/// ```
pub type BrowserPoolData = web::Data<SharedBrowserPool>;

/// Extension trait for `BrowserPool` with Actix-web helpers.
///
/// Provides convenient methods for integrating with Actix-web.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_alias_compiles() {
        // This test just verifies the type alias is valid
        fn _accepts_pool_data(_: BrowserPoolData) {}
    }
}