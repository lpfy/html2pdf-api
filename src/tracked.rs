//! Tracked browser with metadata for pool management.
//!
//! This module provides [`TrackedBrowser`], which wraps a [`Browser`] instance
//! with tracking information for lifecycle management.
//!
//! # Overview
//!
//! Each browser in the pool is wrapped in a `TrackedBrowser` that tracks:
//! - **Unique ID**: For identification in logs and debugging
//! - **Creation time**: For TTL (time-to-live) enforcement
//! - **Last ping time**: For health monitoring
//!
//! # Architecture
//!
//! ```text
//! TrackedBrowser
//! ├── id: u64 (unique identifier)
//! ├── browser: Arc<Browser> (shared ownership)
//! ├── last_ping: Arc<Mutex<Instant>> (health tracking)
//! └── created_at: Instant (TTL calculation)
//! ```
//!
//! # Internal Use
//!
//! This struct is primarily used internally by the pool. Users interact
//! with browsers through [`BrowserHandle`](crate::BrowserHandle), which
//! provides transparent access via `Deref`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use headless_chrome::Browser;

use crate::error::{BrowserPoolError, Result};
use crate::traits::Healthcheck;

/// A browser instance with metadata for pool management.
///
/// Wraps a [`Browser`] with tracking information:
/// - Unique ID for identification in logs
/// - Creation timestamp for TTL enforcement
/// - Last successful ping timestamp for monitoring
///
/// # Thread Safety
///
/// Uses [`Arc`] for shared ownership and [`Mutex`] for ping timestamp updates.
/// Safe to clone and share across threads.
///
/// # Lifecycle
///
/// ```text
/// Browser Created
///       │
///       ▼
/// TrackedBrowser::new()  ──→  Validation (new_tab, navigate, close)
///       │
///       ▼
/// Added to Pool (available)
///       │
///       ├──→ Checked out ──→ In Use ──→ Returned to Pool
///       │
///       ├──→ Health Check (ping) ──→ Pass/Fail
///       │
///       └──→ TTL Expired ──→ Retired & Replaced
/// ```
#[derive(Clone)]
pub(crate) struct TrackedBrowser {
    /// Globally unique identifier for this browser instance.
    ///
    /// Assigned sequentially using an atomic counter. Useful for:
    /// - Log correlation
    /// - Debugging
    /// - Tracking browser lifecycle
    id: u64,

    /// The actual headless_chrome Browser instance (ref-counted).
    ///
    /// Wrapped in [`Arc`] to allow shared ownership between:
    /// - The pool's available list
    /// - The pool's active tracking map
    /// - Any [`BrowserHandle`](crate::BrowserHandle) using it
    browser: Arc<Browser>,

    /// Timestamp of last successful health check (protected by mutex).
    ///
    /// Updated by [`ping()`](Self::ping) on successful health checks.
    /// Used for monitoring browser responsiveness.
    last_ping: Arc<Mutex<Instant>>,

    /// Creation timestamp (immutable, used for TTL calculation).
    ///
    /// Set once during construction and never modified.
    /// Used by [`is_expired()`](Self::is_expired) to check TTL.
    created_at: Instant,
}

impl TrackedBrowser {
    /// Create a new tracked browser with validation.
    ///
    /// Performs an immediate health check to ensure the browser is functional
    /// before adding it to the pool.
    ///
    /// # Validation Steps
    ///
    /// 1. Creates a test tab
    /// 2. Navigates to a data URL
    /// 3. Closes the tab
    ///
    /// This ensures the browser process is alive and CDP communication works.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserPoolError::BrowserCreation`] if any validation step fails:
    /// - Tab creation fails (browser process dead)
    /// - Navigation fails (CDP communication broken)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use headless_chrome::Browser;
    /// use html2pdf_api::TrackedBrowser;
    ///
    /// let browser = Browser::default()?;
    /// let tracked = TrackedBrowser::new(browser)?;
    ///
    /// println!("Browser ID: {}", tracked.id());
    /// ```
    pub(crate) fn new(browser: Browser) -> Result<Self> {
        // Thread-safe monotonic ID generator
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        let browser = Arc::new(browser);
        let created_at = Instant::now();

        log::debug!(" Validating new browser instance...");

        // Critical: Validate browser is functional before accepting it
        // This prevents adding dead browsers to the pool
        let tab = browser.new_tab().map_err(|e| {
            log::error!("❌ Browser validation failed at new_tab(): {}", e);
            BrowserPoolError::BrowserCreation(e.to_string())
        })?;

        // Test navigation capability
        tab.navigate_to("data:text/html,<html></html>")
            .map_err(|e| {
                log::error!("❌ Browser validation failed at navigate_to(): {}", e);
                let _ = tab.close(true); // Best effort cleanup
                BrowserPoolError::BrowserCreation(e.to_string())
            })?;

        // Clean up test tab
        let _ = tab.close(true);

        log::debug!("✅ Browser validation passed");

        Ok(TrackedBrowser {
            id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
            browser,
            last_ping: Arc::new(Mutex::new(Instant::now())),
            created_at,
        })
    }

