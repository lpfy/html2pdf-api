//! Configuration for browser pool behavior and limits.
//!
//! This module provides [`BrowserPoolConfig`] and [`BrowserPoolConfigBuilder`]
//! for configuring pool size, browser lifecycle, and health monitoring parameters.
//!
//! # Example
//!
//! ```rust
//! use std::time::Duration;
//! use html2pdf_api::BrowserPoolConfigBuilder;
//!
//! let config = BrowserPoolConfigBuilder::new()
//!     .max_pool_size(10)
//!     .warmup_count(5)
//!     .browser_ttl(Duration::from_secs(7200))
//!     .build()
//!     .expect("Invalid configuration");
//!
//! assert_eq!(config.max_pool_size, 10);
//! assert_eq!(config.warmup_count, 5);
//! ```
//!
//! # Environment Configuration
//!
//! When the `env-config` feature is enabled, you can load configuration
//! from environment variables and an optional `app.env` file:
//!
//! ```rust,ignore
//! use html2pdf_api::config::env::from_env;
//!
//! let config = from_env()?;
//! ```
//!
//! See [`mod@env`] module for available environment variables.

use std::time::Duration;

/// Configuration for browser pool behavior and limits.
///
/// Controls pool size, browser lifecycle, and health monitoring parameters.
/// Use [`BrowserPoolConfigBuilder`] for validation and convenience.
///
/// # Fields Overview
///
/// | Field | Default | Description |
/// |-------|---------|-------------|
/// | `max_pool_size` | 5 | Maximum browsers in pool |
/// | `warmup_count` | 3 | Browsers to pre-create |
/// | `ping_interval` | 15s | Health check frequency |
/// | `browser_ttl` | 1 hour | Browser lifetime |
/// | `max_ping_failures` | 3 | Failures before removal |
/// | `warmup_timeout` | 60s | Warmup time limit |
///
/// # Example
///
/// ```rust
/// use html2pdf_api::BrowserPoolConfig;
///
/// // Use defaults
/// let config = BrowserPoolConfig::default();
/// assert_eq!(config.max_pool_size, 5);
/// ```
#[derive(Debug, Clone)]
pub struct BrowserPoolConfig {
    /// Maximum number of browsers to keep in the pool (idle + active).
    ///
    /// This is a soft limit - active browsers may temporarily exceed this during high load.
    ///
    /// # Default
    ///
    /// 5 browsers
    ///
    /// # Considerations
    ///
    /// - Higher values = more memory usage, better concurrency
    /// - Lower values = less memory, potential queuing under load
    pub max_pool_size: usize,

    /// Number of browsers to pre-create during warmup phase.
    ///
    /// Must be d `max_pool_size`. Reduces first-request latency.
    ///
    /// # Default
    ///
    /// 3 browsers
    ///
    /// # Considerations
    ///
    /// - Set to `max_pool_size` for fastest first requests
    /// - Set to 0 for lazy initialization (browsers created on demand)
    pub warmup_count: usize,

    /// Interval between health check pings for active browsers.
    ///
    /// Shorter intervals = faster failure detection, higher overhead.
    ///
    /// # Default
    ///
    /// 15 seconds
    ///
    /// # Considerations
    ///
    /// - Too short: Unnecessary CPU/memory overhead
    /// - Too long: Slow detection of crashed browsers
    pub ping_interval: Duration,

    /// Time-to-live for each browser instance before forced retirement.
    ///
    /// Prevents memory leaks from long-running browser processes.
    ///
    /// # Default
    ///
    /// 1 hour (3600 seconds)
    ///
    /// # Considerations
    ///
    /// - Chrome can accumulate memory over time
    /// - Shorter TTL = more browser restarts, fresher instances
    /// - Longer TTL = fewer restarts, potential memory growth
    pub browser_ttl: Duration,

    /// Maximum consecutive ping failures before removing a browser.
    ///
    /// Higher values = more tolerance for transient failures.
    ///
    /// # Default
    ///
    /// 3 consecutive failures
    ///
    /// # Considerations
    ///
    /// - Set to 1 for aggressive failure detection
    /// - Set higher if experiencing transient network issues
    pub max_ping_failures: u32,

    /// Maximum time allowed for warmup process to complete.
    ///
    /// If warmup doesn't complete in this time, it fails with timeout error.
    ///
    /// # Default
    ///
    /// 60 seconds
    ///
    /// # Considerations
    ///
    /// - Should be at least `warmup_count * ~5 seconds` per browser
    /// - Increase if running on slow hardware or with many warmup browsers
    pub warmup_timeout: Duration,
}

