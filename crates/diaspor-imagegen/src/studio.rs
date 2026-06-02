//! [`ImageStudio`]: the cost/quality router over registered providers.

use std::sync::Arc;

use crate::domain::{
    CompositeRequest, GenerateRequest, Image, ImageError, Layer, Policy, ProviderProfile, Result,
};
use crate::ports::{ImageCompositor, ImageGenerator};

/// Routes [`GenerateRequest`]s to the best registered [`ImageGenerator`] for a
/// [`Policy`], and composites via the configured [`ImageCompositor`].
pub struct ImageStudio {
    generators: Vec<Arc<dyn ImageGenerator>>,
    compositor: Arc<dyn ImageCompositor>,
}

impl ImageStudio {
    /// Start building a studio.
    #[must_use]
    pub fn builder() -> ImageStudioBuilder {
        ImageStudioBuilder::default()
    }

    /// Profiles of every registered generator (for diagnostics / UIs).
    #[must_use]
    pub fn generator_profiles(&self) -> Vec<ProviderProfile> {
        self.generators.iter().map(|g| g.profile()).collect()
    }

    /// Generators ordered best-first for `policy` — cheapest first for
    /// [`Policy::CostOptimized`], highest-quality first otherwise (within the
    /// budget for [`Policy::Balanced`]). The router both *picks* and *falls
    /// back* down this order.
    fn ranked_generators(&self, policy: &Policy) -> Vec<&Arc<dyn ImageGenerator>> {
        let mut ranked: Vec<&Arc<dyn ImageGenerator>> = match policy {
            Policy::Balanced { max_cost_usd } => self
                .generators
                .iter()
                .filter(|g| g.profile().cost_usd_per_image <= *max_cost_usd)
                .collect(),
            Policy::CostOptimized | Policy::QualityFirst => self.generators.iter().collect(),
        };
        ranked.sort_by(|a, b| {
            let (pa, pb) = (a.profile(), b.profile());
            match policy {
                Policy::CostOptimized => pa
                    .cost_usd_per_image
                    .total_cmp(&pb.cost_usd_per_image)
                    .then(pb.quality.cmp(&pa.quality)),
                Policy::QualityFirst | Policy::Balanced { .. } => pb
                    .quality
                    .cmp(&pa.quality)
                    .then(pa.cost_usd_per_image.total_cmp(&pb.cost_usd_per_image)),
            }
        });
        ranked
    }

    /// The single best generator for `policy` (the head of the ranking).
    ///
    /// Pure and deterministic — the routing-order logic under test.
    ///
    /// # Errors
    /// Returns [`ImageError::NoProvider`] when no registered generator
    /// satisfies the policy (e.g. all exceed a [`Policy::Balanced`] budget).
    pub fn select_generator(&self, policy: &Policy) -> Result<&Arc<dyn ImageGenerator>> {
        self.ranked_generators(policy)
            .into_iter()
            .next()
            .ok_or_else(|| ImageError::NoProvider(policy.to_string()))
    }

