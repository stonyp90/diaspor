//! Tier Sync Providers - Pluggable providers for moving data between storage tiers
//!
//! Providers can use different technologies:
//! - AWS DataSync for FSx ONTAP ↔ S3
//! - Direct copy for same-storage tier changes
//! - Custom providers via plugins

pub mod datasync;
pub mod provider_registry;

pub use provider_registry::{TierSyncProvider, ProviderRegistry, ProviderConfig};
