//! Concrete provider adapters.
//!
//! [`LocalImageAdapter`] is always available. The networked adapters
//! (`GeminiImageAdapter`, `AzureOpenAiImageAdapter`) require the `remote`
//! feature.

mod local;
pub use local::LocalImageAdapter;

#[cfg(feature = "remote")]
mod gemini;
#[cfg(feature = "remote")]
pub use gemini::GeminiImageAdapter;

#[cfg(feature = "remote")]
mod azure_openai;
#[cfg(feature = "remote")]
pub use azure_openai::AzureOpenAiImageAdapter;

#[cfg(feature = "remote")]
mod openai;
#[cfg(feature = "remote")]
pub use openai::OpenAiImageAdapter;
