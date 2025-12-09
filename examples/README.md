# Examples

This directory contains complete working examples for each supported web framework.

## Prerequisites

1. **Chrome/Chromium** must be installed on your system
2. **Rust 1.85+** must be installed

## Running Examples

### Actix-web Example

```bash
cargo run --example actix_web_example --features actix-integration
```

Then visit: http://localhost:8080/pdf

### Rocket Example

```bash
cargo run --example rocket_example --features rocket-integration
```

Then visit: http://localhost:8000/pdf

### Axum Example

```bash
cargo run --example axum_example --features axum-integration
```

Then visit: http://localhost:3000/pdf

## What the Examples Do

Each example:

1. Creates a browser pool with 3 browsers
2. Warms up the pool
3. Starts a web server with a `/pdf` endpoint
4. When you visit `/pdf`, it:
   - Gets a browser from the pool
   - Navigates to https://example.com
   - Generates a PDF
   - Returns the PDF as a download
5. Handles graceful shutdown on Ctrl+C

## Customizing

You can modify the examples to:

- Change the URL being converted to PDF
- Adjust pool configuration (size, TTL, etc.)
- Add more endpoints
- Integrate with your existing application