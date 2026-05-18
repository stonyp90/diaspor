//! Value Object Tests
//!
//! Tests for value objects like FileSize, StorageTier, etc.

#[cfg(test)]
mod tests {
    use crate::vfs::domain::value_objects::FileSize;

    #[test]
    fn test_file_size_from_bytes() {
        let size = FileSize::from_bytes(1024);
        assert_eq!(size.bytes(), 1024);
    }

    #[test]
    fn test_file_size_human_readable() {
        let size = FileSize::from_bytes(1024);
        let display = size.as_human_readable();
        assert!(display.contains("KB") || display.contains("1024"));
    }
}
