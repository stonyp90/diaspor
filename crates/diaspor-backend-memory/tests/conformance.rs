//! Integration test: run the workspace-wide [`diaspor_conformance`] suite against
//! [`MemoryBackend`]. If this passes, the in-memory backend is observably indistinguishable
//! from any other compliant backend through the public [`diaspor_core::VfsBackend`]
//! trait.

use diaspor_backend_memory::MemoryBackend;

#[tokio::test]
async fn memory_backend_passes_conformance() {
    let backend = MemoryBackend::new();
    diaspor_conformance::run(backend).await;
}
