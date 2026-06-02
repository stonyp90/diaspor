//! The two ports: [`ImageGenerator`] (text → image) and [`ImageCompositor`]
//! (many images → one).

use async_trait::async_trait;

use crate::domain::{CompositeRequest, GenerateRequest, Image, ProviderProfile, Result};

/// A text-to-image provider.
#[async_trait]
pub trait ImageGenerator: Send + Sync {
    /// Static cost/quality profile the router uses to choose this provider.
    fn profile(&self) -> ProviderProfile;

    /// Render `request` into an [`Image`].
    async fn generate(&self, request: &GenerateRequest) -> Result<Image>;
}

/// A provider that merges several images into one.
#[async_trait]
pub trait ImageCompositor: Send + Sync {
    /// Static cost/quality profile.
    fn profile(&self) -> ProviderProfile;

    /// Composite `request.layers` into a single [`Image`].
    async fn composite(&self, request: &CompositeRequest) -> Result<Image>;
}