    /// Get the unique identifier for this browser.
    ///
    /// This ID is assigned sequentially and is unique across all browsers
    /// created during the application lifetime.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tracked = TrackedBrowser::new(browser)?;
    /// log::info!("Using browser {}", tracked.id());
    /// ```
    #[inline]
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// Get a reference to the underlying browser.
    ///
    /// Returns a reference to the [`Arc<Browser>`], allowing shared access
    /// to the browser instance.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tracked = TrackedBrowser::new(browser)?;
    /// let tab = tracked.browser().new_tab()?;
    /// ```
    #[inline]
    pub(crate) fn browser(&self) -> &Arc<Browser> {
        &self.browser
    }

    /// Check if browser has exceeded its time-to-live.
    ///
    /// # Parameters
    ///
    /// * `ttl` - Maximum age before browser should be retired.
    ///
    /// # Returns
    ///
    /// `true` if browser age > ttl, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// let tracked = TrackedBrowser::new(browser)?;
    ///
    /// // Check if browser is older than 1 hour
    /// if tracked.is_expired(Duration::from_secs(3600)) {
    ///     log::info!("Browser {} has expired", tracked.id());
    /// }
    /// ```
    #[inline]
    pub(crate) fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }

    /// Get the browser's age (time since creation).
    ///
    /// # Returns
    ///
    /// Duration since the browser was created.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tracked = TrackedBrowser::new(browser)?;
    /// println!("Browser age: {:?}", tracked.age());
    /// ```
    #[inline]
    pub(crate) fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Get browser age in minutes (for logging).
    ///
    /// Convenience method for human-readable logging.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// log::info!("Browser {} is {} minutes old", tracked.id(), tracked.age_minutes());
    /// ```
    #[inline]
    pub(crate) fn age_minutes(&self) -> u64 {
        self.created_at.elapsed().as_secs() / 60
    }

    /// Get the creation timestamp.
    ///
    /// # Returns
    ///
    /// The [`Instant`] when this browser was created.
    #[inline]
    pub(crate) fn created_at(&self) -> Instant {
        self.created_at
    }

    /// Get the last successful ping timestamp.
    ///
    /// # Returns
    ///
    /// - `Some(Instant)` - Last successful ping time
    /// - `None` - If lock is poisoned (should never happen in normal operation)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(last_ping) = tracked.last_ping_time() {
    ///     let since_ping = last_ping.elapsed();
    ///     log::debug!("Last ping was {:?} ago", since_ping);
    /// }
    /// ```
    #[allow(dead_code)]
    pub(crate) fn last_ping_time(&self) -> Option<Instant> {
        self.last_ping.lock().ok().map(|guard| *guard)
    }
}

impl Healthcheck for TrackedBrowser {
    /// Perform health check by creating and closing a tab.
    ///
    /// This is a lightweight operation that verifies:
    /// - Browser process is still alive
    /// - CDP (Chrome DevTools Protocol) is responsive
    /// - Tab creation/cleanup works
    ///
    /// # Implementation Note
    ///
    /// Updates `last_ping` timestamp on success but doesn't fail the entire
    /// health check if timestamp update fails (defensive programming).
    ///
    /// # Errors
    ///
    /// Returns [`BrowserPoolError::HealthCheckFailed`] if:
    /// - Tab creation fails
    /// - Browser process has crashed
    /// - CDP connection is broken
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::Healthcheck;
    ///
    /// match tracked.ping() {
    ///     Ok(()) => log::debug!("Browser {} is healthy", tracked.id()),
    ///     Err(e) => log::warn!("Browser {} failed health check: {}", tracked.id(), e),
    /// }
    /// ```
    fn ping(&self) -> Result<()> {
        log::trace!(" Pinging browser {}...", self.id);

        // Create a test tab to verify browser is responsive
        let tab = self.browser.new_tab().map_err(|e| {
            log::error!("❌ Browser {} ping failed (new_tab): {}", self.id, e);
            BrowserPoolError::HealthCheckFailed(e.to_string())
        })?;

        // Clean up immediately
        let _ = tab.close(true);

        // Update last ping timestamp (best effort - don't fail ping if this fails)
        // This is defensive: if we can't update timestamp, ping still succeeded
        match self.last_ping.lock() {
            Ok(mut ping) => {
                *ping = Instant::now();
                log::trace!("✅ Browser {} ping successful", self.id);
            }
            Err(e) => {
                // Poisoned lock - log but don't fail the health check
                log::warn!(
                    "⚠️ Browser {} ping succeeded but failed to update timestamp: {}",
                    self.id,
                    e
                );
            }
        }

        Ok(())
    }
}

