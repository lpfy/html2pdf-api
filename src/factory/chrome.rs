//! Chrome/Chromium browser factory implementation.
//!
//! This module provides [`ChromeBrowserFactory`] for creating headless Chrome
//! browser instances with production-ready configurations.
//!
//! # Overview
//!
//! The factory handles:
//! - Chrome binary path detection (or custom path)
//! - Launch options configuration
//! - Memory and stability optimizations
//!
//! # Example
//!
//! ```rust,ignore
//! use html2pdf_api::ChromeBrowserFactory;
//!
//! // Auto-detect Chrome installation
//! let factory = ChromeBrowserFactory::with_defaults();
//!
//! // Or specify custom path
//! let factory = ChromeBrowserFactory::with_path("/usr/bin/google-chrome".to_string());
//! ```

use headless_chrome::{Browser, LaunchOptions};

use crate::error::{BrowserPoolError, Result};
use super::BrowserFactory;

/// Factory for creating Chrome/Chromium browser instances.
///
/// Handles Chrome-specific launch options and path detection.
/// Supports both auto-detection and custom Chrome binary paths.
///
/// # Thread Safety
///
/// This factory is `Send + Sync` and can be safely shared across threads.
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::ChromeBrowserFactory;
///
/// // Auto-detect Chrome
/// let factory = ChromeBrowserFactory::with_defaults();
///
/// // Or use custom path
/// let factory = ChromeBrowserFactory::with_path("/usr/bin/google-chrome".to_string());
/// ```
pub struct ChromeBrowserFactory {
    /// Function that generates launch options for each browser.
    ///
    /// This allows dynamic configuration per browser instance.
    launch_options_fn: Box<dyn Fn() -> Result<LaunchOptions<'static>> + Send + Sync>,
}

impl ChromeBrowserFactory {
    /// Create factory with custom launch options function.
    ///
    /// This is the most flexible constructor, allowing full control
    /// over launch options generation.
    ///
    /// # Parameters
    ///
    /// * `launch_options_fn` - Function called for each browser creation.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::{ChromeBrowserFactory, create_chrome_options, BrowserPoolError};
    ///
    /// let factory = ChromeBrowserFactory::new(|| {
    ///     // Custom logic here
    ///     create_chrome_options(Some("/custom/path"))
    ///         .map_err(|e| BrowserPoolError::Configuration(e.to_string()))
    /// });
    /// ```
    pub fn new<F>(launch_options_fn: F) -> Self
    where
        F: Fn() -> Result<LaunchOptions<'static>> + Send + Sync + 'static,
    {
        Self {
            launch_options_fn: Box::new(launch_options_fn),
        }
    }

    /// Create factory with auto-detected Chrome path.
    ///
    /// This is the recommended default - lets headless_chrome find Chrome.
    /// Works on Linux, macOS, and Windows.
    ///
    /// # Platform Detection
    ///
    /// The `headless_chrome` crate searches common installation paths:
    ///
    /// | Platform | Paths Searched |
    /// |----------|----------------|
    /// | Linux | `/usr/bin/google-chrome`, `/usr/bin/chromium`, etc. |
    /// | macOS | `/Applications/Google Chrome.app/...` |
    /// | Windows | `C:\Program Files\Google\Chrome\...` |
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::ChromeBrowserFactory;
    ///
    /// let factory = ChromeBrowserFactory::with_defaults();
    /// ```
    pub fn with_defaults() -> Self {
        log::debug!(" Creating ChromeBrowserFactory with auto-detect");
        Self::new(|| {
            create_chrome_options(None)
                .map_err(|e| BrowserPoolError::Configuration(e.to_string()))
        })
    }

