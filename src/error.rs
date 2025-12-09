//! Error types for the browser pool.
//!
//! This module provides [`BrowserPoolError`], a unified error type for all
//! browser pool operations, and a convenient [`Result`] type alias.
//!
//! # Example
//!
//! ```rust
//! use html2pdf_api::{BrowserPoolError, Result};
//!
//! fn process_pdf() -> Result<Vec<u8>> {
//!     // Your logic here...
//!     Err(BrowserPoolError::Configuration("example error".to_string()))
//! }
//!
//! match process_pdf() {
//!     Ok(pdf) => println!("Generated {} bytes", pdf.len()),
//!     Err(BrowserPoolError::ShuttingDown) => println!("Pool is shutting down"),
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! ```

/// Errors that can occur during browser pool operations.
///
/// This enum represents all possible error conditions when working with
/// the browser pool. Each variant includes context about what went wrong.
///
/// # Example
///
/// ```rust
/// use html2pdf_api::BrowserPoolError;
///
/// fn handle_error(error: BrowserPoolError) {
///     match error {
///         BrowserPoolError::BrowserCreation(msg) => {
///             eprintln!("Browser creation failed: {}", msg);
///         }
///         BrowserPoolError::HealthCheckFailed(msg) => {
///             eprintln!("Health check failed: {}", msg);
///         }
///         BrowserPoolError::ShuttingDown => {
///             eprintln!("Pool is shutting down");
///         }
///         BrowserPoolError::Configuration(msg) => {
///             eprintln!("Configuration error: {}", msg);
///         }
///     }
/// }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum BrowserPoolError {
    /// Failed to create a new browser instance.
    ///
    /// This typically indicates Chrome/Chromium binary issues or launch flag problems.
    ///
    /// # Common Causes
    ///
    /// - Chrome/Chromium binary not found or not installed
    /// - Invalid Chrome binary path specified
    /// - Insufficient permissions to execute Chrome
    /// - Invalid or conflicting launch flags
    /// - System resource limits exceeded (e.g., too many processes)
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolError;
    ///
    /// let error = BrowserPoolError::BrowserCreation(
    ///     "Chrome binary not found".to_string()
    /// );
    /// println!("{}", error); // "Failed to create browser: Chrome binary not found"
    /// ```
    #[error("Failed to create browser: {0}")]
    BrowserCreation(String),

    /// Browser failed a health check operation.
    ///
    /// Triggered when ping operations (new_tab, navigate, close) fail.
    ///
    /// # Common Causes
    ///
    /// - Browser process crashed unexpectedly
    /// - Out of memory condition
    /// - CDP (Chrome DevTools Protocol) connection lost
    /// - Browser tab became unresponsive
    ///
    /// # Recovery
    ///
    /// The pool automatically removes unhealthy browsers and creates
    /// replacements. Users typically don't need to handle this error
    /// specially unless monitoring browser health.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolError;
    ///
    /// let error = BrowserPoolError::HealthCheckFailed(
    ///     "new_tab() failed: connection refused".to_string()
    /// );
    /// println!("{}", error); // "Browser health check failed: new_tab() failed: connection refused"
    /// ```
    #[error("Browser health check failed: {0}")]
    HealthCheckFailed(String),

    /// Operation attempted during pool shutdown.
    ///
    /// All operations are rejected once shutdown begins.
    ///
    /// This error is returned when:
    /// - [`BrowserPool::shutdown()`](crate::BrowserPool::shutdown) has been called
    /// - [`BrowserPool::shutdown_async()`](crate::BrowserPool::shutdown_async) has been called
    /// - The [`BrowserPool`](crate::BrowserPool) is being dropped
    ///
    /// # Handling
    ///
    /// This error typically occurs during application shutdown. Handle it
    /// gracefully by stopping any pending work rather than retrying.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolError;
    ///
    /// let error = BrowserPoolError::ShuttingDown;
    /// println!("{}", error); // "Pool is shutting down"
    /// ```
    #[error("Pool is shutting down")]
    ShuttingDown,

    /// Invalid configuration provided.
    ///
    /// This error occurs when pool configuration values are invalid.
    ///
    /// # Common Causes
    ///
    /// - `max_pool_size` is set to 0
    /// - `warmup_count` exceeds `max_pool_size`
    /// - Invalid Chrome binary path
    /// - Invalid duration values
    ///
    /// # Prevention
    ///
    /// Use [`BrowserPoolConfigBuilder`](crate::BrowserPoolConfigBuilder)
    /// which validates configuration at build time.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolError;
    ///
    /// let error = BrowserPoolError::Configuration(
    ///     "max_pool_size must be greater than 0".to_string()
    /// );
    /// println!("{}", error); // "Configuration error: max_pool_size must be greater than 0"
    /// ```
    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Convenience conversion from [`String`] to [`BrowserPoolError::Configuration`].
