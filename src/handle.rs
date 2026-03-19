//! RAII handle for browser instances.
//!
//! This module provides [`BrowserHandle`], which wraps a browser instance
//! and automatically returns it to the pool when dropped.
//!
//! # Overview
//!
//! The handle implements the RAII (Resource Acquisition Is Initialization)
//! pattern to ensure browsers are always returned to the pool, even if:
//! - Your code returns early
//! - An error occurs
//! - A panic happens
//!
//! # Usage Pattern
//!
//! ```rust,ignore
//! use html2pdf_api::BrowserPool;
//!
//! let pool = BrowserPool::builder()
//!     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!     .build()?;
//!
//! // Get a browser handle
//! let browser = pool.get()?;
//!
//! // Use it like a regular Browser (via Deref)
//! let tab = browser.new_tab()?;
//! tab.navigate_to("https://example.com")?;
//!
//! // Browser automatically returned when `browser` goes out of scope
//! ```
//!
//! # Deref Behavior
//!
//! `BrowserHandle` implements [`Deref<Target = Browser>`](std::ops::Deref),
//! allowing transparent access to all [`Browser`] methods:
//!
//! ```rust,ignore
//! let browser = pool.get()?;
//!
//! // These all work directly on the handle:
//! let tab = browser.new_tab()?;           // Browser::new_tab
//! let tabs = browser.get_tabs();          // Browser::get_tabs
//! let version = browser.get_version()?;   // Browser::get_version
//! ```

use std::sync::Arc;

use headless_chrome::Browser;

use crate::pool::BrowserPoolInner;
use crate::tracked::TrackedBrowser;

/// RAII handle for browser instances.
///
/// Automatically returns the browser to the pool when dropped.
/// This ensures browsers are always returned even if the code panics.
///
/// # Thread Safety
///
/// `BrowserHandle` is `Send` but not `Sync`. This means:
/// - ✅ You can move it to another thread
/// - ❌ You cannot share it between threads simultaneously
///
/// This matches the typical usage pattern where a single request/task
/// uses a browser exclusively.
///
/// # Usage
///
/// ```rust,ignore
/// let browser_handle = pool.get()?;
///
/// // Use browser via Deref
/// let tab = browser_handle.new_tab()?;
/// // ... do work ...
///
/// // Browser automatically returned to pool when handle goes out of scope
/// ```
///
/// # Explicit Drop
///
/// If you need to return the browser early (before end of scope),
/// you can explicitly drop the handle:
///
/// ```rust,ignore
/// let browser = pool.get()?;
/// let tab = browser.new_tab()?;
/// // ... do work ...
///
/// // Return browser early
/// drop(browser);
///
/// // Browser is now back in the pool and available for others
/// // Attempting to use `browser` here would be a compile error
/// ```
///
/// # Panic Safety
///
/// The RAII pattern ensures browsers are returned even during panics:
///
/// ```rust,ignore
/// let browser = pool.get()?;
///
/// // Even if this panics...
/// some_function_that_might_panic();
///
/// // ...the browser is still returned to the pool during unwinding
/// ```
pub struct BrowserHandle {
    /// The tracked browser (Option allows taking in Drop).
    ///
    /// This is `Option` so we can `take()` it in the `Drop` implementation
    /// without requiring `&mut self` to be valid after drop.
    tracked: Option<Arc<TrackedBrowser>>,

    /// Reference to pool for returning browser.
    ///
    /// We keep an `Arc` reference to the pool's inner state so we can
    /// return the browser even if the original `BrowserPool` has been dropped.
    pool: Arc<BrowserPoolInner>,
}

impl BrowserHandle {
    /// Create a new browser handle.
    ///
    /// This is called internally by [`BrowserPool::get()`](crate::BrowserPool::get).
    /// Users should not need to call this directly.
    ///
    /// # Parameters
    ///
    /// * `tracked` - The tracked browser instance.
    /// * `pool` - Arc reference to the pool's inner state.
    pub(crate) fn new(tracked: Arc<TrackedBrowser>, pool: Arc<BrowserPoolInner>) -> Self {
        Self {
            tracked: Some(tracked),
            pool,
        }
    }

    /// Get the browser's unique ID.
    ///
    /// Useful for logging and debugging.
    ///
    /// # Returns
    ///
    /// The unique ID assigned to this browser instance.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let browser = pool.get()?;
    /// log::info!("Using browser {}", browser.id());
    /// ```
    pub fn id(&self) -> u64 {
        self.tracked.as_ref().map(|t| t.id()).unwrap_or(0)
    }

