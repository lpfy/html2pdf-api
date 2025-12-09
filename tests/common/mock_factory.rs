//! Mock factory for integration tests.

// Re-export from the crate when test-utils feature is enabled
#[cfg(feature = "test-utils")]
pub use html2pdf_api::factory::mock::MockBrowserFactory;