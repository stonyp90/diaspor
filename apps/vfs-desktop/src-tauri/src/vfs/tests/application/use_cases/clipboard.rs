//! Comprehensive Clipboard Use Case Tests
//!
//! Tests for copy, cut, and paste operations between VFS and native filesystem

#[cfg(test)]
mod tests {
    use crate::vfs::use_cases::clipboard::*;
    use crate::vfs::ports::clipboard::{ClipboardSource, IClipboardService, ClipboardContent, PasteResult};
    use std::path::PathBuf;
    use std::sync::Arc;
    

    // Mock clipboard service for testing
    struct MockClipboardService {
        clipboard: Arc<tokio::sync::RwLock<Option<ClipboardContent>>>,
    }

    #[async_trait::async_trait]
    impl IClipboardService for MockClipboardService {
        async fn copy_files(&self, source: ClipboardSource, paths: Vec<PathBuf>) -> anyhow::Result<()> {
            let content = ClipboardContent::copy(source, paths);
            *self.clipboard.write().await = Some(content);
            Ok(())
        }

        async fn cut_files(&self, source: ClipboardSource, paths: Vec<PathBuf>) -> anyhow::Result<()> {
            let content = ClipboardContent::cut(source, paths);
            *self.clipboard.write().await = Some(content);
            Ok(())
        }

        async fn get_clipboard(&self) -> anyhow::Result<Option<ClipboardContent>> {
            Ok(self.clipboard.read().await.clone())
        }

        async fn clear_clipboard(&self) -> anyhow::Result<()> {
            *self.clipboard.write().await = None;
            Ok(())
        }

        async fn has_files(&self) -> anyhow::Result<bool> {
            Ok(self.clipboard.read().await.as_ref().map(|c| !c.paths.is_empty()).unwrap_or(false))
        }

        async fn paste_to_vfs(
            &self,
            _dest_source_id: &str,
            _dest_path: &std::path::Path,
        ) -> anyhow::Result<PasteResult> {
            let content = self.clipboard.read().await.clone();
            if let Some(ref c) = content {
                Ok(PasteResult::success(c.paths.clone()))
            } else {
                Ok(PasteResult::success(vec![]))
            }
        }

        async fn paste_to_native(
            &self,
            _dest_path: &std::path::Path,
        ) -> anyhow::Result<PasteResult> {
            let content = self.clipboard.read().await.clone();
            if let Some(ref c) = content {
                Ok(PasteResult::success(c.paths.clone()))
            } else {
                Ok(PasteResult::success(vec![]))
            }
        }

        async fn read_native_clipboard(&self) -> anyhow::Result<Option<Vec<PathBuf>>> {
            Ok(self.clipboard.read().await.as_ref().map(|c| c.paths.clone()))
        }

        async fn write_native_clipboard(&self, paths: &[PathBuf]) -> anyhow::Result<()> {
            let content = ClipboardContent::copy(ClipboardSource::Native, paths.to_vec());
            *self.clipboard.write().await = Some(content);
            Ok(())
        }
    }

    // ============================================================================
    // Copy Files Use Case Tests
    // ============================================================================

