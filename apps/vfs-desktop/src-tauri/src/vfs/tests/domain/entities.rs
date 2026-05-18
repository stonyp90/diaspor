//! Domain Entity Tests
//!
//! Tests for VirtualFile, StorageSource, and other domain entities.

#[cfg(test)]
mod tests {
    use crate::vfs::domain::VirtualFile;
    use std::path::PathBuf;
    

    #[test]
    fn test_virtual_file_creation() {
        let file = VirtualFile::new(
            "test.txt".to_string(),
            PathBuf::from("/test.txt"),
            1024,
            false,
        );
        
        assert_eq!(file.name, "test.txt");
        assert_eq!(file.size.bytes(), 1024);
        assert!(!file.is_directory);
    }

    #[test]
    fn test_virtual_file_directory() {
        let dir = VirtualFile::new(
            "folder".to_string(),
            PathBuf::from("/folder"),
            0,
            true,
        );
        
        assert!(dir.is_directory);
        assert_eq!(dir.size.bytes(), 0);
    }
}
