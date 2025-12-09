//! Health check trait for browser instances.
//!
//! This module provides the [`Healthcheck`] trait, which defines how
//! browser instances verify they are still functional and responsive.
//!
//! # Overview
//!
//! The browser pool periodically pings active browsers to detect failures.
//! When a browser fails too many consecutive health checks, it is removed
//! from the pool and replaced.
//!
//! # Default Implementation
//!
//! [`TrackedBrowser`](crate::TrackedBrowser) implements this trait by:
//! 1. Creating a new tab
//! 2. Closing the tab
//! 3. Updating the last ping timestamp
//!
//! This is a lightweight operation that verifies the browser process
//! is alive and the CDP (Chrome DevTools Protocol) connection works.

use crate::error::Result;

/// Trait for browser-like objects that support health checking.
///
/// Implementors must provide a [`ping()`](Self::ping) method that verifies
/// the browser is still functional and responsive.
///
/// # Thread Safety
///
/// This trait requires `Send + Sync` because browsers may be health-checked
/// from a background thread while being used from another thread.
///
/// # Example Implementation
///
/// ```rust,ignore
/// use html2pdf_api::{Healthcheck, Result, BrowserPoolError};
///
/// struct MyBrowser {
///     inner: SomeBrowserType,
/// }
///
/// impl Healthcheck for MyBrowser {
///     fn ping(&self) -> Result<()> {
///         // Try to create a tab to verify browser is responsive
///         let tab = self.inner.new_tab()
///             .map_err(|e| BrowserPoolError::HealthCheckFailed(e.to_string()))?;
///         
///         // Clean up
///         let _ = tab.close();
///         
///         Ok(())
///     }
/// }
/// ```
///
/// # How It's Used
///
/// The browser pool's keep-alive thread calls `ping()` on all active browsers
/// at regular intervals (configured via
/// [`ping_interval`](crate::BrowserPoolConfig::ping_interval)).
///
/// ```text
/// Keep-Alive Thread
///       │
///       ├─── ping() ──→ Browser 1 ──→ ✓ OK
///       │
///       ├─── ping() ──→ Browser 2 ──→ ✗ Failed (count: 1)
///       │
///       └─── ping() ──→ Browser 3 ──→ ✓ OK
/// ```
///
/// After [`max_ping_failures`](crate::BrowserPoolConfig::max_ping_failures)
/// consecutive failures, the browser is removed and replaced.
pub trait Healthcheck: Send + Sync {
    /// Perform a health check on the browser.
    ///
    /// Should perform a lightweight operation like creating/closing a tab
    /// to verify the browser process is still responsive.
    ///
    /// # Implementation Guidelines
    ///
    /// - **Keep it fast**: Health checks run frequently; avoid heavy operations
    /// - **Don't hold locks**: Release any locks before performing I/O
    /// - **Be idempotent**: Multiple calls should be safe
    /// - **Clean up**: Close any tabs or resources created during the check
    ///
    /// # Errors
    ///
    /// Returns [`BrowserPoolError::HealthCheckFailed`](crate::BrowserPoolError::HealthCheckFailed)
    /// if the health check fails (browser unresponsive or crashed).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::{Healthcheck, Result};
    ///
    /// fn check_browser_health<T: Healthcheck>(browser: &T) -> Result<()> {
    ///     browser.ping()?;
    ///     println!("Browser is healthy!");
    ///     Ok(())
    /// }
    /// ```
    fn ping(&self) -> Result<()>;
}
