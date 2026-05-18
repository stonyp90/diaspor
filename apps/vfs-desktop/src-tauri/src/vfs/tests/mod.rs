//! VFS Test Suite
//!
//! Tests are organized following Clean Architecture principles, mirroring the source structure:
//!
//! ```
//! tests/
//! ├── domain/          # Domain layer tests (entities, value objects, business rules)
//! ├── ports/           # Port interface tests (contract tests, mock implementations)
//! ├── adapters/        # Adapter implementation tests (concrete implementations)
//! ├── application/     # Application layer tests (use cases, services)
//! ├── infrastructure/  # Infrastructure tests (commands, DI, etc.)
//! └── integration/    # End-to-end integration tests
//! ```
//!
//! ## Test Organization Principles
//!
//! 1. **Domain Tests**: Test business logic, entities, and value objects in isolation
//! 2. **Port Tests**: Test trait contracts and ensure implementations conform
//! 3. **Adapter Tests**: Test concrete implementations against their ports
//! 4. **Application Tests**: Test use cases and service orchestration
//! 5. **Infrastructure Tests**: Test infrastructure concerns (commands, DI, etc.)
//! 6. **Integration Tests**: Test complete workflows end-to-end
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all tests
//! cargo test --lib
//!
//! # Run tests for a specific layer
//! cargo test --lib domain
//! cargo test --lib adapters
//! cargo test --lib application
//!
//! # Run a specific test
//! cargo test --lib test_name
//! ```

// Domain layer tests
#[cfg(test)]
mod domain;

// Port interface tests
#[cfg(test)]
mod ports;

// Adapter implementation tests
#[cfg(test)]
mod adapters;

// Application layer tests
#[cfg(test)]
mod application;

// Infrastructure tests
#[cfg(test)]
mod infrastructure;

// Integration tests
#[cfg(test)]
mod integration;
