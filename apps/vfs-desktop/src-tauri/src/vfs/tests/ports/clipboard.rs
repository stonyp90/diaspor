//! Clipboard Port Tests
//!
//! Tests for IClipboardService trait contract and value objects.

#[cfg(test)]
mod tests {
    use crate::vfs::ports::{ClipboardContent, ClipboardOperation, ClipboardSource, PasteResult};
    use std::path::PathBuf;
    
    #[test]
    fn test_clipboard_content_copy() {
        let content = ClipboardContent::copy(
            ClipboardSource::Native,
            vec![PathBuf::from("/Users/test/file.txt")],
        );
        
        assert_eq!(content.operation, ClipboardOperation::Copy);
        assert!(content.is_native());
        assert!(!content.is_cut());
    }
    
    #[test]
    fn test_clipboard_content_cut() {
        let content = ClipboardContent::cut(
            ClipboardSource::Vfs { source_id: "s3-bucket".to_string() },
            vec![PathBuf::from("/videos/clip.mp4")],
        );
        
        assert_eq!(content.operation, ClipboardOperation::Cut);
        assert!(content.is_vfs());
        assert!(content.is_cut());
    }
    
    #[test]
    fn test_paste_result_success() {
        let result = PasteResult::success(vec![
            PathBuf::from("/dest/file1.txt"),
            PathBuf::from("/dest/file2.txt"),
        ]);
        
        assert_eq!(result.files_pasted, 2);
        assert_eq!(result.files_failed, 0);
        assert!(result.errors.is_empty());
    }
}