    /// Generate one image, trying the policy-ranked providers in order and
    /// falling back to the next when one errors (e.g. an unfunded provider
    /// returning HTTP 429). Returns the last error if every provider fails.
    ///
    /// # Errors
    /// [`ImageError::NoProvider`] when nothing satisfies the policy, otherwise
    /// the final provider's error after all have failed.
    pub async fn generate(&self, request: &GenerateRequest, policy: &Policy) -> Result<Image> {
        let ranked = self.ranked_generators(policy);
        if ranked.is_empty() {
            return Err(ImageError::NoProvider(policy.to_string()));
        }
        let mut last_error = None;
        for generator in ranked {
            match generator.generate(request).await {
                Ok(image) => return Ok(image),
                Err(error) => {
                    let provider = generator.profile().name;
                    tracing::warn!(
                        %provider,
                        %error,
                        "image generator failed; falling back to the next provider"
                    );
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| ImageError::NoProvider(policy.to_string())))
    }

    /// Composite pre-made layers with the configured compositor.
    ///
    /// # Errors
    /// Propagates compositor errors.
    pub async fn composite(&self, request: &CompositeRequest) -> Result<Image> {
        self.compositor.composite(request).await
    }

    /// Full pipeline: generate one image per prompt with the policy-selected
    /// provider, then composite them (bottom-first) onto a `width`×`height`
    /// canvas using `instruction`.
    ///
    /// # Errors
    /// Returns [`ImageError::InvalidRequest`] for an empty prompt list, and
    /// propagates routing / provider / compositor errors.
    pub async fn generate_and_compose(
        &self,
        prompts: &[GenerateRequest],
        instruction: impl Into<String> + Send,
        width: u32,
        height: u32,
        policy: &Policy,
    ) -> Result<Image> {
        if prompts.is_empty() {
            return Err(ImageError::InvalidRequest("no prompts to generate".into()));
        }
        let mut layers = Vec::with_capacity(prompts.len());
        for prompt in prompts {
            layers.push(Layer::new(self.generate(prompt, policy).await?));
        }
        self.compositor
            .composite(&CompositeRequest::new(instruction, layers, width, height))
            .await
    }
}

/// Builder for [`ImageStudio`].
#[derive(Default)]
pub struct ImageStudioBuilder {
    generators: Vec<Arc<dyn ImageGenerator>>,
    compositor: Option<Arc<dyn ImageCompositor>>,
}

impl ImageStudioBuilder {
    /// Register a generator. Call repeatedly to give the router choices.
    #[must_use]
    pub fn generator(mut self, generator: Arc<dyn ImageGenerator>) -> Self {
        self.generators.push(generator);
        self
    }

    /// Set the compositor (required).
    #[must_use]
    pub fn compositor(mut self, compositor: Arc<dyn ImageCompositor>) -> Self {
        self.compositor = Some(compositor);
        self
    }

