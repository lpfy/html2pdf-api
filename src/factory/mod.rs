//! Browser factory implementations.
//!
//! This module provides the [`BrowserFactory`] trait and implementations
//! for creating browser instances.
//!
//! # Overview
//!
//! The factory pattern abstracts browser creation, allowing:
//! - Different browser implementations (Chrome, Chromium, etc.)
//! - Custom launch configurations
//! - Mock factories for testing
//!
//! # Available Factories
//!
//! | Factory | Description |
//! |---------|-------------|
//! | [`ChromeBrowserFactory`] | Creates Chrome/Chromium browsers |
//! | [`mock::MockBrowserFactory`] | For testing (feature-gated) |
//!
//! # Example
//!
//! ```rust,ignore
//! use html2pdf_api::{BrowserFactory, ChromeBrowserFactory};
//!
//! // Create factory with auto-detected Chrome
//! let factory = ChromeBrowserFactory::with_defaults();
//!
//! // Create a browser
//! let browser = factory.create()?;
//! ```
//!
//! # Custom Factory
//!
//! You can implement [`BrowserFactory`] for custom browser creation:
//!
//! ```rust,ignore
//! use html2pdf_api::{BrowserFactory, BrowserPoolError, Result};
//! use headless_chrome::Browser;
//!
//! struct MyCustomFactory {
//!     // your configuration
//! }
//!
//! impl BrowserFactory for MyCustomFactory {
//!     fn create(&self) -> Result<Browser> {
//!         // Your custom browser creation logic
//!         todo!()
//!     }
//! }
//! ```

mod chrome;

#[cfg(any(test, feature = "test-utils"))]
pub mod mock;

pub use chrome::{ChromeBrowserFactory, create_chrome_options};

use crate::error::Result;
use headless_chrome::Browser;

/// Trait for browser factory pattern.
///
/// Abstracts browser creation to allow different implementations
/// (Chrome, Firefox, mock browsers for testing, etc.)
///
/// # Thread Safety
///
/// This trait requires `Send + Sync` because factories are shared
/// across threads in the browser pool.
///
/// # Implementors
///
/// - [`ChromeBrowserFactory`] - Creates Chrome/Chromium browsers
/// - [`mock::MockBrowserFactory`] - For testing (when `test-utils` feature enabled)
///
/// # Example
///
/// ```rust,ignore
/// use html2pdf_api::{BrowserFactory, ChromeBrowserFactory};
///
/// fn use_factory(factory: &dyn BrowserFactory) {
///     match factory.create() {
///         Ok(browser) => println!("Browser created!"),
///         Err(e) => eprintln!("Failed: {}", e),
///     }
/// }
///
/// let factory = ChromeBrowserFactory::with_defaults();
/// use_factory(&factory);
/// ```
pub trait BrowserFactory: Send + Sync {
    /// Create a new browser instance.
    ///
    /// # Errors
    ///
    /// Returns error if browser creation fails:
    /// - [`BrowserPoolError::Configuration`](crate::BrowserPoolError::Configuration) -
    ///   Invalid launch options
    /// - [`BrowserPoolError::BrowserCreation`](crate::BrowserPoolError::BrowserCreation) -
    ///   Binary not found, launch fails, etc.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use html2pdf_api::{BrowserFactory, ChromeBrowserFactory};
    ///
    /// let factory = ChromeBrowserFactory::with_defaults();
    /// let browser = factory.create()?;
    /// // Use browser...
    /// ```
    fn create(&self) -> Result<Browser>;
}