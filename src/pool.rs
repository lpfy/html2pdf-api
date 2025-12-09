//! Browser pool with lifecycle management.
//!
//! This module provides [`BrowserPool`], the main entry point for managing
//! a pool of headless Chrome browsers with automatic lifecycle management.
//!
//! # Overview
//!
//! The browser pool provides:
//! - **Connection Pooling**: Reuses browser instances to avoid expensive startup costs
//! - **Health Monitoring**: Background thread continuously checks browser health
//! - **TTL Management**: Automatically retires old browsers and creates replacements
//! - **Race-Free Design**: Careful lock ordering prevents deadlocks
//! - **Graceful Shutdown**: Clean termination of all background tasks
//! - **RAII Pattern**: Automatic return of browsers to pool via Drop
//!
//! # Architecture
//!
//! ```text
//! BrowserPool
//!   ├─ BrowserPoolInner (shared state)
//!   │   ├─ available: Vec<TrackedBrowser>  (pooled, ready to use)
//!   │   ├─ active: HashMap<id, TrackedBrowser>  (in-use, tracked for health)
//!   │   └─ replacement_tasks: Vec<JoinHandle>  (async replacement creators)
//!   └─ keep_alive_handle: JoinHandle  (health monitoring thread)
//! ```
//!
//! # Critical Invariants
//!
//! 1. **Lock Order**: Always acquire `active` before `available` to prevent deadlocks
//! 2. **Shutdown Flag**: Check before all expensive operations
//! 3. **Health Checks**: Never hold locks during I/O operations
//!
//! # Example
//!
//! ```rust,no_run
//! use html2pdf_api::{BrowserPool, BrowserPoolConfigBuilder, ChromeBrowserFactory};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create pool
//!     let mut pool = BrowserPool::builder()
//!         .config(
//!             BrowserPoolConfigBuilder::new()
//!                 .max_pool_size(5)
//!                 .warmup_count(3)
//!                 .build()?
//!         )
//!         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
//!         .build()?;
//!
//!     // Warmup
//!     pool.warmup().await?;
//!
//!     // Use browsers
//!     {
//!         let browser = pool.get()?;
//!         let tab = browser.new_tab()?;
//!         // ... do work ...
//!     } // browser returned to pool automatically
//!
//!     // Shutdown
//!     pool.shutdown_async().await;
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::task::JoinHandle as TokioJoinHandle;

use crate::config::BrowserPoolConfig;
use crate::error::{BrowserPoolError, Result};
use crate::factory::BrowserFactory;
use crate::handle::BrowserHandle;
use crate::stats::PoolStats;
use crate::tracked::TrackedBrowser;

// ============================================================================
// BrowserPoolInner
// ============================================================================

/// Internal shared state for the browser pool.
///
/// This struct contains all shared state and is wrapped in Arc for thread-safe
/// sharing between the pool, handles, and background threads.
///
/// # Lock Ordering (CRITICAL)
///
/// Always acquire locks in this order to prevent deadlocks:
/// 1. `active` (browsers currently in use)
/// 2. `available` (browsers in pool ready for use)
///
/// Never hold locks during I/O operations or browser creation.
///
/// # Thread Safety
///
/// All fields are protected by appropriate synchronization primitives:
/// - `Mutex` for mutable collections
/// - `AtomicBool` for shutdown flag
/// - `Arc` for shared ownership
pub(crate) struct BrowserPoolInner {
    /// Configuration (immutable after creation).
    config: BrowserPoolConfig,

    /// Browsers available for checkout (not currently in use).
    ///
    /// Protected by Mutex. Browsers are moved from here when checked out
    /// and returned here when released (if pool not full).
    available: Mutex<Vec<TrackedBrowser>>,

    /// All browsers that exist (both pooled and checked out).
    ///
    /// Protected by Mutex. Used for health monitoring and lifecycle tracking.
    /// Maps browser ID -> TrackedBrowser for fast lookup.
    active: Mutex<HashMap<u64, TrackedBrowser>>,

    /// Factory for creating new browser instances.
    factory: Box<dyn BrowserFactory>,

    /// Atomic flag indicating shutdown in progress.
    ///
    /// Checked before expensive operations. Once set, no new operations start.
    shutting_down: AtomicBool,

    /// Background tasks creating replacement browsers.
    ///
    /// Tracked so we can abort them during shutdown.
    replacement_tasks: Mutex<Vec<TokioJoinHandle<()>>>,

    /// Handle to tokio runtime for spawning async tasks.
    ///
    /// Captured at creation time to allow spawning from any context.
    runtime_handle: tokio::runtime::Handle,

    /// Shutdown signaling mechanism for keep-alive thread.
    ///
    /// Tuple of (flag, condvar) allows immediate wake-up on shutdown
    /// instead of waiting for full ping_interval.
    shutdown_signal: Arc<(Mutex<bool>, Condvar)>,
}

impl BrowserPoolInner {
    /// Create a new browser pool inner state.
    ///
    /// # Parameters
    ///
    /// * `config` - Validated configuration.
    /// * `factory` - Browser factory for creating instances.
    ///
    /// # Panics
    ///
    /// Panics if called outside a tokio runtime context.
    pub(crate) fn new(config: BrowserPoolConfig, factory: Box<dyn BrowserFactory>) -> Arc<Self> {
        log::info!(
            " Initializing browser pool with capacity {}",
            config.max_pool_size
        );
        log::debug!(
            " Pool config: warmup={}, TTL={}s, ping_interval={}s",
            config.warmup_count,
            config.browser_ttl.as_secs(),
            config.ping_interval.as_secs()
        );

        // Capture runtime handle for spawning async tasks
        // This allows us to spawn from sync contexts (like Drop)
        let runtime_handle = tokio::runtime::Handle::current();

        Arc::new(Self {
            config,
            available: Mutex::new(Vec::new()),
            active: Mutex::new(HashMap::new()),
            factory,
            shutting_down: AtomicBool::new(false),
            replacement_tasks: Mutex::new(Vec::new()),
            runtime_handle,
            shutdown_signal: Arc::new((Mutex::new(false), Condvar::new())),
        })
    }

    /// Create a browser directly without using the pool.
    ///
    /// Used for:
    /// - Initial warmup
    /// - Replacing failed browsers
    /// - When pool is empty
    ///
    /// # Important
    ///
    /// Adds the browser to `active` tracking immediately for health monitoring.
    ///
    /// # Errors
    ///
    /// - Returns [`BrowserPoolError::ShuttingDown`] if pool is shutting down.
    /// - Returns [`BrowserPoolError::BrowserCreation`] if factory fails.
    pub(crate) fn create_browser_direct(&self) -> Result<TrackedBrowser> {
        // Early exit if shutting down (don't waste time creating browsers)
        if self.shutting_down.load(Ordering::Acquire) {
            log::debug!(" Skipping browser creation - pool is shutting down");
            return Err(BrowserPoolError::ShuttingDown);
        }

        log::debug!("️ Creating new browser directly via factory...");

        // Factory handles all Chrome launch complexity
        let browser = self.factory.create()?;

        // Wrap with tracking metadata
        let tracked = TrackedBrowser::new(browser)?;
        let id = tracked.id();

        // Add to active tracking immediately for health monitoring
        // This ensures keep-alive thread will monitor it
        if let Ok(mut active) = self.active.lock() {
            active.insert(id, tracked.clone());
            log::debug!(
                " Browser {} added to active tracking (total active: {})",
                id,
                active.len()
            );
        } else {
            log::warn!(
                "⚠️ Failed to add browser {} to active tracking (poisoned lock)",
                id
            );
        }

        log::info!("✅ Created new browser with ID {}", id);
        Ok(tracked)
    }

