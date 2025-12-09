//! Convenient imports for common usage patterns.
//!
//! This module re-exports the most commonly used types from `html2pdf-api`,
//! allowing you to quickly get started with a single import.
//!
//! # Usage
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//! ```
//!
//! This imports:
//!
//! - [`BrowserPool`] - Main pool type
//! - [`BrowserPoolBuilder`] - Pool builder
//! - [`BrowserPoolConfig`] - Configuration struct
//! - [`BrowserPoolConfigBuilder`] - Configuration builder
//! - [`BrowserPoolError`] - Error type
//! - [`Result`] - Result type alias
//! - [`BrowserHandle`] - RAII browser handle
//! - [`PoolStats`] - Pool statistics
//! - [`BrowserFactory`] - Factory trait
//! - [`ChromeBrowserFactory`] - Chrome factory
//! - [`Healthcheck`] - Health check trait
//! - [`SharedBrowserPool`] - Type alias for shared pool
//!
//! # Example
//!
//! ```rust,ignore
//! use html2pdf_api::prelude::*;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = BrowserPoolConfigBuilder::new()
//!         .max_pool_size(5)
//!         .warmup_count(3)
//!         .build()?;
//!
//!     let pool = BrowserPool::builder()
//!         .config(config)
//!         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!         .build()?;
//!
//!     pool.warmup().await?;
//!
//!     {
//!         let browser = pool.get()?;
//!         let tab = browser.new_tab()?;
//!         // ... use browser
//!     }
//!
//!     Ok(())
//! }
//! ```

// Core types
pub use crate::config::{BrowserPoolConfig, BrowserPoolConfigBuilder};
pub use crate::error::{BrowserPoolError, Result};
pub use crate::factory::{BrowserFactory, ChromeBrowserFactory};
pub use crate::handle::BrowserHandle;
pub use crate::pool::{BrowserPool, BrowserPoolBuilder};
pub use crate::stats::PoolStats;
pub use crate::traits::Healthcheck;
pub use crate::SharedBrowserPool;

// Feature-gated exports
#[cfg(feature = "env-config")]
pub use crate::config::env::{chrome_path_from_env, from_env};

#[cfg(feature = "env-config")]
pub use crate::pool::init_browser_pool;

// Re-export Arc and Mutex for convenience (commonly needed with SharedBrowserPool)
pub use std::sync::{Arc, Mutex};