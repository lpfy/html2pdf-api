# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2026-03-14

### Fixed
- Fixed broken `Mutex` intra-doc link in `prelude` module that caused `rustdoc -D warnings` CI failure.

## [0.3.0] - 2026-03-14

### ⚠️ Breaking Changes
- **`SharedBrowserPool` type changed** from `Arc<Mutex<BrowserPool>>` to `Arc<BrowserPool>`. Callers no longer need to `.lock()` the pool before use — the pool uses fine-grained internal locks for thread safety.
- **`SharedPool` type aliases updated** in all integration modules (actix, axum, rocket) to match the new `Arc<BrowserPool>` type.
- **`PdfServiceError::PoolLockFailed` variant removed** — no longer applicable without the outer mutex.
- **`Mutex` no longer re-exported from `prelude`** — use `std::sync::Mutex` directly if needed elsewhere.

### Changed
- All service functions (`generate_pdf_from_url`, `generate_pdf_from_html`, `get_pool_stats`, `is_pool_ready`, `acquire_browser`) now accept `&BrowserPool` instead of `&Mutex<BrowserPool>`.
- Integration handler signatures and helper functions updated for the new pool type.
- All example files updated to remove `.lock()` calls and use `Arc::new(pool)` directly.

### Fixed
- **Mutex poison safety**: All 16 internal `.lock().unwrap()` calls in `pool.rs` replaced with `.unwrap_or_else(|poisoned| poisoned.into_inner())` poison recovery, preventing cascading panics from a single thread failure.

## [0.2.8] - 2026-03-14

### Added
- Comprehensive Axum integration with pre-built handlers and `configure_routes()` function.
- `axum_example.rs` demonstrating all integration patterns for Axum.

### Changed
- **Standardization**: Renamed Rocket's `routes()` function to `configure_routes()` to match Actix-web and Axum consistency.
- **Improved Timeouts**: Increased default warmup and browser creation timeouts in examples to handle slow Chromium binary downloads on WSL/slow networks.
- **Dependency Optimization**: Cleaned up unused imports and addressed `clippy` warnings across integrations and examples.

### Fixed
- Fixed Axum response building logic to correctly handle PDF data ownership.

## [0.2.7] - 2025-12-24
  ### Added
   - Examples for Rocket integration with pre-built handlers

## [0.2.6] - 2025-12-23

### Changed
 - Minor fixes for tests, add required dependency for features

## [0.2.5] - 2025-12-23

### Changed
 - Minor fixes for doc generating and tests

## [0.2.4] - 2025-12-23

### Changed
 - Minor fixes for code formats and tests

## [0.2.3] - 2025-12-23

### Added
- Pre-built handlers: pdf_from_url, pdf_from_html, pool_stats, health_check, readiness_check
- configure_routes() function
- SharedPool type alias (aligns with service module)
- Response builder helpers
- Examples of Pre-built handlers for actix_web 

## [0.2.2] - 2025-12-22

### Changed
- Fix changelog extraction in release workflow

## [0.2.1] - 2025-12-12

### Changed
- Fix changelog extraction in release workflow

## [0.2.0] - 2025-12-12

### Added

- **Automatic Chromium Download**: Chrome/Chromium is now automatically downloaded if not detected on the system. No manual installation required for first-time users.
  - Downloaded binaries are cached in platform-specific directories:
    - Linux: `~/.local/share/headless-chrome`
    - macOS: `~/Library/Application Support/headless-chrome`
    - Windows: `C:\Users\<User>\AppData\Roaming\headless-chrome\data`
  - First run may take a few minutes to download (~170MB)
  - Subsequent runs use the cached version instantly

### Changed

- Updated `headless_chrome` dependency to include the `fetch` feature by default
- Manual Chrome/Chromium installation is now optional (still supported via `CHROME_PATH` environment variable)

## [0.1.0] - 2025-12-11

### Added

- Initial release
- `BrowserPool` - Thread-safe browser pool with automatic lifecycle management
- `BrowserPoolConfig` - Configuration struct with builder pattern
- `BrowserPoolConfigBuilder` - Fluent configuration builder with validation
- `BrowserHandle` - RAII handle for automatic browser return
- `BrowserFactory` trait - Abstraction for browser creation
- `ChromeBrowserFactory` - Factory for Chrome/Chromium browsers
- `MockBrowserFactory` - Mock factory for testing (feature-gated)
- `PoolStats` - Pool statistics for monitoring
- `Healthcheck` trait - Health check abstraction
- Health monitoring with configurable ping interval and failure tolerance
- TTL-based browser retirement with staggered warmup
- Graceful shutdown with async and sync variants
- Environment-based configuration (`env-config` feature)
- Actix-web integration (`actix-integration` feature)
- Rocket integration (`rocket-integration` feature)
- Axum integration (`axum-integration` feature)
- Comprehensive documentation with examples
- Unit tests for configuration and error handling

### Features

- `env-config` - Load configuration from environment variables
- `actix-integration` - Actix-web framework support
- `rocket-integration` - Rocket framework support
- `axum-integration` - Axum framework support
- `test-utils` - Mock factory for testing

### Documentation

- Full API documentation with examples
- Architecture diagrams
- Web framework integration guides
- Configuration guide
- Error handling guide

[0.1.0]: https://github.com/lpfy/html2pdf-api/releases/tag/v0.1.0