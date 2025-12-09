# Contributing to html2pdf-api

Thank you for your interest in contributing! This document provides guidelines and information for contributors.

## Code of Conduct

Please be respectful and constructive in all interactions.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/html2pdf-api.git
   cd html2pdf-api
   ```
3. **Create a branch** for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```

## Development Setup

### Prerequisites

- Rust 1.85 or later
- Chrome or Chromium installed
- Git

### Building

```bash
# Build all features
cargo build --all-features

# Run tests
cargo test --all-features

# Run clippy
cargo clippy --all-features -- -D warnings

# Format code
cargo fmt
```

### Running Examples

```bash
# Run Actix-web example
cargo run --example actix_web_example --features actix-integration

# Run Rocket example
cargo run --example rocket_example --features rocket-integration

# Run Axum example
cargo run --example axum_example --features axum-integration
```

## Making Changes

### Code Style

- Follow Rust conventions and idioms
- Use `cargo fmt` before committing
- Ensure `cargo clippy` passes without warnings
- Write documentation for public items
- Include examples in doc comments where appropriate

### Documentation

- All public items must have doc comments (`///`)
- Include `# Example` sections for non-trivial functions
- Document error conditions with `# Errors` section
- Document panics with `# Panics` section

### Testing

- Write unit tests for new functionality
- Integration tests go in `tests/` directory
- Use `MockBrowserFactory` for tests that don't need real Chrome
- Ensure all tests pass: `cargo test --all-features`

### Commit Messages

Follow conventional commits format:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `test`: Test changes
- `refactor`: Code refactoring
- `chore`: Maintenance tasks

Example:
```
feat(pool): add configurable grace period for TTL

Added ttl_grace_period config option to prevent serving browsers
that are about to expire.

Closes #123
```

## Pull Request Process

1. **Update documentation** if you've changed APIs
2. **Add tests** for new functionality
3. **Update CHANGELOG.md** with your changes
4. **Ensure CI passes** - all tests, clippy, and formatting checks
5. **Request review** from maintainers

### PR Title Format

Use the same format as commit messages:
```
feat(pool): add configurable grace period
```

## Feature Flags

When adding new features that require dependencies:

1. Make the feature optional with a feature flag
2. Use `#[cfg(feature = "...")]` for conditional compilation
3. Document the feature in `Cargo.toml` and `README.md`
4. Add the feature to CI test matrix

## Releasing

Releases are handled by maintainers:

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create a git tag: `git tag v0.1.0`
4. Push tag: `git push origin v0.1.0`
5. CI will publish to crates.io

## Questions?

- Open an issue for bugs or feature requests
- Use discussions for questions and ideas

Thank you for contributing! 