    #[tokio::test]
    async fn test_copy_files_use_case_success() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(None)),
        });
        
        let use_case = CopyFilesUseCase::new(clipboard.clone());
        
        let input = CopyFilesInput {
            source: ClipboardSource::Vfs { source_id: "test-source".to_string() },
            paths: vec![PathBuf::from("/test/file1.txt"), PathBuf::from("/test/file2.txt")],
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_ok());
        
        let output = result.unwrap();
        assert_eq!(output.files_copied, 2);
        
        // Verify clipboard has content
        let has_files = clipboard.has_files().await.unwrap();
        assert!(has_files);
    }

    #[tokio::test]
    async fn test_copy_files_use_case_empty_paths() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(None)),
        });
        
        let use_case = CopyFilesUseCase::new(clipboard);
        
        let input = CopyFilesInput {
            source: ClipboardSource::Native,
            paths: vec![],
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No files to copy"));
    }

    #[tokio::test]
    async fn test_copy_files_use_case_native_source() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(None)),
        });
        
        let use_case = CopyFilesUseCase::new(clipboard.clone());
        
        let input = CopyFilesInput {
            source: ClipboardSource::Native,
            paths: vec![PathBuf::from("/Users/test/file.txt")],
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_ok());
        
        let content = clipboard.get_clipboard().await.unwrap().unwrap();
        assert!(content.is_native());
        assert!(!content.is_cut());
    }

    // ============================================================================
    // Cut Files Use Case Tests
    // ============================================================================

    #[tokio::test]
    async fn test_cut_files_use_case_success() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(None)),
        });
        
        let use_case = CutFilesUseCase::new(clipboard.clone());
        
        let input = CutFilesInput {
            source: ClipboardSource::Native,
            paths: vec![PathBuf::from("/test/file.txt")],
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_ok());
        
        let output = result.unwrap();
        assert_eq!(output.files_cut, 1);
        
        // Verify clipboard has cut content
        let content = clipboard.get_clipboard().await.unwrap().unwrap();
        assert!(content.is_cut());
    }

    #[tokio::test]
    async fn test_cut_files_use_case_empty_paths() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(None)),
        });
        
        let use_case = CutFilesUseCase::new(clipboard);
        
        let input = CutFilesInput {
            source: ClipboardSource::Vfs { source_id: "test".to_string() },
            paths: vec![],
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No files to cut"));
    }

    #[tokio::test]
    async fn test_cut_files_use_case_vfs_source() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(None)),
        });
        
        let use_case = CutFilesUseCase::new(clipboard.clone());
        
        let input = CutFilesInput {
            source: ClipboardSource::Vfs { source_id: "s3-bucket".to_string() },
            paths: vec![PathBuf::from("/videos/clip.mp4")],
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_ok());
        
        let content = clipboard.get_clipboard().await.unwrap().unwrap();
        assert!(content.is_vfs());
        assert!(content.is_cut());
    }

    // ============================================================================
    // Paste to VFS Use Case Tests
    // ============================================================================

    #[tokio::test]
    async fn test_paste_to_vfs_use_case_empty_clipboard() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(None)),
        });
        
        let use_case = PasteToVfsUseCase::new(clipboard);
        
        let input = PasteToVfsInput {
            dest_source_id: "dest-source".to_string(),
            dest_path: PathBuf::from("/dest"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Clipboard is empty"));
    }

    #[tokio::test]
    async fn test_paste_to_vfs_use_case_empty_source_id() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(Some(
                ClipboardContent::copy(ClipboardSource::Native, vec![PathBuf::from("/test/file.txt")])
            ))),
        });
        
        let use_case = PasteToVfsUseCase::new(clipboard);
        
        let input = PasteToVfsInput {
            dest_source_id: String::new(), // Empty
            dest_path: PathBuf::from("/dest"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_paste_to_vfs_use_case_success() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(Some(
                ClipboardContent::copy(
                    ClipboardSource::Vfs { source_id: "src-source".to_string() },
                    vec![PathBuf::from("/src/file1.txt"), PathBuf::from("/src/file2.txt")]
                )
            ))),
        });
        
        let use_case = PasteToVfsUseCase::new(clipboard);
        
        let input = PasteToVfsInput {
            dest_source_id: "dest-source".to_string(),
            dest_path: PathBuf::from("/dest"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_ok());
        
        let output = result.unwrap();
        assert_eq!(output.result.files_pasted, 2);
        assert_eq!(output.result.files_failed, 0);
    }

    // ============================================================================
    // Paste to Native Use Case Tests
    // ============================================================================

    #[tokio::test]
    async fn test_paste_to_native_use_case_relative_path() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(Some(
                ClipboardContent::copy(ClipboardSource::Native, vec![PathBuf::from("/test/file.txt")])
            ))),
        });
        
        let use_case = PasteToNativeUseCase::new(clipboard);
        
        let input = PasteToNativeInput {
            dest_path: PathBuf::from("relative/path"), // Not absolute
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("absolute"));
    }

    #[tokio::test]
    async fn test_paste_to_native_use_case_empty_clipboard() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(None)),
        });
        
        let use_case = PasteToNativeUseCase::new(clipboard);
        
        let input = PasteToNativeInput {
            dest_path: PathBuf::from("/tmp/dest"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Clipboard is empty"));
    }

    #[tokio::test]
    async fn test_paste_to_native_use_case_success() {
        let clipboard = Arc::new(MockClipboardService {
            clipboard: Arc::new(tokio::sync::RwLock::new(Some(
                ClipboardContent::copy(
                    ClipboardSource::Vfs { source_id: "vfs-source".to_string() },
                    vec![PathBuf::from("/vfs/file.txt")]
                )
            ))),
        });
        
        let use_case = PasteToNativeUseCase::new(clipboard);
        
        let input = PasteToNativeInput {
            dest_path: PathBuf::from("/tmp/dest"),
        };
        
        let result = use_case.execute(input).await;
        assert!(result.is_ok());
        
        let output = result.unwrap();
        assert_eq!(output.result.files_pasted, 1);
    }
}
