//! Integration test: run the workspace-wide [`cairn_conformance`] suite against
//! [`MemoryBackend`]. If this passes, the in-memory backend is observably indistinguishable
//! from any other compliant backend through the public [`cairn_core::VfsBackend`]
//! trait.

use cairn_backend_memory::MemoryBackend;

#[tokio::test]
async fn memory_backend_passes_conformance() {
    let backend = MemoryBackend::new();
    cairn_conformance::run(backend).await;
}