    /// Create factory with custom Chrome binary path.
    ///
    /// Use this when Chrome is installed in a non-standard location.
    ///
    /// # Parameters
    ///
    /// * `chrome_path` - Full path to Chrome/Chromium binary.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::ChromeBrowserFactory;
    ///
    /// // Linux
    /// let factory = ChromeBrowserFactory::with_path("/usr/bin/google-chrome".to_string());
    ///
    /// // macOS
    /// let factory = ChromeBrowserFactory::with_path(
    ///     "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string()
    /// );
    ///
    /// // Windows
    /// let factory = ChromeBrowserFactory::with_path(
    ///     r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string()
    /// );
    /// ```
    pub fn with_path(chrome_path: String) -> Self {
        log::debug!(" Creating ChromeBrowserFactory with custom path: {}", chrome_path);
        Self::new(move || {
            create_chrome_options(Some(&chrome_path))
                .map_err(|e| BrowserPoolError::Configuration(e.to_string()))
        })
    }
}

impl BrowserFactory for ChromeBrowserFactory {
    /// Create a new Chrome browser instance.
    ///
    /// Calls the launch options function and launches Chrome with those options.
    ///
    /// # Errors
    ///
    /// * Returns [`BrowserPoolError::Configuration`] if launch options generation fails.
    /// * Returns [`BrowserPoolError::BrowserCreation`] if Chrome fails to launch.
    fn create(&self) -> Result<Browser> {
        log::trace!(" ChromeBrowserFactory::create() called");

        // Generate launch options
        let options = (self.launch_options_fn)()?;

        // Launch browser
        log::debug!(" Launching Chrome browser...");
        Browser::new(options)
            .map_err(|e| {
                log::error!("❌ Chrome launch failed: {}", e);
                BrowserPoolError::BrowserCreation(e.to_string())
            })
    }
}

