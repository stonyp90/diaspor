//! File Operations Port Tests
//!
//! Tests for IFileOperations trait contract and value objects.

#[cfg(test)]
mod tests {
    use crate::vfs::ports::file_operations::{FileEntry, FileStat, CopyOptions};
    
    #[test]
    fn test_file_entry_default() {
        let entry = FileEntry::default();
        assert!(entry.is_file);
        assert!(!entry.is_dir);
        assert_eq!(entry.size, 0);
    }
    
    #[test]
    fn test_file_stat_default() {
        let stat = FileStat::default();
        assert!(stat.is_file);
        assert!(!stat.is_dir);
        assert_eq!(stat.mode, 0o644);
    }
    
    #[test]
    fn test_copy_options_default() {
        let opts = CopyOptions::default();
        assert!(!opts.overwrite);
        assert!(!opts.recursive);
    }
}