///
/// Allows using the `?` operator with functions that return `String` errors
/// in contexts expecting [`BrowserPoolError`].
///
/// # Example
///
/// ```rust
/// use html2pdf_api::BrowserPoolError;
///
/// let error: BrowserPoolError = "invalid configuration".to_string().into();
/// assert!(matches!(error, BrowserPoolError::Configuration(_)));
/// ```
impl From<String> for BrowserPoolError {
    fn from(msg: String) -> Self {
        BrowserPoolError::Configuration(msg)
    }
}

/// Convenience conversion from `&str` to [`BrowserPoolError::Configuration`].
///
/// Allows using string literals directly where [`BrowserPoolError`] is expected.
///
/// # Example
///
/// ```rust
/// use html2pdf_api::BrowserPoolError;
///
/// let error: BrowserPoolError = "invalid setting".into();
/// assert!(matches!(error, BrowserPoolError::Configuration(_)));
/// ```
impl From<&str> for BrowserPoolError {
    fn from(msg: &str) -> Self {
        BrowserPoolError::Configuration(msg.to_string())
    }
}

/// Result type alias using [`BrowserPoolError`].
///
/// This is the standard result type returned by most browser pool operations.
///
/// # Example
///
/// ```rust
/// use html2pdf_api::Result;
///
/// fn my_function() -> Result<String> {
///     Ok("success".to_string())
/// }
/// ```
pub type Result<T> = std::result::Result<T, BrowserPoolError>;

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies error type conversions from String and &str.
    ///
    /// The pool uses custom error types with convenient From implementations.
    /// This test ensures error conversion works as expected.
    #[test]
    fn test_error_conversion() {
        // Test conversion from &str
        let error: BrowserPoolError = "test error".into();
        match error {
            BrowserPoolError::Configuration(msg) => {
                assert_eq!(msg, "test error", "Error message should be preserved");
            }
            _ => panic!("Expected Configuration error variant"),
        }

        // Test conversion from String
        let error: BrowserPoolError = "another error".to_string().into();
        match error {
            BrowserPoolError::Configuration(msg) => {
                assert_eq!(msg, "another error", "Error message should be preserved");
            }
            _ => panic!("Expected Configuration error variant"),
        }
    }

    /// Verifies that error Display formatting works correctly.
    #[test]
    fn test_error_display() {
        let error = BrowserPoolError::BrowserCreation("chrome not found".to_string());
        assert_eq!(
            error.to_string(),
            "Failed to create browser: chrome not found"
        );

        let error = BrowserPoolError::HealthCheckFailed("ping failed".to_string());
        assert_eq!(
            error.to_string(),
            "Browser health check failed: ping failed"
        );

        let error = BrowserPoolError::ShuttingDown;
        assert_eq!(error.to_string(), "Pool is shutting down");

        let error = BrowserPoolError::Configuration("bad config".to_string());
        assert_eq!(error.to_string(), "Configuration error: bad config");
    }

    /// Verifies that BrowserPoolError implements std::error::Error.
    #[test]
    fn test_error_is_std_error() {
        fn assert_std_error<T: std::error::Error>() {}
        assert_std_error::<BrowserPoolError>();
    }

    /// Verifies that BrowserPoolError is Send + Sync for thread safety.
    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BrowserPoolError>();
    }
}