    /// Get the browser's age (time since creation).
    ///
    /// Useful for monitoring and debugging.
    ///
    /// # Returns
    ///
    /// Duration since the browser was created.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let browser = pool.get()?;
    /// log::debug!("Browser age: {:?}", browser.age());
    /// ```
    pub fn age(&self) -> std::time::Duration {
        self.tracked.as_ref().map(|t| t.age()).unwrap_or_default()
    }

    /// Get the browser's age in minutes.
    ///
    /// Convenience method for human-readable logging.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let browser = pool.get()?;
    /// log::info!("Browser {} is {} minutes old", browser.id(), browser.age_minutes());
    /// ```
    pub fn age_minutes(&self) -> u64 {
        self.tracked.as_ref().map(|t| t.age_minutes()).unwrap_or(0)
    }
}

impl std::ops::Deref for BrowserHandle {
    type Target = Browser;

    /// Transparently access the underlying Browser.
    ///
    /// This allows using all [`Browser`] methods directly on the handle:
    ///
    /// ```rust,ignore
    /// let browser = pool.get()?;
    ///
    /// // new_tab() is a Browser method, but works on BrowserHandle
    /// let tab = browser.new_tab()?;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if called after the browser has been returned to the pool.
    /// This should never happen in normal usage since the handle owns
    /// the browser until it's dropped.
    fn deref(&self) -> &Self::Target {
        self.tracked.as_ref().unwrap().browser()
    }
}

impl Drop for BrowserHandle {
    /// Automatically return browser to pool when handle is dropped.
    ///
    /// This is the critical RAII pattern that ensures browsers are always
    /// returned to the pool, even if the code using them panics.
    ///
    /// # Implementation Details
    ///
    /// - Uses `Option::take()` to move the browser out of the handle
    /// - Calls `BrowserPoolInner::return_browser()` to return it
    /// - Safe to call multiple times (subsequent calls are no-ops)
    fn drop(&mut self) {
        if let Some(tracked) = self.tracked.take() {
            log::debug!(
                "♻️ BrowserHandle {} being dropped, returning to pool...",
                tracked.id()
            );

            // Return to pool using static method (avoids &mut self issues)
            BrowserPoolInner::return_browser(&self.pool, tracked);
        }
    }
}

impl std::fmt::Debug for BrowserHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.tracked {
            Some(tracked) => f
                .debug_struct("BrowserHandle")
                .field("id", &tracked.id())
                .field("age_minutes", &tracked.age_minutes())
                .finish(),
            None => f
                .debug_struct("BrowserHandle")
                .field("state", &"returned")
                .finish(),
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BrowserPoolConfig;
    use crate::factory::mock::MockBrowserFactory;
    use crate::pool::BrowserPoolInner;

    fn create_test_pool_inner() -> Arc<BrowserPoolInner> {
        Arc::new(BrowserPoolInner::new_for_test(
            BrowserPoolConfig::default(),
            Box::new(MockBrowserFactory::always_fails("test")),
            tokio::runtime::Handle::current(),
        ))
    }

    /// Verifies that BrowserHandle methods return graceful fallbacks when empty (post-drop).
    #[tokio::test]
    async fn test_handle_id_returns_zero_when_tracked_is_none() {
        let handle = BrowserHandle {
            tracked: None,
            pool: create_test_pool_inner(),
        };
        assert_eq!(handle.id(), 0);
        assert_eq!(handle.age_minutes(), 0);
        assert_eq!(handle.age(), std::time::Duration::default());
    }

    /// Verifies that Debug is formatted safely for a returned handle.
    #[tokio::test]
    async fn test_handle_debug_when_returned() {
        let handle = BrowserHandle {
            tracked: None,
            pool: create_test_pool_inner(),
        };
        let debug_output = format!("{:?}", handle);
        assert!(debug_output.contains("returned"));
        assert!(!debug_output.contains("age_minutes"));
    }

    /// Verifies Debug incorporates age if active, although requires real chrome dependencies to fully run locally.
    #[test]
    #[ignore = "Requires launching a real Chrome process for TrackedBrowser initialization"]
    fn test_handle_debug_when_active() {
        // Assertions verifying that `Debug` includes `age_minutes`
        // when `tracked: Some(...)` would live here.
    }
}
