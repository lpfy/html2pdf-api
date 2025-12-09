//! Mock browser factory for testing.
//!
//! This module provides a mock implementation of [`BrowserFactory`] that
//! can be configured to succeed or fail, useful for testing pool behavior
//! without requiring Chrome to be installed.
//!
//! # Feature Flag
//!
//! This module is only available when:
//! - The `test-utils` feature is enabled, OR
//! - During testing (`#[cfg(test)]`)
//!
//! # Example
//!
//! ```rust,ignore
//! use html2pdf_api::factory::mock::MockBrowserFactory;
//!
//! // Factory that always fails
//! let factory = MockBrowserFactory::always_fails("Chrome not installed");
//!
//! // Factory that fails after N successful creations
//! let factory = MockBrowserFactory::fail_after_n(3, "Resource exhausted");
//! ```

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use headless_chrome::Browser;

use crate::error::{BrowserPoolError, Result};
use super::BrowserFactory;

/// Mock browser factory for testing without Chrome.
///
/// This factory can be configured to:
/// - Always succeed (creates real browsers if Chrome available)
/// - Always fail with a specific error
/// - Fail after N successful creations
/// - Track creation count for verification
///
/// # Thread Safety
///
/// This factory is `Send + Sync` and tracks state using atomic operations.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::factory::mock::MockBrowserFactory;
///
/// // Create a factory that always fails
/// let factory = MockBrowserFactory::always_fails("Test error");
/// assert!(factory.create().is_err());
/// assert_eq!(factory.creation_count(), 1);
///
/// // Create a factory that fails after 2 successful creations
/// let factory = MockBrowserFactory::fail_after_n(2, "Exhausted");
/// ```
pub struct MockBrowserFactory {
    /// Whether to fail on creation.
    should_fail: bool,

    /// Custom error message when failing.
    error_message: String,

    /// Number of browsers created (for verification in tests).
    creation_count: Arc<AtomicUsize>,

    /// Optional: fail after this many successful creations.
    fail_after: Option<usize>,
}

impl MockBrowserFactory {
    /// Create a mock factory that attempts real browser creation.
    ///
    /// Note: This still requires Chrome to be installed to actually
    /// create browsers. For pure mocking without Chrome, use
    /// [`always_fails`](Self::always_fails).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let factory = MockBrowserFactory::new();
    /// // Will attempt real browser creation
    /// let result = factory.create();
    /// ```
    pub fn new() -> Self {
        Self {
            should_fail: false,
            error_message: String::new(),
            creation_count: Arc::new(AtomicUsize::new(0)),
            fail_after: None,
        }
    }

    /// Create a mock factory that always fails with the given message.
    ///
    /// This is useful for testing error handling paths without
    /// requiring Chrome to be installed.
    ///
    /// # Parameters
    ///
    /// * `message` - Error message to return on creation attempts.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let factory = MockBrowserFactory::always_fails("Chrome not installed");
    /// let result = factory.create();
    /// assert!(result.is_err());
    /// ```
    pub fn always_fails<S: Into<String>>(message: S) -> Self {
        Self {
            should_fail: true,
            error_message: message.into(),
            creation_count: Arc::new(AtomicUsize::new(0)),
            fail_after: None,
        }
    }

    /// Create a mock factory that fails after N successful creations.
    ///
    /// Useful for testing pool behavior when browsers start failing
    /// after some have been successfully created (e.g., resource exhaustion).
    ///
    /// # Parameters
    ///
    /// * `n` - Number of successful creations before failing.
    /// * `message` - Error message after failures begin.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let factory = MockBrowserFactory::fail_after_n(3, "Resource exhausted");
    /// // First 3 calls may succeed (if Chrome installed), subsequent calls fail
    /// ```
    pub fn fail_after_n<S: Into<String>>(n: usize, message: S) -> Self {
        Self {
            should_fail: false,
            error_message: message.into(),
            creation_count: Arc::new(AtomicUsize::new(0)),
            fail_after: Some(n),
        }
    }

    /// Get the number of browser creation attempts by this factory.
    ///
    /// Useful for verifying pool behavior in tests.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let factory = MockBrowserFactory::always_fails("test");
    /// assert_eq!(factory.creation_count(), 0);
    /// let _ = factory.create();
    /// assert_eq!(factory.creation_count(), 1);
    /// ```
    pub fn creation_count(&self) -> usize {
        self.creation_count.load(Ordering::SeqCst)
    }

    /// Reset the creation counter to zero.
    ///
    /// Useful when reusing a factory across multiple tests.
    pub fn reset_count(&self) {
        self.creation_count.store(0, Ordering::SeqCst);
    }

    /// Get a clone of the creation counter for external tracking.
    ///
    /// This allows test code to monitor creation count even after
    /// the factory has been moved into a pool.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let factory = MockBrowserFactory::new();
    /// let counter = factory.counter();
    ///
    /// // Move factory into pool
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(factory))
    ///     .build()?;
    ///
    /// // Can still check count via cloned counter
    /// println!("Created: {}", counter.load(Ordering::SeqCst));
    /// ```
    pub fn counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.creation_count)
    }
}