    /// Finish building.
    ///
    /// # Errors
    /// Returns [`ImageError::NoProvider`] if no generator or no compositor was
    /// supplied.
    pub fn build(self) -> Result<ImageStudio> {
        if self.generators.is_empty() {
            return Err(ImageError::NoProvider("no generators registered".into()));
        }
        let compositor = self
            .compositor
            .ok_or_else(|| ImageError::NoProvider("no compositor registered".into()))?;
        Ok(ImageStudio {
            generators: self.generators,
            compositor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Arc, ImageStudio, Policy, ProviderProfile};
    use crate::domain::{
        CompositeRequest, GenerateRequest, Image, ImageError, ImageFormat, Result,
    };
    use crate::ports::{ImageCompositor, ImageGenerator};
    use async_trait::async_trait;

    fn profile(name: &str, quality: u8, cost: f64) -> ProviderProfile {
        ProviderProfile {
            name: name.into(),
            quality,
            cost_usd_per_image: cost,
            supports_compose: false,
            offline: cost == 0.0,
        }
    }

    struct FakeGen(ProviderProfile);
    #[async_trait]
    impl ImageGenerator for FakeGen {
        fn profile(&self) -> ProviderProfile {
            self.0.clone()
        }
        async fn generate(&self, request: &GenerateRequest) -> Result<Image> {
            Ok(Image {
                bytes: Vec::new(),
                format: ImageFormat::Png,
                width: request.width,
                height: request.height,
            })
        }
    }

    /// A generator that always errors — used to exercise router fallback.
    struct FailingGen(ProviderProfile);
    #[async_trait]
    impl ImageGenerator for FailingGen {
        fn profile(&self) -> ProviderProfile {
            self.0.clone()
        }
        async fn generate(&self, _request: &GenerateRequest) -> Result<Image> {
            Err(ImageError::Provider {
                provider: self.0.name.clone(),
                message: "simulated provider failure".into(),
            })
        }
    }

    struct FakeCompositor;
    #[async_trait]
    impl ImageCompositor for FakeCompositor {
        fn profile(&self) -> ProviderProfile {
            profile("compositor", 50, 0.0)
        }
        async fn composite(&self, request: &CompositeRequest) -> Result<Image> {
            Ok(Image {
                bytes: Vec::new(),
                format: ImageFormat::Png,
                width: request.width,
                height: request.height,
            })
        }
    }

    fn studio(profiles: Vec<ProviderProfile>) -> ImageStudio {
        let mut builder = ImageStudio::builder().compositor(Arc::new(FakeCompositor));
        for p in profiles {
            builder = builder.generator(Arc::new(FakeGen(p)));
        }
        builder.build().expect("valid studio")
    }

    #[tokio::test]
    async fn generate_falls_back_when_the_top_provider_fails() {
        // QualityFirst ranks the q95 provider first; it always errors, so the
        // router must fall back to the working q80 provider.
        let studio = ImageStudio::builder()
            .generator(Arc::new(FailingGen(profile("flaky-premium", 95, 0.080))))
            .generator(Arc::new(FakeGen(profile("reliable", 80, 0.020))))
            .compositor(Arc::new(FakeCompositor))
            .build()
            .expect("valid studio");
        let image = studio
            .generate(&GenerateRequest::new("x", 8, 8), &Policy::QualityFirst)
            .await
            .expect("falls back to the working provider");
        assert_eq!((image.width, image.height), (8, 8));
    }

    #[tokio::test]
    async fn generate_errors_when_every_provider_fails() {
        let studio = ImageStudio::builder()
            .generator(Arc::new(FailingGen(profile("a", 95, 0.080))))
            .generator(Arc::new(FailingGen(profile("b", 50, 0.010))))
            .compositor(Arc::new(FakeCompositor))
            .build()
            .expect("valid studio");
        assert!(matches!(
            studio
                .generate(&GenerateRequest::new("x", 8, 8), &Policy::QualityFirst)
                .await,
            Err(ImageError::Provider { .. })
        ));
    }

    #[test]
    fn cost_optimized_picks_cheapest() {
        let s = studio(vec![
            profile("premium", 95, 0.080),
            profile("cheap", 60, 0.002),
            profile("local", 30, 0.0),
        ]);
        assert_eq!(
            s.select_generator(&Policy::CostOptimized)
                .unwrap()
                .profile()
                .name,
            "local"
        );
    }

    #[test]
    fn quality_first_picks_best() {
        let s = studio(vec![
            profile("premium", 95, 0.080),
            profile("cheap", 60, 0.002),
        ]);
        assert_eq!(
            s.select_generator(&Policy::QualityFirst)
                .unwrap()
                .profile()
                .name,
            "premium"
        );
    }

    #[test]
    fn balanced_picks_best_under_budget() {
        let s = studio(vec![
            profile("premium", 95, 0.080),
            profile("mid", 75, 0.010),
            profile("local", 30, 0.0),
        ]);
        assert_eq!(
            s.select_generator(&Policy::Balanced { max_cost_usd: 0.02 })
                .unwrap()
                .profile()
                .name,
            "mid"
        );
    }

    #[test]
    fn balanced_errors_when_all_exceed_budget() {
        let s = studio(vec![profile("premium", 95, 0.080)]);
        assert!(matches!(
            s.select_generator(&Policy::Balanced { max_cost_usd: 0.01 }),
            Err(ImageError::NoProvider(_))
        ));
    }

    #[test]
    fn build_requires_a_generator() {
        assert!(
            ImageStudio::builder()
                .compositor(Arc::new(FakeCompositor))
                .build()
                .is_err()
        );
    }

    #[tokio::test]
    async fn generate_routes_through_selected_provider() {
        let s = studio(vec![
            profile("premium", 95, 0.080),
            profile("local", 30, 0.0),
        ]);
        let img = s
            .generate(&GenerateRequest::new("x", 16, 16), &Policy::CostOptimized)
            .await
            .unwrap();
        assert_eq!((img.width, img.height), (16, 16));
    }
}
