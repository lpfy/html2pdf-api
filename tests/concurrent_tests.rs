//! Concurrent access tests for the browser pool.

use html2pdf_api::prelude::*;
use std::sync::Arc;
use tokio::task::JoinSet;

/// Test concurrent access to pool stats.
#[tokio::test]
async fn test_concurrent_stats_access() {
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

    let shared_pool = Arc::new(std::sync::Mutex::new(pool));

    let mut tasks = JoinSet::new();

    // Spawn multiple tasks accessing stats concurrently
    for _ in 0..10 {
        let pool = Arc::clone(&shared_pool);
        tasks.spawn(async move {
            for _ in 0..100 {
                let pool_guard = pool.lock().unwrap();
                let _stats = pool_guard.stats();
            }
        });
    }

    // Wait for all tasks to complete
    while let Some(result) = tasks.join_next().await {
        assert!(result.is_ok(), "Task should complete without panic");
    }
}
