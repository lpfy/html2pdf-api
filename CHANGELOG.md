# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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