impl Default for MockBrowserFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserFactory for MockBrowserFactory {
    /// Create a browser or return a mock error.
    ///
    /// Behavior depends on factory configuration:
    /// - If `should_fail` is true, always returns error
    /// - If `fail_after` is set and count exceeded, returns error
    /// - Otherwise, attempts real browser creation
    ///
    /// # Errors
    ///
    /// Returns [`BrowserPoolError::BrowserCreation`] when configured to fail.
    fn create(&self) -> Result<Browser> {
        let count = self.creation_count.fetch_add(1, Ordering::SeqCst);

        // Check if configured to always fail
        if self.should_fail {
            log::debug!("MockBrowserFactory: Returning configured failure");
            return Err(BrowserPoolError::BrowserCreation(
                self.error_message.clone(),
            ));
        }

        // Check if we should fail after N creations
        if let Some(fail_after) = self.fail_after {
            if count >= fail_after {
                log::debug!(
                    "MockBrowserFactory: Failing after {} creations",
                    fail_after
                );
                return Err(BrowserPoolError::BrowserCreation(
                    self.error_message.clone(),
                ));
            }
        }

        // Attempt real browser creation
        log::debug!("MockBrowserFactory: Attempting real browser creation #{}", count + 1);

        use super::chrome::create_chrome_options;

        let options = create_chrome_options(None)
            .map_err(|e| BrowserPoolError::Configuration(e.to_string()))?;

        Browser::new(options).map_err(|e| {
            log::error!("MockBrowserFactory: Real browser creation failed: {}", e);
            BrowserPoolError::BrowserCreation(e.to_string())
        })
    }
}

impl std::fmt::Debug for MockBrowserFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockBrowserFactory")
            .field("should_fail", &self.should_fail)
            .field("error_message", &self.error_message)
            .field("creation_count", &self.creation_count.load(Ordering::SeqCst))
            .field("fail_after", &self.fail_after)
            .finish()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that MockBrowserFactory can be created with different configurations.
    #[test]
    fn test_mock_factory_creation() {
        let _factory = MockBrowserFactory::new();
        let _factory = MockBrowserFactory::always_fails("test");
        let _factory = MockBrowserFactory::fail_after_n(3, "exhausted");
    }

    /// Verifies that always_fails factory returns error.
    #[test]
    fn test_mock_factory_always_fails() {
        let factory = MockBrowserFactory::always_fails("Test error");

        let result = factory.create();
        assert!(result.is_err());

        match result {
            Err(BrowserPoolError::BrowserCreation(msg)) => {
                assert_eq!(msg, "Test error");
            }
            _ => panic!("Expected BrowserCreation error"),
        }
    }

    /// Verifies that creation_count tracks attempts.
    #[test]
    fn test_mock_factory_creation_count() {
        let factory = MockBrowserFactory::always_fails("Test");

        assert_eq!(factory.creation_count(), 0);
        let _ = factory.create();
        assert_eq!(factory.creation_count(), 1);
        let _ = factory.create();
        assert_eq!(factory.creation_count(), 2);
    }

    /// Verifies that reset_count works.
    #[test]
    fn test_mock_factory_reset_count() {
        let factory = MockBrowserFactory::always_fails("Test");

        let _ = factory.create();
        let _ = factory.create();
        assert_eq!(factory.creation_count(), 2);

        factory.reset_count();
        assert_eq!(factory.creation_count(), 0);
    }

    /// Verifies that counter() returns a shared reference.
    #[test]
    fn test_mock_factory_counter() {
        let factory = MockBrowserFactory::always_fails("Test");
        let counter = factory.counter();

        assert_eq!(counter.load(Ordering::SeqCst), 0);
        let _ = factory.create();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// Verifies fail_after_n behavior.
    #[test]
    fn test_mock_factory_fail_after_n() {
        let factory = MockBrowserFactory::fail_after_n(2, "Exhausted");

        // First two attempts increment counter but may succeed or fail
        // depending on Chrome availability - we just verify the count
        let _ = factory.create();
        let _ = factory.create();
        assert_eq!(factory.creation_count(), 2);

        // Third attempt should definitely fail with our message
        let result = factory.create();
        assert!(result.is_err());

        if let Err(BrowserPoolError::BrowserCreation(msg)) = result {
            assert_eq!(msg, "Exhausted");
        }
    }

    /// Verifies Default implementation.
    #[test]
    fn test_mock_factory_default() {
        let factory: MockBrowserFactory = Default::default();
        assert_eq!(factory.creation_count(), 0);
        assert!(!factory.should_fail);
    }

    /// Verifies Debug implementation.
    #[test]
    fn test_mock_factory_debug() {
        let factory = MockBrowserFactory::always_fails("Test");
        let debug_str = format!("{:?}", factory);

        assert!(debug_str.contains("MockBrowserFactory"));
        assert!(debug_str.contains("should_fail"));
        assert!(debug_str.contains("true"));
    }
}