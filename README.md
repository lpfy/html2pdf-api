# html2pdf-api

> Thread-safe headless browser pool for high-performance HTML to PDF conversion with native Rust web framework integration.

[![Crates.io](https://img.shields.io/crates/v/html2pdf-api.svg)](https://crates.io/crates/html2pdf-api)
[![Documentation](https://docs.rs/html2pdf-api/badge.svg)](https://docs.rs/html2pdf-api)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/lpfy/html2pdf-api#license)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

A production-ready Rust library for managing a pool of headless Chrome browsers to convert HTML to PDF. Designed for high-performance web APIs with built-in support for popular Rust web frameworks.

## ✨ Features

- 🔄 **Thread-Safe Pool Management** - Efficient browser reuse with RAII handles
- ❤️ **Automatic Health Monitoring** - Background health checks with automatic browser retirement
- ⏰ **TTL-Based Lifecycle** - Configurable browser time-to-live prevents memory leaks
- 🚀 **Production-Ready** - Comprehensive error handling and graceful shutdown
- 🌐 **Framework Integration** - First-class support for Actix-web, Rocket, and Axum
- 🔧 **Flexible Configuration** - Environment variables, config files, or direct configuration
- 📊 **Pool Statistics** - Real-time metrics for monitoring
- 🎯 **Cross-Platform** - Works on Linux, macOS, and Windows


## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
html2pdf-api = "0.2"
```

### Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `env-config` | Load configuration from environment variables | Yes |
| `actix-integration` | Actix-web framework support | No |
| `rocket-integration` | Rocket framework support | No |
| `axum-integration` | Axum framework support | No |
| `test-utils` | Mock factory for testing | No |

Enable features as needed:

```toml
[dependencies]
html2pdf-api = { version = "0.2", features = ["actix-integration", "env-config"] }
```

## Quick Start

### Basic Usage

```rust
use html2pdf_api::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create pool with configuration
    let pool = BrowserPool::builder()
        .config(
            BrowserPoolConfigBuilder::new()
                .max_pool_size(5)
                .warmup_count(3)
                .browser_ttl(Duration::from_secs(3600))
                .build()?
        )
        .factory(Box::new(ChromeBrowserFactory::with_defaults()))
        .build()?;

    // Warmup the pool (recommended for production)
    pool.warmup().await?;

    // Use a browser
    {
        let browser = pool.get()?;
        let tab = browser.new_tab()?;
        
        // Navigate and generate PDF
        tab.navigate_to("https://example.com")?;
        tab.wait_until_navigated()?;
        let pdf_data = tab.print_to_pdf(None)?;
        
        println!("Generated PDF: {} bytes", pdf_data.len());
    } // Browser automatically returned to pool

    // Graceful shutdown
    pool.shutdown_async().await;

    Ok(())
}
```

### Environment Configuration

Enable the `env-config` feature for simpler initialization:

```rust
use html2pdf_api::init_browser_pool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Reads configuration from environment variables
    let pool = init_browser_pool().await?;
    
    // Pool is Arc<Mutex<BrowserPool>>, ready for web handlers
    Ok(())
}
```

#### Environment Variables

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `BROWSER_POOL_SIZE` | usize | 5 | Maximum browsers in pool |
| `BROWSER_WARMUP_COUNT` | usize | 3 | Browsers to pre-create on startup |
| `BROWSER_TTL_SECONDS` | u64 | 3600 | Browser lifetime before retirement |
| `BROWSER_WARMUP_TIMEOUT_SECONDS` | u64 | 60 | Maximum warmup duration |
| `BROWSER_PING_INTERVAL_SECONDS` | u64 | 15 | Health check frequency |
| `BROWSER_MAX_PING_FAILURES` | u32 | 3 | Failures before browser removal |
| `CHROME_PATH` | String | auto | Custom Chrome/Chromium binary path |

## Web Framework Integration

### Actix-web

```rust
use actix_web::{web, App, HttpServer, HttpResponse, Responder};
use html2pdf_api::prelude::*;

async fn generate_pdf(
    pool: web::Data<SharedBrowserPool>,
) -> impl Responder {
    let pool_guard = pool.lock().unwrap();
    let browser = pool_guard.get().unwrap();
    
    let tab = browser.new_tab().unwrap();
    tab.navigate_to("https://example.com").unwrap();
    let pdf = tab.print_to_pdf(None).unwrap();
    
    HttpResponse::Ok()
        .content_type("application/pdf")
        .body(pdf)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = BrowserPool::builder()
        .factory(Box::new(ChromeBrowserFactory::with_defaults()))
        .build()
        .unwrap();
    
    pool.warmup().await.unwrap();
    let shared_pool = pool.into_shared();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(shared_pool.clone()))
            .route("/pdf", web::get().to(generate_pdf))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```

### Rocket

```rust
use rocket::{get, launch, routes, State};
use html2pdf_api::prelude::*;

#[get("/pdf")]
async fn generate_pdf(pool: &State<SharedBrowserPool>) -> Vec<u8> {
    let pool_guard = pool.lock().unwrap();
    let browser = pool_guard.get().unwrap();
    
    let tab = browser.new_tab().unwrap();
    tab.navigate_to("https://example.com").unwrap();
    tab.print_to_pdf(None).unwrap()
}

#[launch]
async fn rocket() -> _ {
    let pool = BrowserPool::builder()
        .factory(Box::new(ChromeBrowserFactory::with_defaults()))
        .build()
        .unwrap();
    
    pool.warmup().await.unwrap();

    rocket::build()
        .manage(pool.into_shared())
        .mount("/", routes![generate_pdf])
}
```

### Axum

```rust
use axum::{Router, routing::get, extract::State, response::IntoResponse};
use html2pdf_api::prelude::*;

async fn generate_pdf(
    State(pool): State<SharedBrowserPool>,
) -> impl IntoResponse {
    let pool_guard = pool.lock().unwrap();
    let browser = pool_guard.get().unwrap();
    
    let tab = browser.new_tab().unwrap();
    tab.navigate_to("https://example.com").unwrap();
    let pdf = tab.print_to_pdf(None).unwrap();
    
    (
        [(axum::http::header::CONTENT_TYPE, "application/pdf")],
        pdf,
    )
}

#[tokio::main]
async fn main() {
    let pool = BrowserPool::builder()
        .factory(Box::new(ChromeBrowserFactory::with_defaults()))
        .build()
        .unwrap();
    
    pool.warmup().await.unwrap();

    let app = Router::new()
        .route("/pdf", get(generate_pdf))
        .with_state(pool.into_shared());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## Architecture

```text
┌─────────────────────────────────────────────┐
│         Your Web Application                │
│      (Actix-web / Rocket / Axum)            │
└─────────────────┬───────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│              BrowserPool                    │
│ ┌─────────────────────────────────────────┐ │
│ │   Available Pool (idle browsers)        │ │
│ │   [Browser1] [Browser2] [Browser3]      │ │
│ └─────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────┐ │
│ │   Active Tracking (in-use browsers)     │ │
│ │   {id → Browser}                        │ │
│ └─────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────┐ │
│ │   Keep-Alive Thread                     │ │
│ │   (health checks + TTL enforcement)     │ │
│ └─────────────────────────────────────────┘ │
└─────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│        Headless Chrome Browsers             │
│     (managed by headless_chrome crate)      │
└─────────────────────────────────────────────┘
```

### Key Design Decisions

- **RAII Pattern**: Browsers are automatically returned to the pool when `BrowserHandle` is dropped
- **Lock Ordering**: Strict lock ordering (active → available) prevents deadlocks
- **Health Checks**: Lock-free health checks avoid blocking other operations
- **Staggered Warmup**: TTLs are offset to prevent simultaneous browser expiration
- **Graceful Shutdown**: Condvar signaling enables immediate shutdown response

## ⚙️ Configuration Guide

### Recommended Production Settings

```rust
use std::time::Duration;
use html2pdf_api::BrowserPoolConfigBuilder;

let config = BrowserPoolConfigBuilder::new()
    .max_pool_size(10)                           // Adjust based on load
    .warmup_count(5)                             // Pre-warm half the pool
    .browser_ttl(Duration::from_secs(3600))      // 1 hour lifetime
    .ping_interval(Duration::from_secs(30))      // Check every 30s
    .max_ping_failures(3)                        // Tolerate transient failures
    .warmup_timeout(Duration::from_secs(120))    // 2 min warmup limit
    .build()?;
```

### Custom Chrome Path

```rust
use html2pdf_api::ChromeBrowserFactory;

// Linux
let factory = ChromeBrowserFactory::with_path("/usr/bin/google-chrome");

// macOS
let factory = ChromeBrowserFactory::with_path(
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
);

// Windows
let factory = ChromeBrowserFactory::with_path(
    r"C:\Program Files\Google\Chrome\Application\chrome.exe"
);
```

## Testing

Use the `test-utils` feature for testing without Chrome:

```rust
use html2pdf_api::factory::mock::MockBrowserFactory;

// Factory that always fails (for error handling tests)
let factory = MockBrowserFactory::always_fails("Simulated failure");

// Factory that fails after N creations (for exhaustion tests)
let factory = MockBrowserFactory::fail_after_n(3, "Resource exhausted");

let pool = BrowserPool::builder()
    .factory(Box::new(factory))
    .enable_keep_alive(false)  // Disable for faster tests
    .build()?;
```

## Monitoring

```rust
let stats = pool.stats();

println!("Available browsers: {}", stats.available);
println!("Active browsers: {}", stats.active);
println!("Checked out: {}", stats.checked_out());

// For metrics systems
metrics::gauge!("browser_pool.available", stats.available as f64);
metrics::gauge!("browser_pool.active", stats.active as f64);
```

## ❗ Error Handling

```rust
use html2pdf_api::{BrowserPool, BrowserPoolError};

match pool.get() {
    Ok(browser) => {
        // Use browser
    }
    Err(BrowserPoolError::ShuttingDown) => {
        // Pool is shutting down - stop processing
    }
    Err(BrowserPoolError::BrowserCreation(msg)) => {
        // Chrome failed to start - check installation
        log::error!("Browser creation failed: {}", msg);
    }
    Err(BrowserPoolError::HealthCheckFailed(msg)) => {
        // Browser became unhealthy - will be replaced automatically
        log::warn!("Health check failed: {}", msg);
    }
    Err(e) => {
        log::error!("Pool error: {}", e);
    }
}
```

## Requirements

- **Rust**: 1.85 or later
- **Tokio**: Runtime required for async operations

### Chrome/Chromium

**No installation required!**

The library automatically downloads a compatible Chromium binary if Chrome is not detected on your system. Downloaded binaries are cached for future use:

| Platform | Cache Location |
|----------|----------------|
| Linux | `~/.local/share/headless-chrome` |
| macOS | `~/Library/Application Support/headless-chrome` |
| Windows | `C:\Users\<User>\AppData\Roaming\headless-chrome\data` |

> **First run**: May take a few minutes to download Chromium (~170MB)  
> **Subsequent runs**: Uses cached version instantly

### Chrome/Chromium - Manual Installation (Optional)

**While not required, you can install Chrome manually if preferred:**

**Ubuntu/Debian:**
```bash
wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
sudo dpkg -i google-chrome-stable_current_amd64.deb
```

**macOS:**
```bash
brew install --cask google-chrome
```

**Windows:**
Download from [google.com/chrome](https://www.google.com/chrome/)

## Examples

See the [examples](examples/) directory for complete working examples:

- [`actix_web_example.rs`](examples/actix_web_example.rs) - Actix-web integration
- [`rocket_example.rs`](examples/rocket_example.rs) - Rocket integration
- [`axum_example.rs`](examples/axum_example.rs) - Axum integration

Run examples:

```bash
# Actix-web
cargo run --example actix_web_example --features actix-integration

# Rocket
cargo run --example rocket_example --features rocket-integration

# Axum
cargo run --example axum_example --features axum-integration
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed:

- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

This crate builds upon the excellent [headless_chrome](https://crates.io/crates/headless_chrome) crate.