//! Application Layer - Business logic orchestration
//!
//! This layer contains the application-specific business rules.
//! It orchestrates the flow of data to and from entities and
//! directs those entities to use their enterprise-wide business rules.

pub mod vfs_service;

pub use vfs_service::VfsService;



