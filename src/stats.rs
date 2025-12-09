//! Pool statistics for monitoring and health checks.
//!
//! This module provides [`PoolStats`], a snapshot of the browser pool's
//! current state. Use it for monitoring, logging, and health checks.
//!
//! # Example
//!
//! ```rust,ignore
//! use html2pdf_api::BrowserPool;
//!
//! let pool = BrowserPool::builder()
//!     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!     .build()?;
//!
//! let stats = pool.stats();
//! println!("Available: {}, Active: {}", stats.available, stats.active);
//! ```

/// Snapshot of pool statistics at a point in time.
///
/// Useful for monitoring, logging, and health checks.
///
/// # Fields
///
/// | Field | Description |
/// |-------|-------------|
/// | `available` | Browsers ready for checkout |
/// | `active` | All tracked browsers (pooled + checked-out) |
/// | `total` | Reserved for future use (currently same as `active`) |
///
/// # Example
///
/// ```rust
/// use html2pdf_api::PoolStats;
///
/// let stats = PoolStats {
///     available: 3,
///     active: 5,
///     total: 5,
/// };
///
/// println!("Pool status: {}/{} available", stats.available, stats.active);
/// ```
///
/// # Usage with BrowserPool
///
/// ```rust,ignore
/// let pool = /* ... */;
///
/// // Get current stats
/// let stats = pool.stats();
///
/// // Use for health checks
/// if stats.available == 0 {
///     log::warn!("No browsers available in pool!");
/// }
///
/// // Use for monitoring
/// metrics::gauge!("browser_pool.available", stats.available as f64);
/// metrics::gauge!("browser_pool.active", stats.active as f64);
/// ```
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Number of browsers available in pool (ready for checkout).
    ///
    /// These browsers are idle and can be immediately returned by
    /// [`BrowserPool::get()`](crate::BrowserPool::get).
    ///
    /// # Note
    ///
    /// This value can change immediately after reading if another thread
    /// checks out or returns a browser.
    pub available: usize,

    /// Number of active browsers (all browsers being tracked).
    ///
    /// This includes both pooled and checked-out browsers.
    ///
    /// # Relationship to `available`
    ///
    /// - `active` >= `available` (always)
    /// - `active` - `available` = browsers currently checked out
    pub active: usize,

    /// Total browsers (currently same as active, reserved for future use).
    ///
    /// # Future Use
    ///
    /// This field may be used to track browsers in different states
    /// (e.g., browsers being created, browsers being destroyed).
    pub total: usize,
}

impl PoolStats {
    /// Get the number of browsers currently checked out.
    ///
    /// This is a convenience method that calculates `active - available`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::PoolStats;
    ///
    /// let stats = PoolStats {
    ///     available: 3,
    ///     active: 5,
    ///     total: 5,
    /// };
    ///
    /// assert_eq!(stats.checked_out(), 2);
    /// ```
    #[inline]
    pub fn checked_out(&self) -> usize {
        self.active.saturating_sub(self.available)
    }

    /// Check if the pool has available browsers.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::PoolStats;
    ///
    /// let stats = PoolStats {
    ///     available: 3,
    ///     active: 5,
    ///     total: 5,
    /// };
    ///
    /// assert!(stats.has_available());
    /// ```
    #[inline]
    pub fn has_available(&self) -> bool {
        self.available > 0
    }

    /// Check if the pool is empty (no browsers at all).
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::PoolStats;
    ///
    /// let stats = PoolStats {
    ///     available: 0,
    ///     active: 0,
    ///     total: 0,
    /// };
    ///
    /// assert!(stats.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.active == 0
    }
}

impl std::fmt::Display for PoolStats {
    /// Format stats for logging.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::PoolStats;
    ///
    /// let stats = PoolStats {
    ///     available: 3,
    ///     active: 5,
    ///     total: 5,
    /// };
    ///
    /// assert_eq!(
    ///     stats.to_string(),
    ///     "PoolStats { available: 3, active: 5, total: 5 }"
    /// );
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PoolStats {{ available: {}, active: {}, total: {} }}",
            self.available, self.active, self.total
        )
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies PoolStats structure and field access.
    ///
    /// PoolStats is a simple data structure returned by `pool.stats()`.
    /// This test ensures the structure is correctly defined.
    #[test]
    fn test_pool_stats_structure() {
        let stats = PoolStats {
            available: 5,
            active: 3,
            total: 8,
        };

        assert_eq!(
            stats.available, 5,
            "Available browsers should be accessible"
        );
        assert_eq!(stats.active, 3, "Active browsers should be accessible");
        assert_eq!(stats.total, 8, "Total browsers should be accessible");
    }

    /// Verifies the checked_out() convenience method.
    #[test]
    fn test_checked_out() {
        let stats = PoolStats {
            available: 2,
            active: 5,
            total: 5,
        };

        assert_eq!(stats.checked_out(), 3);
    }

    /// Verifies checked_out() handles edge case where available > active.
    #[test]
    fn test_checked_out_saturating() {
        // Edge case: shouldn't happen in practice, but handle gracefully
        let stats = PoolStats {
            available: 10,
            active: 5,
            total: 5,
        };

        assert_eq!(stats.checked_out(), 0); // saturating_sub prevents underflow
    }

    /// Verifies has_available() method.
    #[test]
    fn test_has_available() {
        let stats_with = PoolStats {
            available: 1,
            active: 1,
            total: 1,
        };
        assert!(stats_with.has_available());

        let stats_without = PoolStats {
            available: 0,
            active: 1,
            total: 1,
        };
        assert!(!stats_without.has_available());
    }

    /// Verifies is_empty() method.
    #[test]
    fn test_is_empty() {
        let empty = PoolStats {
            available: 0,
            active: 0,
            total: 0,
        };
        assert!(empty.is_empty());

        let not_empty = PoolStats {
            available: 0,
            active: 1,
            total: 1,
        };
        assert!(!not_empty.is_empty());
    }

    /// Verifies Display implementation.
    #[test]
    fn test_display() {
        let stats = PoolStats {
            available: 3,
            active: 5,
            total: 5,
        };

        assert_eq!(
            stats.to_string(),
            "PoolStats { available: 3, active: 5, total: 5 }"
        );
    }

    /// Verifies that PoolStats implements Clone.
    #[test]
    fn test_clone() {
        let stats = PoolStats {
            available: 3,
            active: 5,
            total: 5,
        };

        let cloned = stats.clone();
        assert_eq!(cloned.available, stats.available);
        assert_eq!(cloned.active, stats.active);
        assert_eq!(cloned.total, stats.total);
    }

    /// Verifies that PoolStats implements Debug.
    #[test]
    fn test_debug() {
        let stats = PoolStats {
            available: 3,
            active: 5,
            total: 5,
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("PoolStats"));
        assert!(debug_str.contains("available"));
    }
}
