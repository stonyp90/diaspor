//! Comprehensive Move Use Case Tests
//!
//! Tests for move operations within VFS, between VFS sources, and between VFS and native FS

#[cfg(test)]
mod tests {
    use crate::vfs::use_cases::move_ops::*;
    use crate::vfs::ports::{IFileOperationsProvider, FileStat};
    use crate::vfs::ports::CopyOptions;
    use std::path::PathBuf;
    use std::sync::Arc;
    use anyhow::Result;
    use std::collections::HashMap;

    // Mock file operations provider for testing
    struct MockFileOpsProvider {
        files: Arc<tokio::sync::RwLock<HashMap<String, Vec<u8>>>>,
    }

    #[async_trait::async_trait]
    impl IFileOperationsProvider for MockFileOpsProvider {
        async fn get_file_ops(&self, _source_id: &str) -> Result<Arc<dyn crate::vfs::ports::IFileOperations>> {
            // Return a mock implementation
            todo!("Mock IFileOperations implementation needed")
        }

        async fn mkdir_p(&self, _source_id: &str, _path: &std::path::Path) -> Result<()> {
            Ok(())
        }

        async fn write(&self, _source_id: &str, path: &std::path::Path, data: &[u8]) -> Result<()> {
            self.files.write().await.insert(path.to_string_lossy().to_string(), data.to_vec());
            Ok(())
        }

        async fn read(&self, _source_id: &str, path: &std::path::Path) -> Result<Vec<u8>> {
            let files = self.files.read().await;
            Ok(files.get(&path.to_string_lossy().to_string()).cloned().unwrap_or_default())
        }

        async fn stat(&self, _source_id: &str, path: &std::path::Path) -> Result<FileStat> {
            let files = self.files.read().await;
            let exists = files.contains_key(&path.to_string_lossy().to_string());
            Ok(FileStat {
                size: if exists { 100 } else { 0 },
                is_dir: false,
                is_file: exists,
                is_symlink: false,
                mtime: None,
                atime: None,
                ctime: None,
                mode: 0o644,
                nlink: 1,
                uid: 0,
                gid: 0,
                blksize: 4096,
                blocks: 0,
            })
        }

        async fn list_files(&self, _source_id: &str, _path: &std::path::Path) -> Result<Vec<crate::vfs::ports::FileEntry>> {
            Ok(vec![])
        }

        async fn copy(
            &self,
            _source_id: &str,
            _from: &std::path::Path,
            _to: &std::path::Path,
            _options: CopyOptions,
        ) -> Result<()> {
            Ok(())
        }

        async fn copy_to_source(
            &self,
            _src_source_id: &str,
            _from: &std::path::Path,
            _dest_source_id: &str,
            _to: &std::path::Path,
        ) -> Result<()> {
            Ok(())
        }

        async fn rm(&self, _source_id: &str, path: &std::path::Path) -> Result<()> {
            self.files.write().await.remove(&path.to_string_lossy().to_string());
            Ok(())
        }

        async fn rm_rf(&self, _source_id: &str, path: &std::path::Path) -> Result<()> {
            self.files.write().await.remove(&path.to_string_lossy().to_string());
            Ok(())
        }

        async fn exists(&self, _source_id: &str, path: &std::path::Path) -> Result<bool> {
            let files = self.files.read().await;
            Ok(files.contains_key(&path.to_string_lossy().to_string()))
        }
    }

    // ============================================================================
    // Move Within VFS Use Case Tests
    // ============================================================================

    #[tokio::test]
    async fn test_move_within_vfs_use_case_same_path() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveWithinVfsUseCase::new(provider);
        