/// Create Chrome launch options with optional custom path.
///
/// This function generates production-ready Chrome launch options with:
/// - Memory optimization flags
/// - GPU acceleration disabled (for headless stability)
/// - Unnecessary features disabled
/// - Security settings for automation
///
/// # Parameters
///
/// * `chrome_path` - Optional custom Chrome binary path. If None, auto-detects.
///
/// # Returns
///
/// LaunchOptions configured for stable headless operation.
///
/// # Errors
///
/// Returns error if options builder fails (rare, usually a bug).
///
/// # Chrome Flags Applied
///
/// ## Memory and Performance
/// - `--disable-dev-shm-usage` - Use /tmp instead of /dev/shm (container-friendly)
/// - `--disable-crash-reporter` - No crash reporting
/// - `--max_old_space_size=1024` - Limit V8 heap to 1GB
///
/// ## GPU and Rendering
/// - `--disable-gpu-compositing`
/// - `--disable-software-rasterizer`
/// - `--disable-accelerated-2d-canvas`
/// - `--disable-gl-drawing-for-tests`
/// - `--disable-webgl`
/// - `--disable-webgl2`
///
/// ## Disabled Features
/// - `--disable-extensions`
/// - `--disable-plugins`
/// - `--disable-sync`
/// - `--disable-default-apps`
///
/// ## Security and Automation
/// - `--disable-web-security` - Allow cross-origin requests (for scraping)
/// - `--enable-automation` - Mark as automated browser
///
/// ## Stability
/// - `--disable-background-timer-throttling`
/// - `--disable-backgrounding-occluded-windows`
/// - `--disable-hang-monitor`
/// - `--disable-popup-blocking`
/// - `--disable-renderer-backgrounding`
/// - `--disable-ipc-flooding-protection`
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::create_chrome_options;
///
/// // Auto-detect Chrome path
/// let options = create_chrome_options(None)?;
///
/// // Custom Chrome path
/// let options = create_chrome_options(Some("/usr/bin/chromium"))?;
/// ```
pub fn create_chrome_options(
    chrome_path: Option<&str>
) -> std::result::Result<LaunchOptions<'static>, Box<dyn std::error::Error + Send + Sync>> {

    match chrome_path {
        Some(path) => log::debug!(" Creating Chrome options with custom path: {}", path),
        None => log::debug!(" Creating Chrome options (auto-detect browser)"),
    }

    let mut builder = LaunchOptions::default_builder();

    // Set path if provided, otherwise let headless_chrome auto-detect
    if let Some(path) = chrome_path {
        builder.path(Some(path.to_string().into()));
        log::trace!(" Chrome path set to: {}", path);
    } else {
        log::trace!(" Chrome path: auto-detect");
    }

    // Configure launch options for stable headless operation
    builder
        .headless(true)  // Run in headless mode
        .sandbox(false)  // Disable sandbox (required in containers)
        .disable_default_args(true)  // Use our custom args only
        .args(vec![
            // ===== Memory and Performance Optimization =====
            "--disable-dev-shm-usage".as_ref(),  // Use /tmp instead of /dev/shm (container-friendly)
            "--disable-crash-reporter".as_ref(),  // No crash reporting
            "--max_old_space_size=1024".as_ref(),  // Limit V8 heap to 1GB

            // ===== GPU and Rendering Flags =====
            // Disable GPU features for headless stability
            "--disable-gpu-compositing".as_ref(),
            "--disable-software-rasterizer".as_ref(),
            "--disable-accelerated-2d-canvas".as_ref(),
            "--disable-gl-drawing-for-tests".as_ref(),
            "--disable-webgl".as_ref(),
            "--disable-webgl2".as_ref(),

            // ===== Disable Unnecessary Features =====
            "--disable-extensions".as_ref(),  // No browser extensions
            "--disable-plugins".as_ref(),  // No plugins
            "--disable-sync".as_ref(),  // No Chrome sync
            "--disable-default-apps".as_ref(),  // No default apps

            // ===== Security and Functionality =====
            "--disable-web-security".as_ref(),  // Allow cross-origin requests (for scraping)

            // ===== Automation and Debugging =====
            "--enable-automation".as_ref(),  // Mark as automated browser

            // ===== Stability and Performance =====
            "--disable-background-timer-throttling".as_ref(),  // Don't throttle background tabs
            "--disable-backgrounding-occluded-windows".as_ref(),  // Don't suspend hidden windows
            "--disable-hang-monitor".as_ref(),  // Disable hang detection

            // ===== UI Flags =====
            "--disable-popup-blocking".as_ref(),  // Allow popups

            // ===== Better CDP (Chrome DevTools Protocol) Stability =====
            "--disable-renderer-backgrounding".as_ref(),  // Don't deprioritize renderer
            "--disable-ipc-flooding-protection".as_ref(),  // Allow rapid IPC messages
        ])
        .build()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            let path_msg = chrome_path.unwrap_or("auto-detect");
            log::error!("❌ Failed to build Chrome launch options (path: {}): {}", path_msg, e);
            e.into()
        })
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that ChromeBrowserFactory can be instantiated.
    ///
    /// Tests that factory construction works with both auto-detect
    /// and custom path modes. Does not actually create browsers.
    #[test]
    fn test_chrome_factory_creation() {
        // Test auto-detect mode
        let _factory = ChromeBrowserFactory::with_defaults();

        // Test custom path mode
        let _factory_with_path = ChromeBrowserFactory::with_path("/custom/chrome/path".to_string());

        // If we got here without panicking, factory creation works
    }

    /// Verifies that Chrome launch options can be built.
    ///
    /// Tests the option builder for both auto-detect and custom path modes.
    /// This verifies the configuration is valid, but doesn't launch Chrome.
    #[test]
    fn test_create_chrome_options() {
        // Test with auto-detect (should build successfully)
        let result = create_chrome_options(None);
        assert!(
            result.is_ok(),
            "Auto-detect Chrome options should build successfully: {:?}",
            result.err()
        );

        // Test with custom path (should build successfully)
        let result = create_chrome_options(Some("/custom/chrome/path"));
        assert!(
            result.is_ok(),
            "Custom path Chrome options should build successfully: {:?}",
            result.err()
        );
    }
}