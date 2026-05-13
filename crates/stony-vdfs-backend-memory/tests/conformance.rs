//! Integration test: run the workspace-wide [`stony_vdfs_conformance`] suite against
//! [`MemoryBackend`]. If this passes, the in-memory backend is observably indistinguishable
//! from any other compliant backend through the public [`stony_vdfs_core::VfsBackend`]
//! trait.

use stony_vdfs_backend_memory::MemoryBackend;

#[tokio::test]
async fn memory_backend_passes_conformance() {
    let backend = MemoryBackend::new();
    stony_vdfs_conformance::run(backend).await;
}