        let input = MoveWithinVfsInput {
            source_id: "test-source".to_string(),
            from_path: PathBuf::from("/test/file.txt"),
            to_path: PathBuf::from("/test/file.txt"), // Same path
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("same"));
    }

    #[tokio::test]
    async fn test_move_within_vfs_use_case_empty_source_id() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveWithinVfsUseCase::new(provider);
        
        let input = MoveWithinVfsInput {
            source_id: String::new(), // Empty
            from_path: PathBuf::from("/test/file.txt"),
            to_path: PathBuf::from("/dest/file.txt"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_move_within_vfs_use_case_source_not_exists() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveWithinVfsUseCase::new(provider);
        
        let input = MoveWithinVfsInput {
            source_id: "test-source".to_string(),
            from_path: PathBuf::from("/nonexistent/file.txt"),
            to_path: PathBuf::from("/dest/file.txt"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not exist"));
    }

    // ============================================================================
    // Move Between VFS Sources Use Case Tests
    // ============================================================================

    #[tokio::test]
    async fn test_move_between_vfs_use_case_same_source() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveBetweenVfsUseCase::new(provider);
        
        let input = MoveBetweenVfsInput {
            src_source_id: "same-source".to_string(),
            from_path: PathBuf::from("/test/file.txt"),
            dest_source_id: "same-source".to_string(), // Same source
            to_path: PathBuf::from("/dest/file.txt"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("MoveWithinVfsUseCase"));
    }

    #[tokio::test]
    async fn test_move_between_vfs_use_case_empty_source_ids() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveBetweenVfsUseCase::new(provider);
        
        let input = MoveBetweenVfsInput {
            src_source_id: String::new(), // Empty
            from_path: PathBuf::from("/test/file.txt"),
            dest_source_id: "dest-source".to_string(),
            to_path: PathBuf::from("/dest/file.txt"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    // ============================================================================
    // Move VFS to Native Use Case Tests
    // ============================================================================

    #[tokio::test]
    async fn test_move_vfs_to_native_use_case_relative_path() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveVfsToNativeUseCase::new(provider);
        
        let input = MoveVfsToNativeInput {
            source_id: "test-source".to_string(),
            vfs_path: PathBuf::from("/test/file.txt"),
            native_path: PathBuf::from("relative/path"), // Not absolute
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("absolute"));
    }

    #[tokio::test]
    async fn test_move_vfs_to_native_use_case_empty_source_id() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveVfsToNativeUseCase::new(provider);
        
        let input = MoveVfsToNativeInput {
            source_id: String::new(), // Empty
            vfs_path: PathBuf::from("/test/file.txt"),
            native_path: PathBuf::from("/tmp/dest"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_move_vfs_to_native_use_case_source_not_exists() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveVfsToNativeUseCase::new(provider);
        
        let input = MoveVfsToNativeInput {
            source_id: "test-source".to_string(),
            vfs_path: PathBuf::from("/nonexistent/file.txt"),
            native_path: PathBuf::from("/tmp/dest"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not exist"));
    }

    // ============================================================================
    // Move Native to VFS Use Case Tests
    // ============================================================================

    #[tokio::test]
    async fn test_move_native_to_vfs_use_case_relative_path() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveNativeToVfsUseCase::new(provider);
        
        let input = MoveNativeToVfsInput {
            native_path: PathBuf::from("relative/path"), // Not absolute
            dest_source_id: "dest-source".to_string(),
            vfs_path: PathBuf::from("/dest/file.txt"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("absolute"));
    }

    #[tokio::test]
    async fn test_move_native_to_vfs_use_case_empty_dest_source_id() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveNativeToVfsUseCase::new(provider);
        
        let input = MoveNativeToVfsInput {
            native_path: PathBuf::from("/tmp/source"),
            dest_source_id: String::new(), // Empty
            vfs_path: PathBuf::from("/dest/file.txt"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_move_native_to_vfs_use_case_source_not_exists() {
        let provider = Arc::new(MockFileOpsProvider {
            files: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        });
        
        let use_case = MoveNativeToVfsUseCase::new(provider);
        
        let input = MoveNativeToVfsInput {
            native_path: PathBuf::from("/nonexistent/file.txt"),
            dest_source_id: "dest-source".to_string(),
            vfs_path: PathBuf::from("/dest/file.txt"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not exist"));
    }
}
