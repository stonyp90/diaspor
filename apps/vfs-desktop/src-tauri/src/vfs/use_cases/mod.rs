//! Use Cases Module
//!
//! All use cases are organized in this folder following Clean Architecture principles.
//! Each use case encapsulates a single business operation with clear input/output contracts.

pub mod clipboard;
pub mod move_ops;
pub mod list_files;
pub mod hydrate;
pub mod mount;
pub mod gpu;
pub mod metrics;
pub mod settings;
pub mod ai;
pub mod transcription;

// Re-export commonly used use cases
pub use clipboard::*;
pub use move_ops::*;
pub use list_files::*;
pub use hydrate::*;
pub use mount::*;
pub use gpu::*;
pub use metrics::*;
pub use settings::*;
pub use ai::*;
pub use transcription::*;