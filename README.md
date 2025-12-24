# html2pdf-api

> Thread-safe headless browser pool for high-performance HTML to PDF conversion with native Rust web framework integration.

[![Crates.io](https://img.shields.io/crates/v/html2pdf-api.svg)](https://crates.io/crates/html2pdf-api)
[![Documentation](https://docs.rs/html2pdf-api/badge.svg)](https://docs.rs/html2pdf-api)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/lpfy/html2pdf-api#license)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

A production-ready Rust library for managing a pool of headless Chrome browsers to convert HTML to PDF. Designed for high-performance web APIs with built-in support for popular Rust web frameworks.

## ✨ Features

- 🔒 **Thread-Safe Pool Management** - Efficient browser reuse with RAII handles
- ❤️ **Automatic Health Monitoring** - Background health checks with automatic browser retirement
- ⏰ **TTL-Based Lifecycle** - Configurable browser time-to-live prevents memory leaks
- 🛡️ **Production-Ready** - Comprehensive error handling and graceful shutdown
- 🚀 **Framework Integration** - Pre-built handlers for Actix-web, Rocket, and Axum
- ⚙️ **Flexible Configuration** - Environment variables or direct configuration
- 📊 **Pool Statistics** - Real-time metrics for monitoring
- 🌍 **Cross-Platform** - Works on Linux, macOS, and Windows

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
| `actix-integration` | Actix-web framework support with pre-built handlers | No |
| `rocket-integration` | Rocket framework support | No |
| `axum-integration` | Axum framework support | No |
| `test-utils` | Mock factory for testing | No |

Enable features as needed:

```toml
[dependencies]
html2pdf-api = { version = "0.2", features = ["actix-integration"] }
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

### Environment Variables

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

#### Option 1: Pre-built Routes (Recommended)

Get a fully functional PDF API with just a few lines of code:

```rust
use actix_web::{App, HttpServer, web};
use html2pdf_api::prelude::*;
use html2pdf_api::integrations::actix::configure_routes;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = init_browser_pool().await
        .expect("Failed to initialize browser pool");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .configure(configure_routes)  // Adds all PDF endpoints!
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```

This gives you these endpoints automatically:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/pdf?url=https://example.com` | Convert URL to PDF |
| POST | `/pdf/html` | Convert HTML to PDF |
| GET | `/pool/stats` | Pool statistics |
| GET | `/health` | Health check |
| GET | `/ready` | Readiness check |

#### Option 2: Custom Handler with Service Functions

For custom logic (authentication, rate limiting, etc.):

```rust
use actix_web::{web, HttpResponse, Responder};
use html2pdf_api::prelude::*;
use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};

async fn my_pdf_handler(
    pool: web::Data<SharedBrowserPool>,
    query: web::Query<PdfFromUrlRequest>,
) -> impl Responder {
    // Custom pre-processing: auth, rate limiting, logging, etc.
    log::info!("Custom handler: {}", query.url);

    let pool = pool.into_inner();
    let request = query.into_inner();

    // Call service in blocking context
    let result = web::block(move || {
        generate_pdf_from_url(&pool, &request)
    }).await;

    match result {
        Ok(Ok(pdf)) => HttpResponse::Ok()
            .content_type("application/pdf")
            .insert_header(("Content-Disposition", pdf.content_disposition()))
            .body(pdf.data),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
```

#### Option 3: Manual Browser Control

For complete control over browser operations:

```rust
use actix_web::{web, HttpResponse, Responder};
use html2pdf_api::prelude::*;

async fn generate_pdf(
    pool: web::Data<SharedBrowserPool>,
) -> impl Responder {
    let pool_guard = pool.lock().unwrap();
    let browser = pool_guard.get().unwrap();
    
    let tab = browser.new_tab().unwrap();
    tab.navigate_to("https://example.com").unwrap();
    tab.wait_until_navigated().unwrap();
    let pdf = tab.print_to_pdf(None).unwrap();
    
    HttpResponse::Ok()
        .content_type("application/pdf")
        .body(pdf)
}
```

### Rocket

#### Option 1: Pre-built Routes (Recommended)

Get a fully functional PDF API with just a few lines of code:

```rust
use html2pdf_api::prelude::*;
use html2pdf_api::integrations::rocket::routes;

#[rocket::launch]
async fn launch() -> _ {
    let pool = init_browser_pool().await
        .expect("Failed to initialize browser pool");

    rocket::build()
        .manage(pool)
        .mount("/", routes())  // Adds all PDF endpoints!
}
```

This gives you these endpoints automatically:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/pdf?url=https://example.com` | Convert URL to PDF |
| POST | `/pdf/html` | Convert HTML to PDF |
| GET | `/pool/stats` | Pool statistics |
| GET | `/health` | Health check |
| GET | `/ready` | Readiness check |

#### Option 2: Custom Handler with Service Functions

For custom logic (authentication, rate limiting, etc.):

```rust
use rocket::{get, State, http::ContentType, Response};
use html2pdf_api::prelude::*;
use html2pdf_api::service::{generate_pdf_from_url, PdfFromUrlRequest};
use std::io::Cursor;

#[get("/custom-pdf?<url>&<filename>&<waitsecs>&<landscape>&<download>&<print_background>")]
pub fn my_pdf_handler(
    pool: &State<SharedBrowserPool>,
    url: String,
    filename: Option<String>,
    waitsecs: Option<u64>,
    landscape: Option<bool>,
    download: Option<bool>,
    print_background: Option<bool>,
) -> Result<Response<'static>, rocket::http::Status> {
    // Custom pre-processing: auth, rate limiting, logging, etc.
    log::info!("Custom handler: {}", url);

    let request = PdfFromUrlRequest {
        url,
        filename,
        waitsecs,
        landscape,
        download,
        print_background,
    };

    match generate_pdf_from_url(pool.inner(), &request) {
        Ok(pdf) => {
            let response = Response::build()
                .header(ContentType::PDF)
                .raw_header("Content-Disposition", pdf.content_disposition())
                .sized_body(pdf.data.len(), Cursor::new(pdf.data))
                .finalize();
            Ok(response)
        }
        Err(e) => {
            log::error!("PDF generation failed: {}", e);
            Err(rocket::http::Status::new(e.status_code()))
        }
    }
}
```

#### Option 3: Manual Browser Control

For complete control over browser operations:

```rust
use rocket::{get, State, http::ContentType, Response};
use html2pdf_api::prelude::*;
use std::io::Cursor;

#[get("/manual-pdf")]
pub fn generate_pdf(
    pool: &State<SharedBrowserPool>,
) -> Result<Response<'static>, rocket::http::Status> {
    let pool_guard = pool.lock().unwrap();
    let browser = pool_guard.get()
        .map_err(|_| rocket::http::Status::ServiceUnavailable)?;
    
    let tab = browser.new_tab()
        .map_err(|_| rocket::http::Status::InternalServerError)?;
    tab.navigate_to("https://example.com")
        .map_err(|_| rocket::http::Status::BadGateway)?;
    tab.wait_until_navigated()
        .map_err(|_| rocket::http::Status::BadGateway)?;
    let pdf = tab.print_to_pdf(None)
        .map_err(|_| rocket::http::Status::InternalServerError)?;
    
    let response = Response::build()
        .header(ContentType::PDF)
        .sized_body(pdf.len(), Cursor::new(pdf))
        .finalize();
    Ok(response)
}
```

### Axum (Manual Browser Control Only)

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

## Pre-built API Endpoints (Actix-web)

When using `configure_routes`, these endpoints are available:

### GET /pdf - Convert URL to PDF

**Query Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `url` | string | Yes | - | URL to convert |
| `filename` | string | No | `document.pdf` | Output filename |
| `waitsecs` | u64 | No | 5 | Seconds to wait for JavaScript |
| `landscape` | bool | No | false | Landscape orientation |
| `download` | bool | No | false | Force download vs inline display |
| `print_background` | bool | No | true | Include background graphics |

**Example:**

```bash
curl "http://localhost:8080/pdf?url=https://example.com&filename=report.pdf&landscape=true" \
  --output report.pdf
```

### POST /pdf/html - Convert HTML to PDF

**Request Body (JSON):**

```json
{
    "html": "<html><body><h1>Hello World</h1></body></html>",
    "filename": "document.pdf",
    "waitsecs": 2,
    "landscape": false,
    "download": false,
    "print_background": true
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/pdf/html \
  -H "Content-Type: application/json" \
  -d '{"html": "<h1>Hello</h1>", "filename": "hello.pdf"}' \
  --output hello.pdf
```

### GET /pool/stats - Pool Statistics

**Response:**

```json
{
    "available": 3,
    "active": 2,
    "total": 5
}
```

### GET /health - Health Check

**Response (200 OK):**

```json
{
    "status": "healthy",
    "service": "html2pdf-api"
}
```

### GET /ready - Readiness Check

**Response (200 OK):**

```json
{
    "status": "ready"
}
```

**Response (503 Service Unavailable):**

```json
{
    "status": "not_ready",
    "reason": "no_available_capacity"
}
```

## JavaScript Wait Behavior

The `waitsecs` parameter controls how long to wait for JavaScript rendering. For pages that signal completion, you can enable early exit:

```javascript
// In your web page, signal when rendering is complete:
window.isPageDone = true;
```

The service polls every 200ms for this flag. If set, PDF generation proceeds immediately without waiting the full duration.

**Recommended `waitsecs` values:**

| Page Type | Value |
|-----------|-------|
| Static HTML | 1-2 |
| Light JavaScript | 3-5 |
| Heavy SPA (React, Vue) | 5-10 |
| Complex charts/visualizations | 10-15 |

## Architecture

```
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
    .ping_interval(Duration::from_secs(15))      // Check every 15s
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
println!("Total browsers: {}", stats.total);

// For metrics systems
metrics::gauge!("browser_pool.available", stats.available as f64);
metrics::gauge!("browser_pool.active", stats.active as f64);
```

## ❗ Error Handling

### Pool Errors

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

### Service Errors (Actix-web Integration)

When using the service layer, errors include HTTP status code mapping:

```rust
use html2pdf_api::service::{PdfServiceError, ErrorResponse};

fn handle_error(error: PdfServiceError) -> (u16, ErrorResponse) {
    let status = error.status_code();  // e.g., 400, 503, 504
    let response = ErrorResponse::from(&error);
    
    // Check if error is worth retrying
    if error.is_retryable() {
        log::info!("Transient error, consider retry: {}", error);
    }
    
    (status, response)
}
```

**Error Codes:**

| Error | HTTP Status | Retryable |
|-------|-------------|-----------|
| `INVALID_URL` | 400 | No |
| `EMPTY_HTML` | 400 | No |
| `BROWSER_UNAVAILABLE` | 503 | Yes |
| `NAVIGATION_FAILED` | 502 | Yes |
| `NAVIGATION_TIMEOUT` | 504 | Yes |
| `PDF_GENERATION_FAILED` | 502 | Yes |
| `TIMEOUT` | 504 | Yes |
| `POOL_SHUTTING_DOWN` | 503 | No |

## Requirements

- **Rust**: 1.85 or later
- **Tokio**: Runtime required for async operations
- **Chrome/Chromium**

### No installation required! 🎉

The library automatically downloads a compatible Chromium binary if Chrome is not detected on your system. Downloaded binaries are cached for future use:

| Platform | Cache Location |
|----------|----------------|
| Linux | `~/.local/share/headless-chrome` |
| macOS | `~/Library/Application Support/headless-chrome` |
| Windows | `C:\Users\<User>\AppData\Roaming\headless-chrome\data` |

- **First run**: May take a few minutes to download Chromium (~170MB)
- **Subsequent runs**: Uses cached version instantly

### Chrome/Chromium - Manual Installation (Optional)

While not required, you can install Chrome manually if preferred:

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

See the `examples` directory for complete working examples:

- `actix_web_example.rs` - Actix-web with pre-built routes, custom handlers, and manual control
- `rocket_example.rs` - Rocket integration
- `axum_example.rs` - Axum integration

Run examples:

```bash
# Actix-web (demonstrates all integration patterns)
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

This crate builds upon the excellent [headless_chrome](https://github.com/rust-headless-chrome/rust-headless-chrome) crate.