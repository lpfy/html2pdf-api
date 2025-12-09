//! Rocket integration example.
//!
//! Run with:
//! ```bash
//! cargo run --example rocket_example --features rocket-integration
//! ```
//!
//! Then visit: http://localhost:8000/pdf

use html2pdf_api::prelude::*;
use rocket::http::{ContentType, Status};
use rocket::response::{self, Responder, Response};
use rocket::{get, launch, routes, State};
use std::io::Cursor;
use std::time::Duration;

/// Custom responder for PDF responses.
pub struct PdfResponse(pub Vec<u8>);

impl<'r> Responder<'r, 'static> for PdfResponse {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> response::Result<'static> {
        Response::build()
            .header(ContentType::PDF)
            .raw_header(
                "Content-Disposition",
                "attachment; filename=\"example.pdf\"",
            )
            .sized_body(self.0.len(), Cursor::new(self.0))
            .ok()
    }
}

/// Handler that generates a PDF from a URL.
#[get("/pdf")]
async fn generate_pdf(pool: &State<SharedBrowserPool>) -> Result<PdfResponse, Status> {
    // Acquire lock on the pool
    let pool_guard = pool.lock().map_err(|e| {
        log::error!("Failed to lock pool: {}", e);
        Status::InternalServerError
    })?;

    // Get a browser from the pool
    let browser = pool_guard.get().map_err(|e| {
        log::error!("Failed to get browser: {}", e);
        Status::ServiceUnavailable
    })?;

    log::info!("Got browser {} from pool", browser.id());

    // Create a new tab
    let tab = browser.new_tab().map_err(|e| {
        log::error!("Failed to create tab: {}", e);
        Status::InternalServerError
    })?;

    // Navigate to the URL
    tab.navigate_to("https://example.com").map_err(|e| {
        log::error!("Failed to navigate: {}", e);
        Status::InternalServerError
    })?;

    // Wait for navigation to complete
    tab.wait_until_navigated().map_err(|e| {
        log::error!("Navigation timeout: {}", e);
        Status::InternalServerError
    })?;

    // Generate PDF
    let pdf_data = tab.print_to_pdf(None).map_err(|e| {
        log::error!("Failed to generate PDF: {}", e);
        Status::InternalServerError
    })?;

    log::info!("Generated PDF: {} bytes", pdf_data.len());

    Ok(PdfResponse(pdf_data))
}

/// Handler that returns pool statistics.
#[get("/stats")]
fn pool_stats(pool: &State<SharedBrowserPool>) -> Result<String, Status> {
    let pool_guard = pool.lock().map_err(|_| Status::InternalServerError)?;
    let stats = pool_guard.stats();

    Ok(format!(
        r#"{{"available": {}, "active": {}, "total": {}}}"#,
        stats.available, stats.active, stats.total
    ))
}

/// Health check endpoint.
#[get("/health")]
fn health() -> &'static str {
    "OK"
}

#[launch]
async fn rocket() -> _ {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Rocket example...");

    // Create browser pool
    let pool = BrowserPool::builder()
        .config(
            BrowserPoolConfigBuilder::new()
                .max_pool_size(3)
                .warmup_count(2)
                .browser_ttl(Duration::from_secs(3600))
                .ping_interval(Duration::from_secs(30))
                .build()
                .expect("Invalid configuration"),
        )
        .factory(Box::new(ChromeBrowserFactory::with_defaults()))
        .build()
        .expect("Failed to create browser pool");

    log::info!("Browser pool created, warming up...");

    // Warmup the pool
    pool.warmup().await.expect("Failed to warmup pool");

    log::info!("Pool warmed up successfully");

    // Convert to shared state
    let shared_pool = pool.into_shared();

    log::info!("Starting server on http://localhost:8000");

    rocket::build()
        .manage(shared_pool)
        .mount("/", routes![generate_pdf, pool_stats, health])
}