impl Default for BrowserPoolConfig {
    /// Production-ready default configuration.
    ///
    /// - Pool size: 5 browsers
    /// - Warmup: 3 browsers
    /// - Health checks: Every 15 seconds
    /// - TTL: 1 hour
    /// - Failure tolerance: 3 consecutive failures
    /// - Warmup timeout: 60 seconds
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolConfig;
    /// use std::time::Duration;
    ///
    /// let config = BrowserPoolConfig::default();
    ///
    /// assert_eq!(config.max_pool_size, 5);
    /// assert_eq!(config.warmup_count, 3);
    /// assert_eq!(config.ping_interval, Duration::from_secs(15));
    /// assert_eq!(config.browser_ttl, Duration::from_secs(3600));
    /// assert_eq!(config.max_ping_failures, 3);
    /// assert_eq!(config.warmup_timeout, Duration::from_secs(60));
    /// ```
    fn default() -> Self {
        Self {
            max_pool_size: 5,
            warmup_count: 3,
            ping_interval: Duration::from_secs(15),
            browser_ttl: Duration::from_secs(3600), // 1 hour
            max_ping_failures: 3,
            warmup_timeout: Duration::from_secs(60),
        }
    }
}

/// Builder for [`BrowserPoolConfig`] with validation.
///
/// Provides a fluent API for constructing validated configurations.
/// All setter methods can be chained together.
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
/// use html2pdf_api::BrowserPoolConfigBuilder;
///
/// let config = BrowserPoolConfigBuilder::new()
///     .max_pool_size(10)
///     .warmup_count(5)
///     .browser_ttl(Duration::from_secs(7200))
///     .build()
///     .expect("Invalid configuration");
/// ```
///
/// # Validation
///
/// The [`build()`](Self::build) method validates:
/// - `max_pool_size` must be greater than 0
/// - `warmup_count` must be d `max_pool_size`
pub struct BrowserPoolConfigBuilder {
    config: BrowserPoolConfig,
}

impl BrowserPoolConfigBuilder {
    /// Create a new builder with default values.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolConfigBuilder;
    ///
    /// let builder = BrowserPoolConfigBuilder::new();
    /// let config = builder.build().unwrap();
    ///
    /// // Has default values
    /// assert_eq!(config.max_pool_size, 5);
    /// ```
    pub fn new() -> Self {
        Self {
            config: BrowserPoolConfig::default(),
        }
    }

    /// Set maximum pool size (must be > 0).
    ///
    /// # Parameters
    ///
    /// * `size` - Maximum number of browsers in the pool.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolConfigBuilder;
    ///
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .max_pool_size(10)
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(config.max_pool_size, 10);
    /// ```
    pub fn max_pool_size(mut self, size: usize) -> Self {
        self.config.max_pool_size = size;
        self
    }

    /// Set warmup count (must be d max_pool_size).
    ///
    /// # Parameters
    ///
    /// * `count` - Number of browsers to pre-create during warmup.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolConfigBuilder;
    ///
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .max_pool_size(10)
    ///     .warmup_count(5)
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(config.warmup_count, 5);
    /// ```
    pub fn warmup_count(mut self, count: usize) -> Self {
        self.config.warmup_count = count;
        self
    }

    /// Set health check interval.
    ///
    /// # Parameters
    ///
    /// * `interval` - Duration between health check pings.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use html2pdf_api::BrowserPoolConfigBuilder;
    ///
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .ping_interval(Duration::from_secs(30))
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(config.ping_interval, Duration::from_secs(30));
    /// ```
    pub fn ping_interval(mut self, interval: Duration) -> Self {
        self.config.ping_interval = interval;
        self
    }

    /// Set browser time-to-live before forced retirement.
    ///
    /// # Parameters
    ///
    /// * `ttl` - Maximum lifetime for each browser instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use html2pdf_api::BrowserPoolConfigBuilder;
    ///
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .browser_ttl(Duration::from_secs(7200)) // 2 hours
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(config.browser_ttl, Duration::from_secs(7200));
    /// ```
    pub fn browser_ttl(mut self, ttl: Duration) -> Self {
        self.config.browser_ttl = ttl;
        self
    }