    /// Get a browser from pool or create a new one.
    ///
    /// # Algorithm
    ///
    /// 1. Loop through pooled browsers
    /// 2. **Grace Period Check**: Check if browser is within 30s of TTL.
    ///    - If near expiry: Skip (drop) it immediately.
    ///    - It remains in `active` tracking so the `keep_alive` thread handles standard retirement/replacement.
    /// 3. For valid browsers, perform detailed health check (without holding locks)
    /// 4. If healthy, return it
    /// 5. If unhealthy, remove from active tracking and try next
    /// 6. If pool empty or all skipped/unhealthy, create new browser
    ///
    /// # Critical: Lock-Free Health Checks
    ///
    /// Health checks are performed WITHOUT holding locks to avoid blocking
    /// other threads. This is why we use a loop pattern instead of iterator.
    ///
    /// # Returns
    ///
    /// [`BrowserHandle`] that auto-returns browser to pool when dropped.
    ///
    /// # Errors
    ///
    /// - Returns [`BrowserPoolError::ShuttingDown`] if pool is shutting down.
    /// - Returns [`BrowserPoolError::BrowserCreation`] if new browser creation fails.
    pub(crate) fn get_or_create_browser(self: &Arc<Self>) -> Result<BrowserHandle> {
        log::debug!(" Attempting to get browser from pool...");

        // Try to get from pool - LOOP pattern to avoid holding lock during health checks
        // This is critical for concurrency: we release the lock between attempts
        loop {
            // Acquire lock briefly to pop one browser
            let tracked_opt = {
                let mut available = self.available.lock().unwrap();
                let popped = available.pop();
                log::trace!(" Pool size after pop: {}", available.len());
                popped
            }; // Lock released here - critical for performance

            if let Some(tracked) = tracked_opt {
                // === LOGIC START: Grace Period Check ===
                let age = tracked.created_at().elapsed();
                let ttl = self.config.browser_ttl;

                // Safety margin matching your stagger interval
                let safety_margin = Duration::from_secs(30);

                // If browser is about to expire, don't use it.
                if age + safety_margin > ttl {
                    log::debug!(
                        "⏳ Browser {} is near expiry (Age: {}s, Margin: 30s), skipping.",
                        tracked.id(),
                        age.as_secs()
                    );

                    // CRITICAL: We do NOT remove/recreate here.
                    // By simply 'continuing', we drop this 'tracked' instance.
                    // 1. It is NOT returned to 'available' (so no user gets it).
                    // 2. It REMAINS in 'active' (so the keep_alive thread still tracks it).
                    // 3. The keep_alive thread will see it expire and handle standard cleanup/replacement.
                    continue;
                }
                // === LOGIC END: Grace Period Check ===

                log::debug!(" Testing browser {} from pool for health...", tracked.id());

                // Detailed health check WITHOUT holding any locks
                // This prevents blocking other threads during I/O
                match tracked.browser().new_tab() {
                    Ok(tab) => {
                        log::trace!(
                            "✅ Browser {} health check: new_tab() successful",
                            tracked.id()
                        );

                        // Test navigation capability (full health check)
                        match tab
                            .navigate_to("data:text/html,<html><body>Health check</body></html>")
                        {
                            Ok(_) => {
                                log::trace!(
                                    "✅ Browser {} health check: navigation successful",
                                    tracked.id()
                                );

                                // Test cleanup capability
                                match tab.close(true) {
                                    Ok(_) => {
                                        log::debug!(
                                            "✅ Browser {} passed full health check - ready for use",
                                            tracked.id()
                                        );

                                        // Get pool size for logging (brief lock)
                                        let pool_size = {
                                            let available = self.available.lock().unwrap();
                                            available.len()
                                        };

                                        log::info!(
                                            "♻️ Reusing healthy browser {} from pool (pool size: {})",
                                            tracked.id(),
                                            pool_size
                                        );

                                        // Return healthy browser wrapped in RAII handle
                                        return Ok(BrowserHandle::new(tracked, Arc::clone(self)));
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "❌ Browser {} health check: tab close failed: {}",
                                            tracked.id(),
                                            e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!(
                                    "❌ Browser {} health check: navigation failed: {}",
                                    tracked.id(),
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "❌ Browser {} health check: new_tab() failed: {}",
                            tracked.id(),
                            e
                        );
                    }
                }

                // If we reach here, health check failed
                // Remove from active tracking (browser is dead)
                log::warn!(
                    "️ Removing unhealthy browser {} from active tracking",
                    tracked.id()
                );
                {
                    let mut active = self.active.lock().unwrap();
                    active.remove(&tracked.id());
                    log::debug!(" Active browsers after removal: {}", active.len());
                }

                // Continue loop to try next browser in pool
                log::debug!(" Trying next browser from pool...");
            } else {
                // Pool is empty, break to create new browser
                log::debug!(" Pool is empty, will create new browser");
                break;
            }
        }

        // Pool is empty or no healthy browsers found
        log::info!("️ Creating new browser (pool was empty or all browsers unhealthy)");

        let tracked = self.create_browser_direct()?;

        log::info!("✅ Returning newly created browser {}", tracked.id());
        Ok(BrowserHandle::new(tracked, Arc::clone(self)))
    }

    /// Return a browser to the pool (called by BrowserHandle::drop).
    ///
    /// # Critical Lock Ordering
    ///
    /// Always acquires locks in order: active -> available.
    /// Both locks are held together to prevent race conditions.
    ///
    /// # Algorithm
    ///
    /// 1. Acquire both locks (order: active, then available)
    /// 2. Verify browser is in active tracking
    /// 3. Check TTL - if expired, retire and trigger replacement
    /// 4. If pool has space, add to available pool
    /// 5. If pool full, remove from active (browser gets dropped)
    ///
    /// # Parameters
    ///
    /// * `self_arc` - Arc reference to self (needed for spawning async tasks).
    /// * `tracked` - The browser being returned.
    pub(crate) fn return_browser(self_arc: &Arc<Self>, tracked: TrackedBrowser) {
        log::debug!(" Returning browser {} to pool...", tracked.id());

        // Early exit if shutting down (don't waste time managing pool)
        if self_arc.shutting_down.load(Ordering::Acquire) {
            log::debug!(
                " Pool shutting down, not returning browser {}",
                tracked.id()
            );
            return;
        }

        // CRITICAL: Always acquire in order: active -> pool
        // Holding both locks prevents ALL race conditions:
        // - Prevents concurrent modifications to browser state
        // - Prevents duplicate returns
        // - Ensures pool size limits are respected
        let mut active = self_arc.active.lock().unwrap();
        let mut pool = self_arc.available.lock().unwrap();

        // Verify browser is actually tracked (sanity check)
        if !active.contains_key(&tracked.id()) {
            log::warn!(
                "❌ Browser {} not in active tracking (probably already removed), skipping return",
                tracked.id()
            );
            return;
        }

        // Check TTL before returning to pool
        // Expired browsers should be retired to prevent memory leaks
        if tracked.is_expired(self_arc.config.browser_ttl) {
            log::info!(
                "⏰ Browser {} expired (age: {}min, TTL: {}min), retiring instead of returning",
                tracked.id(),
                tracked.age_minutes(),
                self_arc.config.browser_ttl.as_secs() / 60
            );

            // Remove from active tracking
            active.remove(&tracked.id());
            log::debug!(" Active browsers after TTL retirement: {}", active.len());

            // Release locks before spawning replacement task
            drop(active);
            drop(pool);

            // Trigger async replacement creation (non-blocking)
            log::debug!(" Triggering replacement browser creation for expired browser");
            Self::spawn_replacement_creation(Arc::clone(self_arc), 1);
            return;
        }

        // Prevent duplicate returns (defensive programming)
        if pool.iter().any(|b| b.id() == tracked.id()) {
            log::warn!(
                "⚠️ Browser {} already in pool (duplicate return attempt), skipping",
                tracked.id()
            );
            return;
        }

        // Check if pool has space for this browser
        if pool.len() < self_arc.config.max_pool_size {
            // Add to pool for reuse
            pool.push(tracked.clone());
            log::info!(
                "♻️ Browser {} returned to pool (pool size: {}/{})",
                tracked.id(),
                pool.len(),
                self_arc.config.max_pool_size
            );
        } else {
            // Pool is full, remove from tracking (browser will be dropped)
            log::debug!(
                "️ Pool full ({}/{}), removing browser {} from system",
                pool.len(),
                self_arc.config.max_pool_size,
                tracked.id()
            );
            active.remove(&tracked.id());
            log::debug!(" Active browsers after removal: {}", active.len());
        }
    }

    /// Asynchronously create replacement browsers (internal helper).
    ///
    /// This is the async work function that actually creates browsers.
    /// It's spawned as a tokio task by `spawn_replacement_creation`.
    ///
    /// # Algorithm
    ///
    /// 1. Check shutdown flag before each creation
    /// 2. Check pool space before each creation
    /// 3. Use spawn_blocking for CPU-bound browser creation
    /// 4. Add successful browsers to pool
    /// 5. Log detailed status
    ///
    /// # Parameters
    ///
    /// * `inner` - Arc reference to pool state.
    /// * `count` - Number of browsers to attempt to create.
    async fn spawn_replacement_creation_async(inner: Arc<Self>, count: usize) {
        log::info!(
            " Starting async replacement creation for {} browsers",
            count
        );

        let mut created_count = 0;
        let mut failed_count = 0;

        for i in 0..count {
            // Check shutdown flag before each expensive operation
            if inner.shutting_down.load(Ordering::Acquire) {
                log::info!(
                    " Shutdown detected during replacement creation, stopping at {}/{}",
                    i,
                    count
                );
                break;
            }

            // Check if pool has space BEFORE creating (avoid wasted work)
            let pool_has_space = {
                let pool = inner.available.lock().unwrap();
                let has_space = pool.len() < inner.config.max_pool_size;
                log::trace!(
                    " Pool space check: {}/{} (has space: {})",
                    pool.len(),
                    inner.config.max_pool_size,
                    has_space
                );
                has_space
            };

            if !pool_has_space {
                log::warn!(
                    "⚠️ Pool is full, stopping replacement creation at {}/{}",
                    i,
                    count
                );
                break;
            }

            log::debug!("️ Creating replacement browser {}/{}", i + 1, count);

            // Use spawn_blocking for CPU-bound browser creation
            // This prevents blocking the async runtime
            let inner_clone = Arc::clone(&inner);
            let result =
                tokio::task::spawn_blocking(move || inner_clone.create_browser_direct()).await;

            match result {
                Ok(Ok(tracked)) => {
                    let id = tracked.id();

                    // Add to pool (with space check to handle race conditions)
                    let mut pool = inner.available.lock().unwrap();

                    // Double-check space (another thread might have added browsers)
                    if pool.len() < inner.config.max_pool_size {
                        pool.push(tracked);
                        created_count += 1;
                        log::info!(
                            "✅ Created replacement browser {} and added to pool ({}/{})",
                            id,
                            i + 1,
                            count
                        );
                    } else {
                        log::warn!(
                            "⚠️ Pool became full during creation, replacement browser {} kept in active only",
                            id
                        );
                        created_count += 1; // Still count as created (just not pooled)
                    }
                }
                Ok(Err(e)) => {
                    failed_count += 1;
                    log::error!(
                        "❌ Failed to create replacement browser {}/{}: {}",
                        i + 1,
                        count,
                        e
                    );
                }
                Err(e) => {
                    failed_count += 1;
                    log::error!(
                        "❌ Replacement browser {}/{} task panicked: {:?}",
                        i + 1,
                        count,
                        e
                    );
                }
            }
        }

        // Final status report
        let pool_size = inner.available.lock().unwrap().len();
        let active_size = inner.active.lock().unwrap().len();

        log::info!(
            " Replacement creation completed: {}/{} created, {} failed. Pool: {}, Active: {}",
            created_count,
            count,
            failed_count,
            pool_size,
            active_size
        );
    }

    /// Spawn a background task to create replacement browsers.
    ///
    /// This is non-blocking and returns immediately. The actual browser
    /// creation happens in a tokio task tracked in `replacement_tasks`.
    ///
    /// # Why Async
    ///
    /// Browser creation is slow (1-3 seconds per browser). Spawning async
    /// tasks prevents blocking the caller.
    ///
    /// # Task Tracking
    ///
    /// Tasks are tracked so we can abort them during shutdown.
    ///
    /// # Parameters
    ///
    /// * `inner` - Arc reference to pool state.
    /// * `count` - Number of replacement browsers to create.
    pub(crate) fn spawn_replacement_creation(inner: Arc<Self>, count: usize) {
        log::info!(
            " Spawning async task to create {} replacement browsers",
            count
        );

        // Clone Arc for moving into async task
        let inner_for_task = Arc::clone(&inner);

        // Spawn async task on the captured runtime
        let task_handle = inner.runtime_handle.spawn(async move {
            Self::spawn_replacement_creation_async(inner_for_task, count).await;
        });

        // Track task handle for shutdown cleanup
        if let Ok(mut tasks) = inner.replacement_tasks.lock() {
            // Clean up finished tasks while we have the lock (housekeeping)
            let original_count = tasks.len();
            tasks.retain(|h| !h.is_finished());
            let cleaned = original_count - tasks.len();

            if cleaned > 0 {
                log::trace!("粒 Cleaned up {} finished replacement tasks", cleaned);
            }

            // Add new task
            tasks.push(task_handle);

            log::debug!(" Now tracking {} active replacement tasks", tasks.len());
        } else {
            log::warn!("⚠️ Failed to track replacement task (poisoned lock)");
        }
    }

    /// Get the pool configuration.
    #[inline]
    pub(crate) fn config(&self) -> &BrowserPoolConfig {
        &self.config
    }

    /// Check if the pool is shutting down.
    #[inline]
    pub(crate) fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    /// Set the shutdown flag.
    #[inline]
    pub(crate) fn set_shutting_down(&self, value: bool) {
        self.shutting_down.store(value, Ordering::Release);
    }

    /// Get the shutdown signal for the keep-alive thread.
    #[inline]
    pub(crate) fn shutdown_signal(&self) -> &Arc<(Mutex<bool>, Condvar)> {
        &self.shutdown_signal
    }

    /// Get the available browsers count.
    pub(crate) fn available_count(&self) -> usize {
        self.available.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Get the active browsers count.
    pub(crate) fn active_count(&self) -> usize {
        self.active.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Get a snapshot of active browsers for health checking.
    ///
    /// Returns a cloned list to avoid holding locks during I/O.
    pub(crate) fn get_active_browsers_snapshot(&self) -> Vec<(u64, TrackedBrowser)> {
        let active = self.active.lock().unwrap();
        active
            .iter()
            .map(|(id, tracked)| (*id, tracked.clone()))
            .collect()
    }

    /// Remove a browser from active tracking.
    pub(crate) fn remove_from_active(&self, id: u64) -> Option<TrackedBrowser> {
        let mut active = self.active.lock().unwrap();
        active.remove(&id)
    }

    /// Remove browsers from the available pool by ID.
    pub(crate) fn remove_from_available(&self, ids: &[u64]) {
        let mut pool = self.available.lock().unwrap();
        let original_size = pool.len();
        pool.retain(|b| !ids.contains(&b.id()));
        let removed = original_size - pool.len();
        if removed > 0 {
            log::debug!("️ Removed {} browsers from available pool", removed);
        }
    }

    /// Abort all replacement tasks.
    pub(crate) fn abort_replacement_tasks(&self) -> usize {
        if let Ok(mut tasks) = self.replacement_tasks.lock() {
            let count = tasks.len();
            for handle in tasks.drain(..) {
                handle.abort();
            }
            count
        } else {
            0
        }
    }
}

// ============================================================================
// BrowserPool
// ============================================================================

/// Main browser pool with lifecycle management.
///
/// This is the public-facing API for the browser pool. It wraps the internal
/// state and manages the keep-alive thread.
///
/// # Overview
///
/// `BrowserPool` provides:
/// - Browser checkout via [`get()`](Self::get)
/// - Pool warmup via [`warmup()`](Self::warmup)
/// - Statistics via [`stats()`](Self::stats)
/// - Graceful shutdown via [`shutdown_async()`](Self::shutdown_async)
///
/// # Example
///
/// ```rust,no_run
/// use html2pdf_api::{BrowserPool, BrowserPoolConfigBuilder, ChromeBrowserFactory};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create pool
///     let mut pool = BrowserPool::builder()
///         .config(
///             BrowserPoolConfigBuilder::new()
///                 .max_pool_size(5)
///                 .warmup_count(3)
///                 .build()?
///         )
///         .factory(Box::new(ChromeBrowserFactory::with_defaults()))
///         .build()?;
///
///     // Warmup
///     pool.warmup().await?;
///
///     // Use browsers
///     {
///         let browser = pool.get()?;
///         let tab = browser.new_tab()?;
///         // ... do work ...
///     } // browser returned to pool automatically
///
///     // Shutdown
///     pool.shutdown_async().await;
///
///     Ok(())
/// }
/// ```
///
/// # Thread Safety
///
/// `BrowserPool` is `Send` and can be wrapped in `Arc<Mutex<>>` for sharing
/// across threads. Use [`into_shared()`](Self::into_shared) for convenience.
pub struct BrowserPool {
    /// Shared internal state.
    inner: Arc<BrowserPoolInner>,

    /// Handle to keep-alive monitoring thread.
    ///
    /// Option allows taking during shutdown. None means keep-alive disabled.
    keep_alive_handle: Option<JoinHandle<()>>,
}

impl BrowserPool {
    /// Convert pool into a shared `Arc<Mutex<>>` for use in web handlers.
    ///
    /// This is convenient for web frameworks that need shared state.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?
    ///     .into_shared();
    ///
    /// // Can now be cloned and shared across handlers
    /// let pool_clone = Arc::clone(&pool);
    /// ```
    pub fn into_shared(self) -> Arc<Mutex<BrowserPool>> {
        log::debug!(" Converting BrowserPool into shared Arc<Mutex<>>");
        Arc::new(Mutex::new(self))
    }

    /// Create a new builder for constructing a BrowserPool.
    ///
    /// This is the recommended way to create a pool.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    /// ```
    pub fn builder() -> BrowserPoolBuilder {
        BrowserPoolBuilder::new()
    }

    /// Get a browser from the pool (or create one if empty).
    ///
    /// Returns a [`BrowserHandle`] that implements `Deref<Target=Browser>`,
    /// allowing transparent access to browser methods.
    ///
    /// # Automatic Return
    ///
    /// The browser is automatically returned to the pool when the handle
    /// is dropped, even if your code panics (RAII pattern).
    ///
    /// # Errors
    ///
    /// - Returns [`BrowserPoolError::ShuttingDown`] if pool is shutting down.
    /// - Returns [`BrowserPoolError::BrowserCreation`] if new browser creation fails.
    /// - Returns [`BrowserPoolError::HealthCheckFailed`] if all pooled browsers are unhealthy.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let browser = pool.get()?;
    /// let tab = browser.new_tab()?;
    /// tab.navigate_to("https://example.com")?;
    /// // browser returned automatically when it goes out of scope
    /// ```
    pub fn get(&self) -> Result<BrowserHandle> {
        log::trace!(" BrowserPool::get() called");
        self.inner.get_or_create_browser()
    }

    /// Get pool statistics snapshot.
    ///
    /// # Returns
    ///
    /// [`PoolStats`] containing:
    /// - `available`: Browsers in pool ready for checkout
    /// - `active`: All browsers (pooled + checked out)
    /// - `total`: Currently same as `active` (for future expansion)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let stats = pool.stats();
    /// println!("Available: {}, Active: {}", stats.available, stats.active);
    /// ```
    pub fn stats(&self) -> PoolStats {
        let available = self.inner.available_count();
        let active = self.inner.active_count();

        log::trace!(" Pool stats: available={}, active={}", available, active);

        PoolStats {
            available,
            active,
            total: active,
        }
    }

    /// Warmup the pool by pre-creating browsers.
    ///
    /// This is highly recommended to reduce first-request latency.
    /// Should be called during application startup.
    ///
    /// # Process
    ///
    /// 1. Creates `warmup_count` browsers sequentially with staggered timing
    /// 2. Tests each browser with navigation
    /// 3. Returns all browsers to pool
    /// 4. Entire process has timeout (configurable via `warmup_timeout`)
    ///
    /// # Staggered Creation
    ///
    /// Browsers are created with a 30-second delay between them to ensure
    /// their TTLs are offset. This prevents all browsers from expiring
    /// at the same time.
    ///
    /// # Errors
    ///
    /// - Returns error if warmup times out.
    /// - Returns error if browser creation fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    ///
    /// // Warmup during startup
    /// pool.warmup().await?;
    /// ```
    pub async fn warmup(&self) -> Result<()> {
        let count = self.inner.config().warmup_count;
        let warmup_timeout = self.inner.config().warmup_timeout;

        log::info!(
            " Starting browser pool warmup with {} instances (timeout: {}s)",
            count,
            warmup_timeout.as_secs()
        );

        // Wrap entire warmup in timeout to prevent hanging forever
        let warmup_result = tokio::time::timeout(warmup_timeout, self.warmup_internal(count)).await;

        match warmup_result {
            Ok(Ok(())) => {
                let stats = self.stats();
                log::info!(
                    "✅ Warmup completed successfully - Available: {}, Active: {}",
                    stats.available,
                    stats.active
                );
                Ok(())
            }
            Ok(Err(e)) => {
                log::error!("❌ Warmup failed with error: {}", e);
                Err(e)
            }
            Err(_) => {
                log::error!("❌ Warmup timed out after {}s", warmup_timeout.as_secs());
                Err(BrowserPoolError::Configuration(format!(
                    "Warmup timed out after {}s",
                    warmup_timeout.as_secs()
                )))
            }
        }
    }

    /// Internal warmup implementation (separated for cleaner timeout wrapping).
    ///
    /// Creates browsers sequentially with a delay between them.
    /// This ensures they don't all reach their TTL (expiration) at the exact same moment.
    async fn warmup_internal(&self, count: usize) -> Result<()> {
        log::debug!(" Starting internal warmup process for {} browsers", count);

        // STAGGER CONFIGURATION
        // We wait this long between creations to distribute expiration times
        let stagger_interval = Duration::from_secs(30);

        let mut handles = Vec::new();
        let mut created_count = 0;
        let mut failed_count = 0;

        for i in 0..count {
            log::debug!(" Creating startup browser instance {}/{}", i + 1, count);

            // Per-browser timeout (15s per browser is reasonable)
            // This prevents one slow browser from blocking entire warmup
            let browser_result = tokio::time::timeout(
                Duration::from_secs(15),
                tokio::task::spawn_blocking({
                    let inner = Arc::clone(&self.inner);
                    move || inner.create_browser_direct()
                }),
            )
            .await;

            match browser_result {
                Ok(Ok(Ok(tracked))) => {
                    log::debug!(
                        "✅ Browser {} created, performing validation test...",
                        tracked.id()
                    );

                    // Test the browser with actual navigation
                    match tracked.browser().new_tab() {
                        Ok(tab) => {
                            log::trace!("✅ Browser {} test: new_tab() successful", tracked.id());

                            // Navigate to test page
                            let nav_result = tab.navigate_to(
                                "data:text/html,<html><body>Warmup test</body></html>",
                            );
                            if let Err(e) = nav_result {
                                log::warn!(
                                    "⚠️ Browser {} test navigation failed: {}",
                                    tracked.id(),
                                    e
                                );
                            } else {
                                log::trace!(
                                    "✅ Browser {} test: navigation successful",
                                    tracked.id()
                                );
                            }

                            // Clean up test tab
                            let _ = tab.close(true);

                            // Keep handle so browser stays alive
                            handles.push(BrowserHandle::new(tracked, Arc::clone(&self.inner)));

                            created_count += 1;
                            log::info!(
                                "✅ Browser instance {}/{} ready and validated",
                                i + 1,
                                count
                            );
                        }
                        Err(e) => {
                            failed_count += 1;
                            log::error!(
                                "❌ Browser {} validation test failed: {}",
                                tracked.id(),
                                e
                            );

                            // Remove from active tracking since it's broken
                            self.inner.remove_from_active(tracked.id());
                        }
                    }
                }
                Ok(Ok(Err(e))) => {
                    failed_count += 1;
                    log::error!("❌ Failed to create browser {}/{}: {}", i + 1, count, e);
                }
                Ok(Err(e)) => {
                    failed_count += 1;
                    log::error!(
                        "❌ Browser {}/{} creation task panicked: {:?}",
                        i + 1,
                        count,
                        e
                    );
                }
                Err(_) => {
                    failed_count += 1;
                    log::error!(
                        "❌ Browser {}/{} creation timed out (15s limit)",
                        i + 1,
                        count
                    );
                }
            }

            // === STAGGER LOGIC ===
            // If this is not the last browser, wait before creating the next one.
            // This ensures their TTLs are offset by `stagger_interval`.
            if i < count - 1 {
                log::info!(
                    "⏳ Waiting {}s before creating next warmup browser to stagger TTLs...",
                    stagger_interval.as_secs()
                );
                tokio::time::sleep(stagger_interval).await;
            }
        }

        log::info!(
            " Warmup creation phase: {} created, {} failed",
            created_count,
            failed_count
        );

        // Return all browsers to pool by dropping handles
        log::debug!(" Returning {} warmup browsers to pool...", handles.len());
        drop(handles);

        // Small delay to ensure Drop handlers complete
        tokio::time::sleep(Duration::from_millis(300)).await;

        let final_stats = self.stats();
        log::info!(
            " Warmup internal completed - Pool: {}, Active: {}",
            final_stats.available,
            final_stats.active
        );

        Ok(())
    }

    /// Start the keep-alive monitoring thread.
    ///
    /// This background thread:
    /// - Pings all active browsers periodically
    /// - Removes unresponsive browsers after max_ping_failures
    /// - Retires browsers that exceed TTL
    /// - Spawns replacement browsers as needed
    ///
    /// # Critical Design Notes
    ///
    /// - Uses condvar for immediate shutdown signaling
    /// - Never holds locks during I/O operations
    /// - Uses consistent lock ordering (active -> pool)
    ///
    /// # Parameters
    ///
    /// * `inner` - Arc reference to pool state.
    ///
    /// # Returns
    ///
    /// JoinHandle for the background thread.
    fn start_keep_alive(inner: Arc<BrowserPoolInner>) -> JoinHandle<()> {
        let ping_interval = inner.config().ping_interval;
        let max_failures = inner.config().max_ping_failures;
        let browser_ttl = inner.config().browser_ttl;
        let shutdown_signal = Arc::clone(inner.shutdown_signal());

        log::info!(
            " Starting keep-alive thread (interval: {}s, max failures: {}, TTL: {}min)",
            ping_interval.as_secs(),
            max_failures,
            browser_ttl.as_secs() / 60
        );

        thread::spawn(move || {
            log::info!(" Keep-alive thread started successfully");

            // Track consecutive failures per browser ID
            let mut failure_counts: HashMap<u64, u32> = HashMap::new();

            loop {
                // Wait for next ping interval OR shutdown signal (whichever comes first)
                // Using condvar instead of sleep allows immediate wake-up on shutdown
                let (lock, cvar) = &*shutdown_signal;
                let wait_result = {
                    let shutdown = lock.lock().unwrap();
                    cvar.wait_timeout(shutdown, ping_interval).unwrap()
                };

                let shutdown_flag = *wait_result.0;
                let timed_out = wait_result.1.timed_out();

                // Check if we were signaled to shutdown
                if shutdown_flag {
                    log::info!(" Keep-alive received shutdown signal via condvar");
                    break;
                }

                // Double-check atomic shutdown flag (belt and suspenders)
                if inner.is_shutting_down() {
                    log::info!(" Keep-alive detected shutdown via atomic flag");
                    break;
                }

                // If spuriously woken (not timeout, not shutdown), continue waiting
                if !timed_out {
                    log::trace!("⏰ Keep-alive spuriously woken, continuing wait...");
                    continue;
                }

                log::trace!(" Keep-alive ping cycle starting...");

                // Collect browsers to ping WITHOUT holding locks
                // This is critical: we clone the list and release the lock
                // before doing any I/O operations
                let browsers_to_ping = inner.get_active_browsers_snapshot();
                log::trace!(
                    "Keep-alive checking {} active browsers",
                    browsers_to_ping.len()
                );

                // Now ping browsers without holding any locks
                let mut to_remove = Vec::new();
                let mut expired_browsers = Vec::new();

                for (id, tracked) in browsers_to_ping {
                    // Check shutdown during ping loop (allows early exit)
                    if inner.is_shutting_down() {
                        log::info!("Shutdown detected during ping loop, exiting immediately");
                        return;
                    }

                    // Check TTL before pinging (no point pinging expired browsers)
                    if tracked.is_expired(browser_ttl) {
                        log::info!(
                            "Browser {} expired (age: {}min, TTL: {}min), marking for retirement",
                            id,
                            tracked.age_minutes(),
                            browser_ttl.as_secs() / 60
                        );
                        expired_browsers.push(id);
                        continue; // Skip ping for expired browsers
                    }

                    // Perform health check (this is I/O, no locks held)
                    use crate::traits::Healthcheck;
                    match tracked.ping() {
                        Ok(_) => {
                            // Reset failure count on success
                            if failure_counts.remove(&id).is_some() {
                                log::debug!("Browser {} ping successful, failure count reset", id);
                            }
                        }
                        Err(e) => {
                            // Only process failures if NOT shutting down
                            // (during shutdown, browsers may legitimately fail)
                            if !inner.is_shutting_down() {
                                let failures = failure_counts.entry(id).or_insert(0);
                                *failures += 1;

                                log::warn!(
                                    "Browser {} ping failed (attempt {}/{}): {}",
                                    id,
                                    failures,
                                    max_failures,
                                    e
                                );

                                // Remove if exceeded max failures
                                if *failures >= max_failures {
                                    log::error!(
                                        "Browser {} exceeded max ping failures ({}), marking for removal",
                                        id,
                                        max_failures
                                    );
                                    to_remove.push(id);
                                }
                            }
                        }
                    }
                }

                // Check shutdown before cleanup (avoid work if shutting down)
                if inner.is_shutting_down() {
                    log::info!("Shutdown detected before cleanup, skipping and exiting");
                    break;
                }

                // Handle TTL retirements first (they need replacement browsers)
                if !expired_browsers.is_empty() {
                    log::info!("Processing {} TTL-expired browsers", expired_browsers.len());
                    Self::handle_browser_retirement(&inner, expired_browsers, &mut failure_counts);
                }

                // Handle failed browsers (remove from tracking and pool)
                if !to_remove.is_empty() {
                    log::warn!("Removing {} failed browsers from pool", to_remove.len());

                    // Track how many were actually removed so we know how many to replace
                    let mut actual_removed_count = 0;

                    // Remove dead browsers from active tracking
                    for id in &to_remove {
                        if inner.remove_from_active(*id).is_some() {
                            actual_removed_count += 1;
                            log::debug!("Removed failed browser {} from active tracking", id);
                        }
                        failure_counts.remove(id);
                    }

                    log::debug!(
                        "Active browsers after failure cleanup: {}",
                        inner.active_count()
                    );

                    // Clean up pool (remove dead browsers)
                    inner.remove_from_available(&to_remove);

                    log::debug!("Pool size after cleanup: {}", inner.available_count());

                    // Trigger replacement for the browsers we just removed
                    if actual_removed_count > 0 {
                        log::info!(
                            "Spawning {} replacement browsers for failed ones",
                            actual_removed_count
                        );
                        BrowserPoolInner::spawn_replacement_creation(
                            Arc::clone(&inner),
                            actual_removed_count,
                        );
                    }
                }

                // Log keep-alive cycle summary
                log::debug!(
                    "Keep-alive cycle complete - Active: {}, Pooled: {}, Tracking {} failure states",
                    inner.active_count(),
                    inner.available_count(),
                    failure_counts.len()
                );
            }

            log::info!("Keep-alive thread exiting cleanly");
        })
    }

    /// Handle browser retirement due to TTL expiration.
    ///
    /// This function:
    /// 1. Removes expired browsers from active and pool tracking
    /// 2. Spawns async tasks to create replacement browsers
    /// 3. Maintains pool target size
    ///
    /// # Critical Lock Ordering
    ///
    /// Acquires active -> pool locks together to prevent races.
    ///
    /// # Parameters
    ///
    /// * `inner` - Arc reference to pool state.
    /// * `expired_ids` - List of browser IDs that have exceeded TTL.
    /// * `failure_counts` - Mutable map of failure counts (updated to remove retired browsers).
    fn handle_browser_retirement(
        inner: &Arc<BrowserPoolInner>,
        expired_ids: Vec<u64>,
        failure_counts: &mut HashMap<u64, u32>,
    ) {
        log::info!(
            "Retiring {} expired browsers (TTL enforcement)",
            expired_ids.len()
        );

        // Remove expired browsers from active tracking
        let mut retired_count = 0;
        for id in &expired_ids {
            if inner.remove_from_active(*id).is_some() {
                retired_count += 1;
                log::debug!("Removed expired browser {} from active tracking", id);
            }
            // Clean up failure tracking
            failure_counts.remove(id);
        }

        // Remove from pool as well
        inner.remove_from_available(&expired_ids);

        log::debug!(
            "After retirement - Active: {}, Pooled: {}",
            inner.active_count(),
            inner.available_count()
        );

        // Create replacement browsers to maintain target count
        if retired_count > 0 {
            log::info!(
                "Spawning {} replacement browsers for retired ones",
                retired_count
            );
            BrowserPoolInner::spawn_replacement_creation(Arc::clone(inner), retired_count);
        } else {
            log::debug!("No browsers were actually retired (already removed)");
        }
    }

    /// Asynchronously shutdown the pool (recommended method).
    ///
    /// This is the preferred shutdown method as it can properly await
    /// async task cancellation. Should be called during application shutdown.
    ///
    /// # Shutdown Process
    ///
    /// 1. Set atomic shutdown flag (stops new operations)
    /// 2. Signal condvar to wake keep-alive thread immediately
    /// 3. Wait for keep-alive thread to exit (with timeout)
    /// 4. Abort all replacement creation tasks
    /// 5. Wait briefly for cleanup
    /// 6. Log final statistics
    ///
    /// # Timeout
    ///
    /// Keep-alive thread is given 5 seconds to exit gracefully.
    /// If it doesn't exit, we log an error but continue shutdown.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut pool = /* ... */;
    ///
    /// // During application shutdown
    /// pool.shutdown_async().await;
    /// ```
    pub async fn shutdown_async(&mut self) {
        log::info!("Shutting down browser pool (async mode)...");

        // Step 1: Set shutdown flag (prevents new operations)
        self.inner.set_shutting_down(true);
        log::debug!("Shutdown flag set");

        // Step 2: Signal condvar to wake keep-alive thread immediately
        // This is critical - without this, keep-alive waits for full ping_interval
        {
            let (lock, cvar) = &**self.inner.shutdown_signal();
            let mut shutdown = lock.lock().unwrap();
            *shutdown = true;
            cvar.notify_all();
            log::debug!("Shutdown signal sent to keep-alive thread");
        } // Lock released here

        // Step 3: Wait for keep-alive thread to exit
        if let Some(handle) = self.keep_alive_handle.take() {
            log::debug!("Waiting for keep-alive thread to exit...");

            // Wrap thread join in spawn_blocking to make it async-friendly
            let join_task = tokio::task::spawn_blocking(move || handle.join());

            // Give it 5 seconds to exit gracefully
            match tokio::time::timeout(Duration::from_secs(5), join_task).await {
                Ok(Ok(Ok(_))) => {
                    log::info!("Keep-alive thread stopped cleanly");
                }
                Ok(Ok(Err(_))) => {
                    log::error!("Keep-alive thread panicked during shutdown");
                }
                Ok(Err(_)) => {
                    log::error!("Keep-alive join task panicked");
                }
                Err(_) => {
                    log::error!("Keep-alive thread didn't exit within 5s timeout");
                }
            }
        } else {
            log::debug!("No keep-alive thread to stop (was disabled or already stopped)");
        }

        // Step 4: Abort all replacement creation tasks
        log::info!("Aborting replacement creation tasks...");
        let aborted_count = self.inner.abort_replacement_tasks();
        if aborted_count > 0 {
            log::info!("Aborted {} replacement tasks", aborted_count);
        } else {
            log::debug!("No replacement tasks to abort");
        }

        // Step 5: Small delay to let aborted tasks clean up
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Step 6: Log final statistics
        let stats = self.stats();
        log::info!(
            "Async shutdown complete - Available: {}, Active: {}, Total: {}",
            stats.available,
            stats.active,
            stats.total
        );
    }

    /// Synchronously shutdown the pool (fallback method).
    ///
    /// This is a simplified shutdown for use in Drop or non-async contexts.
    /// Prefer [`shutdown_async()`](Self::shutdown_async) when possible for cleaner task cancellation.
    ///
    /// # Note
    ///
    /// This method doesn't wait for replacement tasks to finish since
    /// there's no async runtime available. Tasks are aborted but may not
    /// have cleaned up yet.
    pub fn shutdown(&mut self) {
        log::debug!("Calling synchronous shutdown...");
        self.shutdown_sync();
    }

    /// Internal synchronous shutdown implementation.
    fn shutdown_sync(&mut self) {
        log::info!("Shutting down browser pool (sync mode)...");

        // Set shutdown flag
        self.inner.set_shutting_down(true);
        log::debug!("Shutdown flag set");

        // Signal condvar (same as async version)
        {
            let (lock, cvar) = &**self.inner.shutdown_signal();
            let mut shutdown = lock.lock().unwrap();
            *shutdown = true;
            cvar.notify_all();
            log::debug!("Shutdown signal sent");
        }

        // Wait for keep-alive thread
        if let Some(handle) = self.keep_alive_handle.take() {
            log::debug!("Joining keep-alive thread (sync)...");

            match handle.join() {
                Ok(_) => log::info!("Keep-alive thread stopped"),
                Err(_) => log::error!("Keep-alive thread panicked"),
            }
        }

        // Abort replacement tasks (best effort - they won't make progress without runtime)
        let aborted_count = self.inner.abort_replacement_tasks();
        if aborted_count > 0 {
            log::debug!("Aborted {} replacement tasks (sync mode)", aborted_count);
        }

        let stats = self.stats();
        log::info!(
            "Sync shutdown complete - Available: {}, Active: {}",
            stats.available,
            stats.active
        );
    }

    /// Get a reference to the inner pool state.
    ///
    /// This is primarily for internal use and testing.
    #[doc(hidden)]
    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> &Arc<BrowserPoolInner> {
        &self.inner
    }
}

impl Drop for BrowserPool {
    /// Automatic cleanup when pool is dropped.
    ///
    /// This ensures resources are released even if shutdown wasn't called explicitly.
    /// Uses sync shutdown since Drop can't be async.
    fn drop(&mut self) {
        log::debug!("� BrowserPool Drop triggered - running cleanup");

        // Only shutdown if not already done
        if !self.inner.is_shutting_down() {
            log::warn!("� BrowserPool dropped without explicit shutdown - cleaning up");
            self.shutdown();
        } else {
            log::debug!(" Pool already shutdown, Drop is no-op");
        }
    }
}

// ============================================================================
// BrowserPoolBuilder
// ============================================================================

/// Builder for constructing a [`BrowserPool`] with validation.
///
/// This is the recommended way to create a pool as it validates
/// configuration and provides sensible defaults.
///
/// # Example
///
/// ```rust,ignore
/// use std::time::Duration;
/// use html2pdf_api::{BrowserPool, BrowserPoolConfigBuilder, ChromeBrowserFactory};
///
/// let pool = BrowserPool::builder()
///     .config(
///         BrowserPoolConfigBuilder::new()
///             .max_pool_size(10)
///             .warmup_count(5)
///             .browser_ttl(Duration::from_secs(7200))
///             .build()?
///     )
///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
///     .enable_keep_alive(true)
///     .build()?;
/// ```
pub struct BrowserPoolBuilder {
    /// Optional configuration (uses default if not provided).
    config: Option<BrowserPoolConfig>,

    /// Browser factory (required).
    factory: Option<Box<dyn BrowserFactory>>,

    /// Whether to enable keep-alive thread (default: true).
    enable_keep_alive: bool,
}

impl BrowserPoolBuilder {
    /// Create a new builder with defaults.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = BrowserPoolBuilder::new();
    /// ```
    pub fn new() -> Self {
        Self {
            config: None,
            factory: None,
            enable_keep_alive: true,
        }
    }

    /// Set custom configuration.
    ///
    /// If not called, uses [`BrowserPoolConfig::default()`].
    ///
    /// # Parameters
    ///
    /// * `config` - Validated configuration from [`crate::BrowserPoolConfigBuilder`].
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = BrowserPoolConfigBuilder::new()
    ///     .max_pool_size(10)
    ///     .build()?;
    ///
    /// let pool = BrowserPool::builder()
    ///     .config(config)
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    /// ```
    pub fn config(mut self, config: BrowserPoolConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set browser factory (required).
    ///
    /// The factory is responsible for creating browser instances.
    /// Use [`ChromeBrowserFactory`](crate::ChromeBrowserFactory) for Chrome/Chromium browsers.
    ///
    /// # Parameters
    ///
    /// * `factory` - A boxed [`BrowserFactory`] implementation.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    /// ```
    pub fn factory(mut self, factory: Box<dyn BrowserFactory>) -> Self {
        self.factory = Some(factory);
        self
    }

    /// Enable or disable keep-alive thread.
    ///
    /// Keep-alive should be disabled only for testing.
    /// Production use should always have it enabled.
    ///
    /// # Parameters
    ///
    /// * `enable` - Whether to enable the keep-alive thread.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Disable for tests
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .enable_keep_alive(false)
    ///     .build()?;
    /// ```
    pub fn enable_keep_alive(mut self, enable: bool) -> Self {
        self.enable_keep_alive = enable;
        self
    }

    /// Build the browser pool.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserPoolError::Configuration`] if factory is not provided.
    ///
    /// # Panics
    ///
    /// Panics if called outside a tokio runtime context.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BrowserPool::builder()
    ///     .factory(Box::new(ChromeBrowserFactory::with_defaults()))
    ///     .build()?;
    /// ```
    pub fn build(self) -> Result<BrowserPool> {
        let config = self.config.unwrap_or_default();
        let factory = self.factory.ok_or_else(|| {
            BrowserPoolError::Configuration("No browser factory provided".to_string())
        })?;

        log::info!("️ Building browser pool with config: {:?}", config);

        // Create inner state
        let inner = BrowserPoolInner::new(config, factory);

        // Start keep-alive thread if enabled
        let keep_alive_handle = if self.enable_keep_alive {
            log::info!(" Starting keep-alive monitoring thread");
            Some(BrowserPool::start_keep_alive(Arc::clone(&inner)))
        } else {
            log::warn!("⚠️ Keep-alive thread disabled (should only be used for testing)");
            None
        };

        log::info!("✅ Browser pool built successfully");

        Ok(BrowserPool {
            inner,
            keep_alive_handle,
        })
    }
}

impl Default for BrowserPoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Environment Initialization (feature-gated)
// ============================================================================

/// Initialize browser pool from environment variables.
///
/// This is a convenience function for common initialization patterns.
/// It reads configuration from environment variables with sensible defaults.
///
/// # Feature Flag
///
/// This function is only available when the `env-config` feature is enabled.
///
/// # Environment Variables
///
/// - `BROWSER_POOL_SIZE`: Maximum pool size (default: 5)
/// - `BROWSER_WARMUP_COUNT`: Warmup browser count (default: 3)
/// - `BROWSER_TTL_SECONDS`: Browser TTL in seconds (default: 3600)
/// - `BROWSER_WARMUP_TIMEOUT_SECONDS`: Warmup timeout (default: 60)
/// - `CHROME_PATH`: Custom Chrome binary path (optional)
///
/// # Returns
///
/// `Arc<Mutex<BrowserPool>>` ready for use in web handlers.
///
/// # Errors
///
/// - Returns error if configuration is invalid.
/// - Returns error if warmup fails.
///
/// # Example
///
/// ```rust,ignore
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     env_logger::init();
///
///     let pool = init_browser_pool().await?;
///
///     // Use pool in handlers...
///
///     Ok(())
/// }
/// ```
#[cfg(feature = "env-config")]
pub async fn init_browser_pool() -> Result<Arc<Mutex<BrowserPool>>> {
    use crate::config::env::{chrome_path_from_env, from_env};
    use crate::factory::ChromeBrowserFactory;

    log::info!("Initializing browser pool from environment...");

    // Load configuration from environment
    let config = from_env()?;

    // Get optional Chrome path
    let chrome_path = chrome_path_from_env();

    log::info!("Pool configuration from environment:");
    log::info!("   - Max pool size: {}", config.max_pool_size);
    log::info!("   - Warmup count: {}", config.warmup_count);
    log::info!(
        "   - Browser TTL: {}s ({}min)",
        config.browser_ttl.as_secs(),
        config.browser_ttl.as_secs() / 60
    );
    log::info!("   - Warmup timeout: {}s", config.warmup_timeout.as_secs());
    log::info!(
        "   - Chrome path: {}",
        chrome_path.as_deref().unwrap_or("auto-detect")
    );

    // Create factory based on whether custom path is provided
    let factory: Box<dyn BrowserFactory> = match chrome_path {
        Some(path) => {
            log::info!("Using custom Chrome path: {}", path);
            Box::new(ChromeBrowserFactory::with_path(path))
        }
        None => {
            log::info!("Using auto-detected Chrome browser");
            Box::new(ChromeBrowserFactory::with_defaults())
        }
    };

    // Create browser pool with Chrome factory
    log::debug!("Building browser pool...");
    let pool = BrowserPool::builder()
        .config(config.clone())
        .factory(factory)
        .enable_keep_alive(true)
        .build()
        .map_err(|e| {
            log::error!("❌ Failed to create browser pool: {}", e);
            e
        })?;

    log::info!("✅ Browser pool created successfully");

    // Warmup the pool
    log::info!(
        "Warming up browser pool with {} instances...",
        config.warmup_count
    );
    pool.warmup().await.map_err(|e| {
        log::error!("❌ Failed to warmup pool: {}", e);
        e
    })?;

    let stats = pool.stats();
    log::info!(
        "✅ Browser pool ready - Available: {}, Active: {}, Total: {}",
        stats.available,
        stats.active,
        stats.total
    );

    Ok(pool.into_shared())
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that BrowserPool builder rejects missing factory.
    ///
    /// A factory is mandatory because the pool needs to know how to
    /// create browser instances. This test ensures proper error handling.
    #[test]
    fn test_pool_builder_missing_factory() {
        // We need a tokio runtime for the builder
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let config = crate::config::BrowserPoolConfigBuilder::new()
                .max_pool_size(3)
                .build()
                .unwrap();

            let result = BrowserPool::builder()
                .config(config)
                // Intentionally missing factory
                .build();

            assert!(result.is_err(), "Build should fail without factory");

            match result {
                Err(BrowserPoolError::Configuration(msg)) => {
                    assert!(
                        msg.contains("No browser factory provided"),
                        "Expected factory error, got: {}",
                        msg
                    );
                }
                _ => panic!("Expected Configuration error for missing factory"),
            }
        });
    }

    /// Verifies that BrowserPoolBuilder implements Default.
    #[test]
    fn test_builder_default() {
        let builder: BrowserPoolBuilder = Default::default();
        assert!(builder.config.is_none());
        assert!(builder.factory.is_none());
        assert!(builder.enable_keep_alive);
    }

    /// Verifies that enable_keep_alive can be disabled.
    #[test]
    fn test_builder_disable_keep_alive() {
        let builder = BrowserPoolBuilder::new().enable_keep_alive(false);
        assert!(!builder.enable_keep_alive);
    }
}
