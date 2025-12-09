//! Rocket framework integration.
//!
//! This module provides helpers for using `BrowserPool` with Rocket.
//!
//! # Setup
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! html2pdf-api = { version = "0.1", features = ["rocket-integration"] }
//! rocket = "0.5"
//! ```
//!
//! # Basic Usage
//!
//! ```rust,ignore
//! use rocket::{get, launch, routes, State};
//! use rocket::http::Status;
//! use html2pdf_api::prelude::*;
//! use std::sync::Arc;
//!
//! #[get("/pdf")]
//! async fn generate_pdf(
//!     pool: &State<SharedBrowserPool>,
//! ) -> Result<Vec<u8>, Status> {
//!     let pool_guard = pool.lock().map_err(|_| Status::InternalServerError)?;
//!     let browser = pool_guard.get().map_err(|_| Status::InternalServerError)?;
//!
//!     let tab = browser.new_tab().map_err(|_| Status::InternalServerError)?;
//!     tab.navigate_to("https://example.com").map_err(|_| Status::InternalServerError)?;
//!
//!     // Generate PDF...
//!     let pdf_data = tab.print_to_pdf(None).map_err(|_| Status::InternalServerError)?;
//!
//!     Ok(pdf_data)
//! }
//!
//! #[launch]
//! async fn rocket() -> _ {
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
//!     rocket::build()
//!         .manage(shared_pool)
//!         .mount("/", routes![generate_pdf])
//! }
//! ```
//!
//! # Using with `init_browser_pool`
//!
//! If you have the `env-config` feature enabled:
//!
//! ```rust,ignore
//! use rocket::{launch, routes};
//! use html2pdf_api::init_browser_pool;
//!
//! #[launch]
//! async fn rocket() -> _ {
//!     let pool = init_browser_pool().await
//!         .expect("Failed to initialize browser pool");
//!
//!     rocket::build()
//!         .manage(pool)
//!         .mount("/", routes![generate_pdf])
//! }
//! ```
//!
//! # Using Fairings for Lifecycle Management
//!
//! For proper startup and shutdown handling, use a custom fairing:
//!
//! ```rust,ignore
//! use rocket::{Rocket, Build, fairing::{self, Fairing, Info, Kind}};
//! use html2pdf_api::prelude::*;
//!
//! pub struct BrowserPoolFairing;
//!
//! #[rocket::async_trait]
//! impl Fairing for BrowserPoolFairing {
//!     fn info(&self) -> Info {
//!         Info {
//!             name: "Browser Pool",
//!             kind: Kind::Ignite | Kind::Shutdown,
//!         }
//!     }
//!
//!     async fn on_ignite(&self, rocket: Rocket<Build>) -> fairing::Result {
//!         let pool = BrowserPool::builder()
//!             .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!             .build()
//!             .expect("Failed to create pool");
//!
//!         pool.warmup().await.expect("Failed to warmup");
//!
//!         Ok(rocket.manage(pool.into_shared()))
//!     }
//!
//!     async fn on_shutdown(&self, rocket: &Rocket<rocket::Orbit>) {
//!         if let Some(pool) = rocket.state::<SharedBrowserPool>() {
//!             if let Ok(mut pool) = pool.lock() {
//!                 pool.shutdown_async().await;
//!             }
//!         }
//!     }
//! }
//!
//! #[launch]
//! fn rocket() -> _ {
//!     rocket::build()
//!         .attach(BrowserPoolFairing)
//!         .mount("/", routes![generate_pdf])
//! }
//! ```
//!
//! # Response Types
//!
//! For PDF responses, you can create a custom responder:
//!
//! ```rust,ignore
//! use rocket::response::{self, Response, Responder};
//! use rocket::http::{ContentType, Status};
//! use rocket::Request;
//! use std::io::Cursor;
//!
//! pub struct PdfResponse(pub Vec<u8>);
//!
//! impl<'r> Responder<'r, 'static> for PdfResponse {
//!     fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
//!         Response::build()
//!             .header(ContentType::PDF)
//!             .sized_body(self.0.len(), Cursor::new(self.0))
//!             .ok()
//!     }
//! }
//! ```

use rocket::State;

use crate::SharedBrowserPool;
use crate::pool::BrowserPool;

/// Type alias for Rocket `State` wrapper around the shared pool.
///
/// Use this type in your handler parameters:
///
/// ```rust,ignore
/// #[get("/pdf")]
/// async fn handler(pool: BrowserPoolState<'_>) -> Result<Vec<u8>, Status> {
///     let pool = pool.lock().unwrap();
///     let browser = pool.get()?;
///     // ...
/// }
/// ```
pub type BrowserPoolState<'r> = &'r State<SharedBrowserPool>;

/// Extension trait for `BrowserPool` with Rocket helpers.
///
/// Provides convenient methods for integrating with Rocket.
pub trait BrowserPoolRocketExt {
    /// Convert the pool into a form suitable for Rocket's `manage()`.
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
    /// let managed_pool = pool.into_rocket_state();
    ///
    /// rocket::build().manage(managed_pool)
    /// ```
    fn into_rocket_state(self) -> SharedBrowserPool;
}

impl BrowserPoolRocketExt for BrowserPool {
    fn into_rocket_state(self) -> SharedBrowserPool {
        self.into_shared()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_trait_exists() {
        // This test just verifies the trait is properly defined
        fn _accepts_shared_pool(_: SharedBrowserPool) {}
    }
}
