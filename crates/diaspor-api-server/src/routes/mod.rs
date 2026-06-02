//! Route handler modules.
//!
//! One file per logical surface so `lib.rs::build_router` stays a flat
//! readable mapping from URL to handler. Cross-cutting concerns (auth,
//! metering, attestation) live next to the routes that need them in
//! the form of helper functions, not as a maze of tower middleware.

pub mod analyze;
pub mod health;
pub mod images;
pub mod modality;
pub mod stream;
pub mod train;