    /// Set maximum consecutive ping failures before removal.
    ///
    /// # Parameters
    ///
    /// * `failures` - Number of consecutive failures tolerated.
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolConfigBuilder;
    ///
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .max_ping_failures(5)
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(config.max_ping_failures, 5);
    /// ```
    pub fn max_ping_failures(mut self, failures: u32) -> Self {
        self.config.max_ping_failures = failures;
        self
    }

    /// Set warmup timeout.
    ///
    /// # Parameters
    ///
    /// * `timeout` - Maximum time allowed for warmup to complete.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use html2pdf_api::BrowserPoolConfigBuilder;
    ///
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .warmup_timeout(Duration::from_secs(120))
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(config.warmup_timeout, Duration::from_secs(120));
    /// ```
    pub fn warmup_timeout(mut self, timeout: Duration) -> Self {
        self.config.warmup_timeout = timeout;
        self
    }

    /// Build and validate the configuration.
    ///
    /// # Errors
    ///
    /// - Returns error if `max_pool_size` is 0
    /// - Returns error if `warmup_count` > `max_pool_size`
    ///
    /// # Example
    ///
    /// ```rust
    /// use html2pdf_api::BrowserPoolConfigBuilder;
    ///
    /// // Valid configuration
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .max_pool_size(10)
    ///     .warmup_count(5)
    ///     .build();
    /// assert!(config.is_ok());
    ///
    /// // Invalid: pool size is 0
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .max_pool_size(0)
    ///     .build();
    /// assert!(config.is_err());
    ///
    /// // Invalid: warmup exceeds pool size
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .max_pool_size(5)
    ///     .warmup_count(10)
    ///     .build();
    /// assert!(config.is_err());
    /// ```
    pub fn build(self) -> std::result::Result<BrowserPoolConfig, String> {
        // Validation: Pool size must be positive
        if self.config.max_pool_size == 0 {
            return Err("max_pool_size must be greater than 0".to_string());
        }

        // Validation: Can't warmup more browsers than pool can hold
        if self.config.warmup_count > self.config.max_pool_size {
            return Err("warmup_count cannot exceed max_pool_size".to_string());
        }

        Ok(self.config)
    }
}

impl Default for BrowserPoolConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Environment Configuration (feature-gated)
// ============================================================================

/// Environment-based configuration loading.
///
/// This module is only available when the `env-config` feature is enabled.
///
/// # Environment File
///
/// This module uses `dotenvy` to load environment variables from an `app.env`
/// file in the current directory. The file is optional - if not found,
/// environment variables and defaults are used.
///
/// # Environment Variables
///
/// | Variable | Type | Default | Description |
/// |----------|------|---------|-------------|
/// | `BROWSER_POOL_SIZE` | usize | 5 | Maximum pool size |
/// | `BROWSER_WARMUP_COUNT` | usize | 3 | Warmup browser count |
/// | `BROWSER_TTL_SECONDS` | u64 | 3600 | Browser TTL in seconds |
/// | `BROWSER_WARMUP_TIMEOUT_SECONDS` | u64 | 60 | Warmup timeout |
/// | `BROWSER_PING_INTERVAL_SECONDS` | u64 | 15 | Health check interval |
/// | `BROWSER_MAX_PING_FAILURES` | u32 | 3 | Max ping failures |
/// | `CHROME_PATH` | String | auto | Custom Chrome binary path |
///
/// # Example `app.env` File
///
/// ```text
/// # Browser Pool Configuration
/// BROWSER_POOL_SIZE=5
/// BROWSER_WARMUP_COUNT=3
/// BROWSER_TTL_SECONDS=3600
/// BROWSER_WARMUP_TIMEOUT_SECONDS=60
/// BROWSER_PING_INTERVAL_SECONDS=15
/// BROWSER_MAX_PING_FAILURES=3
///
/// # Chrome Configuration (optional)
/// # CHROME_PATH=/usr/bin/google-chrome
/// ```
#[cfg(feature = "env-config")]
pub mod env {
    use super::*;
    use crate::error::BrowserPoolError;

    /// Default environment file name.
    pub const ENV_FILE_NAME: &str = "app.env";

