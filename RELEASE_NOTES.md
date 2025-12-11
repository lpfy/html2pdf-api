# html2pdf-api v0.1.0

Initial release of html2pdf-api - a thread-safe browser pool for headless Chrome automation.

## Features

- **Connection Pooling** - Reuse browser instances to avoid expensive startup costs
- **Health Monitoring** - Background thread continuously checks browser health
- **TTL Management** - Automatically retires old browsers and creates replacements
- **Race-Free Design** - Careful lock ordering prevents deadlocks
- **Graceful Shutdown** - Clean termination of all background tasks
- **RAII Pattern** - Automatic return of browsers to pool via Drop

## Web Framework Integrations

- Actix-web (`actix-integration` feature)
- Rocket (`rocket-integration` feature)
- Axum (`axum-integration` feature)

## Installation

```toml
[dependencies]
html2pdf-api = "0.1.0"