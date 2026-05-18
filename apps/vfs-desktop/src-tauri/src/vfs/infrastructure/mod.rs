//! Infrastructure Layer - External framework integrations
//!
//! This layer contains implementations that depend on external frameworks,
//! databases, file systems, and other infrastructure concerns.

pub mod state;
pub mod media_state;
pub mod hls_server;
pub mod di;
pub mod storage_persistence;

pub use state::VfsState;
pub use media_state::MediaStateWrapper;
pub use hls_server::{HlsServer, HlsServerConfig};
pub use di::ServiceContainer;
pub use storage_persistence::StoragePersistence;