    /// Load environment variables from `app.env` file.
    ///
    /// Call this early in your application startup to ensure environment
    /// variables are loaded before any configuration functions are called.
    ///
    /// This function is automatically called by [`from_env`], but you can
    /// call it explicitly if you need to load the file earlier or check
    /// for errors.
    ///
    /// # Returns
    ///
    /// - `Ok(PathBuf)` if the file was found and loaded successfully
    /// - `Err(dotenvy::Error)` if the file was not found or couldn't be parsed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::config::env::load_env_file;
    ///
    /// // Load at application startup
    /// match load_env_file() {
    ///     Ok(path) => println!("Loaded environment from: {:?}", path),
    ///     Err(e) => println!("No app.env file found: {}", e),
    /// }
    /// ```
    pub fn load_env_file() -> Result<std::path::PathBuf, dotenvy::Error> {
        dotenvy::from_filename(ENV_FILE_NAME)
    }

    /// Load configuration from environment variables.
    ///
    /// Reads configuration from environment variables with sensible defaults.
    /// Also loads `app.env` file if present (via `dotenvy`).
    ///
    /// # Environment File
    ///
    /// This function looks for an `app.env` file in the current directory
    /// and loads it if present. The file is optional - if not found,
    /// environment variables and defaults are used.
    ///
    /// # Environment Variables
    ///
    /// - `BROWSER_POOL_SIZE`: Maximum pool size (default: 5)
    /// - `BROWSER_WARMUP_COUNT`: Warmup browser count (default: 3)
    /// - `BROWSER_TTL_SECONDS`: Browser TTL in seconds (default: 3600)
    /// - `BROWSER_WARMUP_TIMEOUT_SECONDS`: Warmup timeout (default: 60)
    /// - `BROWSER_PING_INTERVAL_SECONDS`: Health check interval (default: 15)
    /// - `BROWSER_MAX_PING_FAILURES`: Max ping failures (default: 3)
    ///
    /// # Errors
    ///
    /// Returns [`BrowserPoolError::Configuration`] if configuration values are invalid.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::config::env::from_env;
    ///
    /// // Set environment variables before calling
    /// std::env::set_var("BROWSER_POOL_SIZE", "10");
    ///
    /// let config = from_env()?;
    /// assert_eq!(config.max_pool_size, 10);
    /// ```
    pub fn from_env() -> Result<BrowserPoolConfig, BrowserPoolError> {
        // Load app.env file if present (ignore errors if not found)
        match load_env_file() {
            Ok(path) => {
                log::info!("� Loaded configuration from: {:?}", path);
            }
            Err(e) => {
                log::debug!(
                    "� No {} file found or failed to load: {} (using environment variables and defaults)",
                    ENV_FILE_NAME,
                    e
                );
            }
        }

        let max_pool_size = std::env::var("BROWSER_POOL_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let warmup_count = std::env::var("BROWSER_WARMUP_COUNT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);

        let ttl_seconds = std::env::var("BROWSER_TTL_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600u64);

        let warmup_timeout_seconds = std::env::var("BROWSER_WARMUP_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60u64);

        let ping_interval_seconds = std::env::var("BROWSER_PING_INTERVAL_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(15u64);

        let max_ping_failures = std::env::var("BROWSER_MAX_PING_FAILURES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);

        log::info!("' Loading pool configuration from environment:");
        log::info!("   - Max pool size: {}", max_pool_size);
        log::info!("   - Warmup count: {}", warmup_count);
        log::info!(
            "   - Browser TTL: {}s ({}min)",
            ttl_seconds,
            ttl_seconds / 60
        );
        log::info!("   - Warmup timeout: {}s", warmup_timeout_seconds);
        log::info!("   - Ping interval: {}s", ping_interval_seconds);
        log::info!("   - Max ping failures: {}", max_ping_failures);

        BrowserPoolConfigBuilder::new()
            .max_pool_size(max_pool_size)
            .warmup_count(warmup_count)
            .browser_ttl(Duration::from_secs(ttl_seconds))
            .warmup_timeout(Duration::from_secs(warmup_timeout_seconds))
            .ping_interval(Duration::from_secs(ping_interval_seconds))
            .max_ping_failures(max_ping_failures)
            .build()
            .map_err(BrowserPoolError::Configuration)
    }

    /// Get Chrome path from environment.
    ///
    /// Reads `CHROME_PATH` environment variable.
    ///
    /// **Note:** Call [`from_env`] or [`load_env_file`] first to ensure
    /// `app.env` is loaded if you're using a configuration file.
    ///
    /// # Returns
    ///
    /// - `Some(path)` if `CHROME_PATH` is set
    /// - `None` if not set (will use auto-detection)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::config::env::{load_env_file, chrome_path_from_env};
    ///
    /// // Ensure app.env is loaded first
    /// let _ = load_env_file();
    ///
    /// let path = chrome_path_from_env();
    /// if let Some(p) = path {
    ///     println!("Using Chrome at: {}", p);
    /// }
    /// ```
    pub fn chrome_path_from_env() -> Option<String> {
        std::env::var("CHROME_PATH").ok()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that BrowserPoolConfigBuilder correctly sets all configuration values.
    ///
    /// Tests the happy path where all values are valid and within constraints.
    #[test]
    fn test_config_builder() {
        let config = BrowserPoolConfigBuilder::new()
            .max_pool_size(10)
            .warmup_count(5)
            .browser_ttl(Duration::from_secs(7200))
            .warmup_timeout(Duration::from_secs(120))
            .build()
            .unwrap();

        assert_eq!(config.max_pool_size, 10);
        assert_eq!(config.warmup_count, 5);
        assert_eq!(config.browser_ttl.as_secs(), 7200);
        assert_eq!(config.warmup_timeout.as_secs(), 120);
    }

    /// Verifies that config builder rejects invalid pool size (zero).
    ///
    /// Pool size must be at least 1 to be useful. This test ensures
    /// the validation catches this error at build time.
    #[test]
    fn test_config_validation() {
        let result = BrowserPoolConfigBuilder::new().max_pool_size(0).build();

        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("max_pool_size must be greater than 0"),
            "Expected validation error message, got: {}",
            err_msg
        );
    }

    /// Verifies that warmup count cannot exceed pool size.
    ///
    /// It's illogical to warmup more browsers than the pool can hold.
    /// This test ensures the configuration builder catches this mistake.
    #[test]
    fn test_config_warmup_exceeds_pool() {
        let result = BrowserPoolConfigBuilder::new()
            .max_pool_size(5)
            .warmup_count(10)
            .build();

        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("warmup_count cannot exceed max_pool_size"),
            "Expected validation error message, got: {}",
            err_msg
        );
    }

    /// Verifies that default configuration values are production-ready.
    ///
    /// These defaults are used when no explicit configuration is provided.
    /// They should be safe and reasonable for most use cases.
    #[test]
    fn test_config_defaults() {
        let config = BrowserPoolConfig::default();

        // Verify production-ready defaults
        assert_eq!(config.max_pool_size, 5, "Default pool size should be 5");
        assert_eq!(config.warmup_count, 3, "Default warmup should be 3");
        assert_eq!(
            config.ping_interval,
            Duration::from_secs(15),
            "Default ping interval should be 15s"
        );
        assert_eq!(
            config.browser_ttl,
            Duration::from_secs(3600),
            "Default TTL should be 1 hour"
        );
        assert_eq!(
            config.max_ping_failures, 3,
            "Default max failures should be 3"
        );
        assert_eq!(
            config.warmup_timeout,
            Duration::from_secs(60),
            "Default warmup timeout should be 60s"
        );
    }

    /// Verifies that config builder supports method chaining.
    ///
    /// The builder pattern should allow fluent API usage where all
    /// setters can be chained together.
    #[test]
    fn test_config_builder_chaining() {
        let config = BrowserPoolConfigBuilder::new()
            .max_pool_size(8)
            .warmup_count(4)
            .ping_interval(Duration::from_secs(30))
            .browser_ttl(Duration::from_secs(1800))
            .max_ping_failures(5)
            .warmup_timeout(Duration::from_secs(90))
            .build()
            .unwrap();

        // Verify all chained values were set correctly
        assert_eq!(config.max_pool_size, 8);
        assert_eq!(config.warmup_count, 4);
        assert_eq!(config.ping_interval.as_secs(), 30);
        assert_eq!(config.browser_ttl.as_secs(), 1800);
        assert_eq!(config.max_ping_failures, 5);
        assert_eq!(config.warmup_timeout.as_secs(), 90);
    }

    /// Verifies that BrowserPoolConfigBuilder implements Default.
    #[test]
    fn test_builder_default() {
        let builder: BrowserPoolConfigBuilder = Default::default();
        let config = builder.build().unwrap();

        // Should have same values as BrowserPoolConfig::default()
        assert_eq!(config.max_pool_size, 5);
        assert_eq!(config.warmup_count, 3);
    }
}
