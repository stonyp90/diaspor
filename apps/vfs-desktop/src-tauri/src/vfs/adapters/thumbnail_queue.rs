//! Thumbnail Queue Manager
//!
//! Manages thumbnail generation with:
//! - Concurrent request limiting (max 3-5 qlmanage processes)
//! - Request queuing
//! - Cache checking before generation
//! - Batch processing

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Semaphore, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{debug, warn};

use crate::vfs::adapters::native_thumbnail::{NativeThumbnailAdapter, ThumbnailType};
use crate::vfs::ports::ThumbnailData;

/// Thumbnail request
#[derive(Debug, Clone)]
pub struct ThumbnailRequest {
    pub path: PathBuf,
    pub size: u32,
    pub thumb_type: ThumbnailType,
}

/// Thumbnail result
#[derive(Debug, Clone)]
pub struct ThumbnailResult {
    pub path: PathBuf,
    pub thumbnail: Option<ThumbnailData>,
    pub error: Option<String>,
}

/// Thumbnail queue manager
pub struct ThumbnailQueue {
    /// Adapter for generating thumbnails
    adapter: Arc<NativeThumbnailAdapter>,
    
    /// Semaphore to limit concurrent thumbnail generation
    semaphore: Arc<Semaphore>,
    
    /// Cache of generated thumbnails (path -> thumbnail data)
    cache: Arc<RwLock<HashMap<PathBuf, ThumbnailData>>>,
    
    /// Cache directory for persistent storage
    cache_dir: PathBuf,
    
    /// Maximum concurrent thumbnail generations
    max_concurrent: usize,
}

impl ThumbnailQueue {
    /// Create a new thumbnail queue
    pub async fn new(cache_dir: PathBuf, max_concurrent: usize) -> Result<Self> {
        let adapter = Arc::new(NativeThumbnailAdapter::new(cache_dir.clone()).await?);
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let cache = Arc::new(RwLock::new(HashMap::new()));
        
        Ok(Self {
            adapter,
            semaphore,
            cache,
            cache_dir,
            max_concurrent,
        })
    }
    
    /// Get thumbnail for a single file (checks cache first)
    pub async fn get_thumbnail(
        &self,
        path: &Path,
        size: u32,
    ) -> Result<Option<ThumbnailData>> {
        // Check in-memory cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(path) {
                debug!("Thumbnail cache hit for: {:?}", path);
                return Ok(Some(cached.clone()));
            }
        }
        
        // Check persistent cache
        if let Some(cached) = self.check_persistent_cache(path, size).await? {
            debug!("Persistent cache hit for: {:?}", path);
            // Store in memory cache
            let mut cache = self.cache.write().await;
            cache.insert(path.to_path_buf(), cached.clone());
            return Ok(Some(cached));
        }
        
        // Need to generate - acquire semaphore permit
        let _permit = self.semaphore.acquire().await
            .context("Failed to acquire semaphore permit")?;
        
        // Double-check cache after acquiring permit (another task might have generated it)
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(path) {
                debug!("Thumbnail cache hit (after permit) for: {:?}", path);
                return Ok(Some(cached.clone()));
            }
        }
        
        // Generate thumbnail
        debug!("Generating thumbnail for: {:?}", path);
        match self.adapter.generate_thumbnail(path, size).await {
            Ok(thumb_data) => {
                // Store in cache
                let mut cache = self.cache.write().await;
                cache.insert(path.to_path_buf(), thumb_data.clone());
                
                // Store in persistent cache
                self.save_to_persistent_cache(path, size, &thumb_data).await.ok();
                
                Ok(Some(thumb_data))
            }
            Err(e) => {
                warn!("Failed to generate thumbnail for {:?}: {}", path, e);
                Ok(None)
            }
        }
    }
    
    /// Process multiple thumbnail requests in batches
    pub async fn get_thumbnails_batch(
        &self,
        requests: Vec<ThumbnailRequest>,
    ) -> Vec<ThumbnailResult> {
        let mut results = Vec::with_capacity(requests.len());
        
        // Process in batches to avoid overwhelming the system
        let batch_size = self.max_concurrent;
        for chunk in requests.chunks(batch_size) {
            let mut batch_results = Vec::new();
            
            // Process batch concurrently (limited by semaphore)
            let mut futures = Vec::new();
            for req in chunk {
                let queue = self.clone();
                let path = req.path.clone();
                let size = req.size;
                
                futures.push(async move {
                    match queue.get_thumbnail(&path, size).await {
                        Ok(thumb) => ThumbnailResult {
                            path,
                            thumbnail: thumb,
                            error: None,
                        },
                        Err(e) => ThumbnailResult {
                            path,
                            thumbnail: None,
                            error: Some(e.to_string()),
                        },
                    }
                });
            }
            
            // Wait for batch to complete
            batch_results.extend(futures::future::join_all(futures).await);
            results.extend(batch_results);
            
            // Small delay between batches to prevent overwhelming the system
            if results.len() < requests.len() {
                sleep(Duration::from_millis(100)).await;
            }
        }
        
        results
    }
    
    /// Check persistent cache for thumbnail
    async fn check_persistent_cache(
        &self,
        path: &Path,
        size: u32,
    ) -> Result<Option<ThumbnailData>> {
        let cache_key = self.get_cache_key(path, size);
        let cache_path = self.cache_dir.join(cache_key);
        
        if cache_path.exists() {
            // Check if cache is still valid (file hasn't been modified)
            if let Ok(file_metadata) = tokio::fs::metadata(path).await {
                if let Ok(file_modified) = file_metadata.modified() {
                    if let Ok(cache_metadata) = tokio::fs::metadata(&cache_path).await {
                        if let Ok(cache_modified) = cache_metadata.modified() {
                            // Cache is valid if it's newer than the file
                            if cache_modified >= file_modified {
                                if let Ok(data) = tokio::fs::read(&cache_path).await {
                                    return Ok(Some(ThumbnailData {
                                        data,
                                        timestamp: 0.0,
                                        width: size,
                                        height: size,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    /// Save thumbnail to persistent cache
    async fn save_to_persistent_cache(
        &self,
        path: &Path,
        size: u32,
        thumb_data: &ThumbnailData,
    ) -> Result<()> {
        let cache_key = self.get_cache_key(path, size);
        let cache_path = self.cache_dir.join(cache_key);
        
        tokio::fs::create_dir_all(&self.cache_dir).await?;
        tokio::fs::write(&cache_path, &thumb_data.data).await?;
        
        Ok(())
    }
    
    /// Generate cache key from path and size
    fn get_cache_key(&self, path: &Path, size: u32) -> String {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        size.hash(&mut hasher);
        
        format!("thumb_{:x}.png", hasher.finish())
    }
    
    /// Clear cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

impl Clone for ThumbnailQueue {
    fn clone(&self) -> Self {
        Self {
            adapter: Arc::clone(&self.adapter),
            semaphore: Arc::clone(&self.semaphore),
            cache: Arc::clone(&self.cache),
            cache_dir: self.cache_dir.clone(),
            max_concurrent: self.max_concurrent,
        }
    }
}
