# html2pdf-api

> Thread-safe browser pool for HTML to PDF conversion with web framework integration

[![Crates.io](https://img.shields.io/crates/v/html2pdf-api.svg)](https://crates.io/crates/html2pdf-api)
[![Documentation](https://docs.rs/html2pdf-api/badge.svg)](https://docs.rs/html2pdf-api)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/yourusername/html2pdf-api#license)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)

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
