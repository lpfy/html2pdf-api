//! Web framework integrations.
//!
//! This module provides optional integrations with popular Rust web frameworks,
//! making it easier to use `BrowserPool` in your web applications.
//!
//! # Available Integrations
//!
//! | Framework | Feature Flag | Module |
//! |-----------|--------------|--------|
//! | Actix-web | `actix-integration` | `actix` |
//! | Rocket | `rocket-integration` | `rocket` |
//! | Axum | `axum-integration` | `axum` |
//!
//! # Enabling Integrations
//!
//! Add the desired feature to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! html2pdf-api = { version = "0.1", features = ["actix-integration"] }
//! ```
//!
//! # Common Pattern
//!
//! All integrations follow a similar pattern:
//!
//! 1. Create a `BrowserPool` during application startup
//! 2. Convert to shared state using `into_shared()`
//! 3. Register with your framework's state management
//! 4. Extract the pool in handlers and use `pool.get()`
//!
//! # Example (Generic Pattern)
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 1. Create pool
//!     let pool = BrowserPool::builder()
//!         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!         .build()?;
//!
//!     // 2. Warmup
//!     pool.warmup().await?;
//!
//!     // 3. Convert to shared state
//!     let shared_pool = pool.into_shared();
//!
//!     // 4. Pass to your web framework...
//!
//!     Ok(())
//! }
//! ```

#[cfg(feature = "actix-integration")]
pub mod actix;

#[cfg(feature = "rocket-integration")]
pub mod rocket;

#[cfg(feature = "axum-integration")]
pub mod axum;