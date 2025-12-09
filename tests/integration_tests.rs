//! Integration tests for the browser pool.

mod common;

use html2pdf_api::prelude::*;
use std::time::Duration;

/// Test that pool can be created with default configuration.
#[tokio::test]
async fn test_pool_creation() {
    let result = BrowserPool::builder()
        .config(
            BrowserPoolConfigBuilder::new()
                .max_pool_size(2)
                .warmup_count(0) // No warmup to avoid needing Chrome
                .build()
                .unwrap(),
        )
        .factory(Box::new(
            html2pdf_api::factory::mock::MockBrowserFactory::always_fails("Test mode"),
        ))
        .enable_keep_alive(false)
        .build();

    assert!(result.is_ok(), "Pool creation should succeed");
}

/// Test that pool stats work correctly.
#[tokio::test]
async fn test_pool_stats() {
    let pool = BrowserPool::builder()
        .config(
            BrowserPoolConfigBuilder::new()
                .max_pool_size(5)
                .warmup_count(0)
                .build()
                .unwrap(),
        )
        .factory(Box::new(
            html2pdf_api::factory::mock::MockBrowserFactory::always_fails("Test mode"),
        ))
        .enable_keep_alive(false)
        .build()
        .unwrap();

    let stats = pool.stats();

    assert_eq!(stats.available, 0);
    assert_eq!(stats.active, 0);
}

/// Test configuration validation.
#[test]
fn test_config_validation() {
    // Zero pool size should fail
    let result = BrowserPoolConfigBuilder::new().max_pool_size(0).build();
    assert!(result.is_err());

    // Warmup > pool size should fail
    let result = BrowserPoolConfigBuilder::new()
        .max_pool_size(3)
        .warmup_count(5)
        .build();
    assert!(result.is_err());

    // Valid config should succeed
    let result = BrowserPoolConfigBuilder::new()
        .max_pool_size(5)
        .warmup_count(3)
        .browser_ttl(Duration::from_secs(3600))
        .build();
    assert!(result.is_ok());
}

/// Test that shutdown prevents new operations.
#[tokio::test]
async fn test_shutdown_prevents_operations() {
    let mut pool = BrowserPool::builder()
        .config(
            BrowserPoolConfigBuilder::new()
                .max_pool_size(2)
                .warmup_count(0)
                .build()
                .unwrap(),
        )
        .factory(Box::new(
            html2pdf_api::factory::mock::MockBrowserFactory::always_fails("Test mode"),
        ))
        .enable_keep_alive(false)
        .build()
        .unwrap();

    // Shutdown the pool
    pool.shutdown();

    // Get should fail with ShuttingDown error
    let result = pool.get();
    assert!(matches!(result, Err(BrowserPoolError::ShuttingDown)));
}