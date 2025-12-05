//! Traits for abstraction and extensibility.
//!
//! This module provides traits that define the core abstractions used by
//! the browser pool. These traits enable:
//!
//! - **Health monitoring**: [`Healthcheck`] for verifying browser health
//! - **Extensibility**: Custom implementations for different use cases
//!
//! # Implementing Custom Health Checks
//!
//! While [`TrackedBrowser`](crate::TrackedBrowser) implements [`Healthcheck`]
//! by default, you can implement custom health check logic:
//!
//! ```rust,ignore
//! use html2pdf_api::{Healthcheck, Result, BrowserPoolError};
//!
//! struct MyCustomBrowser {
//!     // your fields
//! }
//!
//! impl Healthcheck for MyCustomBrowser {
//!     fn ping(&self) -> Result<()> {
//!         // Your custom health check logic
//!         Ok(())
//!     }
//! }
//! ```

mod healthcheck;

pub use healthcheck::Healthcheck;