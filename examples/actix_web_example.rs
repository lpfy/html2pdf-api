//! Actix-web integration example.
//!
//! Run with:
//! ```bash
//! cargo run --example actix_web_example --features actix-integration
//! ```
//!
//! Then visit: http://localhost:8080/pdf

use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use html2pdf_api::prelude::*;
use std::sync::Arc;
use std::time::Duration;

/// Handler that generates a PDF from a URL.
async fn generate_pdf(pool: web::Data<SharedBrowserPool>) -> impl Responder {
    // Acquire lock on the pool
    let pool_guard = match pool.lock() {
        Ok(guard) => guard,
        Err(e) => {
            log::error!("Failed to lock pool: {}", e);
            return HttpResponse::InternalServerError().body("Pool lock failed");
        }
    };

    // Get a browser from the pool
    let browser = match pool_guard.get() {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to get browser: {}", e);
            return HttpResponse::ServiceUnavailable().body(format!("Browser unavailable: {}", e));
        }
    };

    log::info!("Got browser {} from pool", browser.id());

    // Create a new tab
    let tab = match browser.new_tab() {
        Ok(t) => t,
        Err(e) => {
            log::error!("Failed to create tab: {}", e);
            return HttpResponse::InternalServerError().body("Failed to create tab");
        }
    };

    // Navigate to the URL
    if let Err(e) = tab.navigate_to("https://example.com") {
        log::error!("Failed to navigate: {}", e);
        return HttpResponse::InternalServerError().body("Failed to navigate");
    }

    // Wait for navigation to complete
    if let Err(e) = tab.wait_until_navigated() {
        log::error!("Navigation timeout: {}", e);
        return HttpResponse::InternalServerError().body("Navigation timeout");
    }

    // Generate PDF
    let pdf_data = match tab.print_to_pdf(None) {
        Ok(data) => data,
        Err(e) => {
            log::error!("Failed to generate PDF: {}", e);
            return HttpResponse::InternalServerError().body("Failed to generate PDF");
        }
    };

    log::info!("Generated PDF: {} bytes", pdf_data.len());

    // Return PDF response
    HttpResponse::Ok()
        .content_type("application/pdf")
        .insert_header((
            "Content-Disposition",
            "attachment; filename=\"example.pdf\"",
        ))
        .body(pdf_data)
}

/// Handler that returns pool statistics.
async fn pool_stats(pool: web::Data<SharedBrowserPool>) -> impl Responder {
    let pool_guard = match pool.lock() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("Pool lock failed"),
    };

    let stats = pool_guard.stats();

    HttpResponse::Ok().json(serde_json::json!({
        "available": stats.available,
        "active": stats.active,
        "total": stats.total
    }))
}

/// Health check endpoint.
async fn health() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Actix-web example...");

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
    let shared_pool = Arc::new(std::sync::Mutex::new(pool));
    let shutdown_pool = Arc::clone(&shared_pool);

    log::info!("Starting server on http://localhost:8080");

    // Start server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(Arc::clone(&shared_pool)))
            .route("/pdf", web::get().to(generate_pdf))
            .route("/stats", web::get().to(pool_stats))
            .route("/health", web::get().to(health))
    })
    .bind("127.0.0.1:8080")?
    .run();

    let result = server.await;

    // Cleanup pool
    log::info!("Server stopped, cleaning up browser pool...");
    if let Ok(mut pool) = shutdown_pool.lock() {
        // Note: In real async context, you'd want to use a runtime
        // For simplicity, we use sync shutdown here
        pool.shutdown();
    }
    log::info!("Cleanup complete");

    result
}