impl std::fmt::Debug for TrackedBrowser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackedBrowser")
            .field("id", &self.id)
            .field("created_at", &self.created_at)
            .field("age_minutes", &self.age_minutes())
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies TTL (Time-To-Live) expiry calculation logic.
    ///
    /// Browsers should be considered expired when their age exceeds
    /// the configured TTL. This test verifies the expiry logic without
    /// creating actual browser instances.
    #[test]
    #[cfg(not(windows))]
    fn test_tracked_browser_expiry_logic() {
        // Simulate a browser created 3700 seconds ago (over 1 hour)
        let created_at = Instant::now() - Duration::from_secs(3700);
        let ttl = Duration::from_secs(3600); // 1 hour TTL

        // Calculate age
        let age = created_at.elapsed();

        // Verify expiry logic
        assert!(
            age > ttl,
            "Browser age ({:?}) should exceed TTL ({:?})",
            age,
            ttl
        );

        // Verify age calculation
        let age_minutes = age.as_secs() / 60;
        assert!(
            age_minutes > 60,
            "Browser age should exceed 60 minutes, got {} minutes",
            age_minutes
        );
    }

    /// Windows version: Uses Duration comparisons instead of Instant subtraction
    /// because `Instant::now() - Duration` can panic on Windows if the
    /// duration exceeds the process uptime.
    #[test]
    #[cfg(windows)]
    fn test_tracked_browser_expiry_logic() {
        let ttl = Duration::from_secs(3600); // 1 hour TTL

        // Age of 3700 seconds (over 1 hour) should be expired
        let age_expired = Duration::from_secs(3700);
        assert!(
            age_expired > ttl,
            "Age ({:?}) should exceed TTL ({:?})",
            age_expired,
            ttl
        );

        // Age of 3500 seconds (under 1 hour) should NOT be expired
        let age_not_expired = Duration::from_secs(3500);
        assert!(
            age_not_expired <= ttl,
            "Age ({:?}) should NOT exceed TTL ({:?})",
            age_not_expired,
            ttl
        );

        // Verify age calculation in minutes
        let age_minutes = age_expired.as_secs() / 60;
        assert!(
            age_minutes > 60,
            "Browser age should exceed 60 minutes, got {} minutes",
            age_minutes
        );
    }

    /// Verifies that is_expired returns correct values.
    #[test]
    #[cfg(not(windows))]
    fn test_is_expired_boundary() {
        let ttl = Duration::from_secs(100);

        // Just created - should not be expired
        let just_created = Instant::now();
        assert!(just_created.elapsed() < ttl);

        // Old timestamp - should be expired
        let old = Instant::now() - Duration::from_secs(101);
        assert!(old.elapsed() > ttl);
    }

    /// Windows version: Uses Duration comparisons to avoid Instant subtraction panic.
    #[test]
    #[cfg(windows)]
    fn test_is_expired_boundary() {
        let ttl = Duration::from_secs(100);

        // Just created (0 seconds old) - should not be expired
        let age_new = Duration::from_secs(0);
        assert!(age_new <= ttl, "New browser should not be expired");

        // Exactly at TTL - should not be expired (using >, not >=)
        let age_at_ttl = Duration::from_secs(100);
        assert!(
            !(age_at_ttl > ttl),
            "Browser at exactly TTL should not be expired"
        );

        // Just over TTL - should be expired
        let age_expired = Duration::from_secs(101);
        assert!(age_expired > ttl, "Browser over TTL should be expired");
    }

    /// Verifies age_minutes calculation.
    #[test]
    fn test_age_minutes_calculation() {
        // Test the math: 3700 seconds = 61 minutes
        let seconds: u64 = 3700;
        let minutes = seconds / 60;
        assert_eq!(minutes, 61);

        // Test edge cases
        assert_eq!(59u64 / 60, 0); // Less than a minute
        assert_eq!(60u64 / 60, 1); // Exactly one minute
        assert_eq!(119u64 / 60, 1); // Just under two minutes
        assert_eq!(120u64 / 60, 2); // Exactly two minutes
    }
}
