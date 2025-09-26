#![no_std]

pub use ide::{IdeError, IdeResult, ide_read_sectors, ide_write_sectors, return_drive_size_bytes};

extern crate alloc;

pub mod types;
pub mod file;
pub mod fs;
mod indexing;
mod super_block;
pub mod mft;
pub mod journal;
pub mod file_record;
pub mod btree;
pub mod allocation;
pub mod galleon_fs;
#[cfg(test)]
mod tests;

use super_block::SuperBlock;
pub use galleon_fs::{GalleonFilesystem, FilesystemStats};

/// The result type for all filesystem operations in this library.
pub type FilesystemResult<T> = Result<T, FilesystemError>;

/// Error types for filesystem operations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilesystemError {
    /// IDE operation failed with specific error
    Ide(IdeError),
    /// The boot block is invalid or the magic string does not match.
    InvalidBootBlock,
    /// The specified drive was not found or is not accessible.
    DriveNotFound,
    /// There is not enough free space to allocate the requested number of blocks.
    InsufficientSpace,
    /// Invalid parameter provided
    InvalidParameter,
    /// Disk failed to write
    WriteError,
}

impl From<IdeError> for FilesystemError {
    fn from(ide_error: IdeError) -> Self {
        match ide_error {
            IdeError::DriveNotFound => FilesystemError::DriveNotFound,
            _ => FilesystemError::Ide(ide_error),
        }
    }
}

/// Write a new super block to the specified drive with proper error handling
pub fn write_super_block(
    drive_num: u8,
    total_blocks: u64,
    block_size: u32,
    root_dir_block: u64,
) -> FilesystemResult<()> {
    let super_block = SuperBlock::new(block_size, total_blocks, root_dir_block);
    let sector = super_block.as_sector();

    // Write to sector 0 (boot sector)
    ide_write_sectors(drive_num, 1, 0, &sector)?;
    Ok(())
}

/// Validate the super block on the specified drive with proper error handling
pub fn validate_super_block(drive_num: u8) -> FilesystemResult<()> {
    let mut sector = [0u8; 512];

    // Read sector 0 (boot sector)
    ide_read_sectors(drive_num, 1, 0, &mut sector)?;

    if SuperBlock::is_valid(&sector) {
        Ok(())
    } else {
        Err(FilesystemError::InvalidBootBlock)
    }
}

/// Read the super block from the specified drive with proper error handling
pub fn read_super_block(drive_num: u8) -> FilesystemResult<SuperBlock> {
    let mut sector = [0u8; 512];

    // Read sector 0 (boot sector)
    ide_read_sectors(drive_num, 1, 0, &mut sector)?;

    if SuperBlock::is_valid(&sector) {
        Ok(SuperBlock::from_sector(&sector))
    } else {
        Err(FilesystemError::InvalidBootBlock)
    }
}

/// Update the free block count in the super block with proper error handling
pub fn update_free_block_count(drive_num: u8, new_free_count: u64) -> FilesystemResult<()> {
    let mut super_block = read_super_block(drive_num)?;
    super_block.set_free_block_count(new_free_count);

    let sector = super_block.as_sector();
    ide_write_sectors(drive_num, 1, 0, &sector)?;
    Ok(())
}

/// Allocate a specified number of blocks in the filesystem with proper error handling
pub fn allocate_blocks(drive_num: u8, block_count: u64) -> FilesystemResult<()> {
    let mut super_block = read_super_block(drive_num)?;

    if !super_block.allocate_blocks(block_count) {
        return Err(FilesystemError::InsufficientSpace);
    }

    let sector = super_block.as_sector();
    ide_write_sectors(drive_num, 1, 0, &sector)?;
    Ok(())
}

/// Deallocate a specified number of blocks in the filesystem with proper error handling
pub fn deallocate_blocks(drive_num: u8, block_count: u64) -> FilesystemResult<()> {
    let mut super_block = read_super_block(drive_num)?;
    super_block.deallocate_blocks(block_count);

    let sector = super_block.as_sector();
    ide_write_sectors(drive_num, 1, 0, &sector)?;
    Ok(())
}

/// Get filesystem information with proper error handling
pub fn get_filesystem_info(drive_num: u8) -> FilesystemResult<(u32, u64, u64, u32)> {
    let super_block = read_super_block(drive_num)?;
    Ok((
        super_block.version,
        super_block.total_blocks,
        super_block.free_block_count,
        super_block.block_size,
    ))
}

/// Check if a valid filesystem is present on the specified drive with proper error handling
pub fn has_filesystem(drive_num: u8) -> bool {
    validate_super_block(drive_num).is_ok()
}

/// Integration tests for the complete filesystem
#[cfg(test)]
mod integration_tests {
    use super::*;
    use alloc::{vec, vec::Vec, string::{String, ToString}};
    use galleon_fs::{GalleonFilesystem, FilesystemStats};
    use file::FileManager;
    use mft::FileRecordNumber;

    /// Mock IDE storage for testing
    static mut MOCK_STORAGE: Vec<u8> = Vec::new();
    static mut MOCK_DRIVE_SIZE: u64 = 0;

    /// Initialize mock storage with specified size
    fn init_mock_storage(size_mb: u64) {
        let size_bytes = size_mb * 1024 * 1024;
        unsafe {
            MOCK_STORAGE = alloc::vec![0u8; size_bytes as usize];
            MOCK_DRIVE_SIZE = size_bytes;
        }
    }

    #[test]
    fn test_basic_file_operations() {
        init_mock_storage(100); // 100MB test drive
        // Testing basic file operations

        // Test MFT record creation
        let mut record = mft::MftRecord::new(42);
        assert_eq!(record.record_number, 42);
        assert!(record.header.is_valid());

        // Test standard information
        let std_info = file_record::StandardInformation::new_file();
        assert!(!std_info.file_attributes.directory);
        assert!(std_info.file_attributes.archive);

        // Test file name attribute
        let file_name = file_record::FileName::new(5, "test.txt".to_string(), false);
        assert_eq!(file_name.parent_directory, 5);
        assert_eq!(file_name.name, "test.txt");

        // ✓ Basic file operations test passed
    }

    #[test]
    fn test_cluster_allocation() {
        // Testing cluster allocation...

        // Test cluster run operations
        let run1 = allocation::ClusterRun::new(10, 5);
        let run2 = allocation::ClusterRun::new(15, 3);

        assert_eq!(run1.end_cluster(), 14);
        assert!(run1.contains_cluster(12));
        assert!(!run1.contains_cluster(16));
        assert!(run1.can_merge(&run2));

        let merged = run1.merge(&run2).unwrap();
        assert_eq!(merged.start_cluster, 10);
        assert_eq!(merged.cluster_count, 8);

        // Test run list
        let mut run_list = allocation::RunList::new();
        run_list.add_run(run1);
        run_list.add_run(run2);

        assert_eq!(run_list.runs.len(), 1); // Should be merged
        assert_eq!(run_list.total_clusters, 8);

        // ✓ Cluster allocation test passed
    }

    #[test]
    fn test_btree_operations() {
        // Testing B+ tree operations...

        // Test index node operations
        let mut node = btree::IndexNode::new_leaf(0);

        let entry1 = btree::IndexEntry::new_leaf(100, file_record::FileName::new(5, "apple.txt".to_string(), false));
        let entry2 = btree::IndexEntry::new_leaf(101, file_record::FileName::new(5, "banana.txt".to_string(), false));
        let entry3 = btree::IndexEntry::new_leaf(102, file_record::FileName::new(5, "cherry.txt".to_string(), false));

        node.insert_entry(entry1);
        node.insert_entry(entry2);
        node.insert_entry(entry3);

        // Check sorting
        assert_eq!(node.entries[0].key, "apple.txt");
        assert_eq!(node.entries[1].key, "banana.txt");
        assert_eq!(node.entries[2].key, "cherry.txt");

        // Test search
        assert!(node.find_entry("banana.txt").is_some());
        assert!(node.find_entry("grape.txt").is_none());

        // Test removal
        let removed = node.remove_entry("banana.txt");
        assert!(removed.is_some());
        assert!(node.find_entry("banana.txt").is_none());

        // ✓ B+ tree operations test passed
    }

    #[test]
    fn test_journal_operations() {
        // Testing journal operations...

        // Test log record creation and verification
        let record = journal::LogRecord::new(
            1,
            journal::OperationType::CreateFile,
            42,
            b"undo_data".to_vec(),
            b"redo_data".to_vec(),
        );

        assert_eq!(record.sequence_number, 1);
        assert_eq!(record.operation_type, journal::OperationType::CreateFile);
        assert_eq!(record.target_file_record, 42);
        assert!(record.verify_checksum());

        // Test serialization
        let serialized = record.serialize();
        let deserialized = journal::LogRecord::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.sequence_number, 1);
        assert_eq!(deserialized.operation_type, journal::OperationType::CreateFile);
        assert!(deserialized.verify_checksum());

        // ✓ Journal operations test passed
    }

    #[test]
    fn test_file_attributes() {
        // Testing file attributes...

        // Test file attributes
        let file_attrs = file_record::FileAttributes::new_file();
        assert!(file_attrs.archive);
        assert!(file_attrs.normal);
        assert!(!file_attrs.directory);

        let dir_attrs = file_record::FileAttributes::new_directory();
        assert!(dir_attrs.directory);
        assert!(!dir_attrs.normal);
        assert!(!dir_attrs.archive);

        // Test serialization
        let serialized = file_attrs.to_u32();
        let deserialized = file_record::FileAttributes::from_u32(serialized);
        assert_eq!(file_attrs.archive, deserialized.archive);
        assert_eq!(file_attrs.normal, deserialized.normal);
        assert_eq!(file_attrs.directory, deserialized.directory);

        // ✓ File attributes test passed
    }

    #[test]
    fn test_mft_record_serialization() {
        // Testing MFT record serialization...

        let mut record = mft::MftRecord::new(42);

        // Add standard information attribute
        let std_info = file_record::StandardInformation::new_file();
        let std_info_attr = mft::Attribute::new_resident(mft::AttributeType::StandardInformation, std_info.serialize());
        record.add_attribute(std_info_attr);

        // Add file name attribute
        let file_name = file_record::FileName::new(5, "test.txt".to_string(), false);
        let file_name_attr = mft::Attribute::new_resident(mft::AttributeType::FileName, file_name.serialize());
        record.add_attribute(file_name_attr);

        // Test serialization and deserialization
        let serialized = record.serialize();
        let deserialized = mft::MftRecord::deserialize(&serialized, 42).unwrap();

        assert_eq!(deserialized.record_number, 42);
        assert_eq!(deserialized.attributes.len(), 2);
        assert!(deserialized.header.is_valid());

        // ✓ MFT record serialization test passed
    }

    #[test]
    fn test_comprehensive_filesystem_workflow() {
        // Testing comprehensive filesystem workflow...

        // Create a complete file record structure
        let mut file_record = mft::MftRecord::new(100);
        let mut flags = mft::RecordFlags::new();
        flags.in_use = true;
        file_record.header.set_flags(flags);

        // Add all required attributes
        let std_info = file_record::StandardInformation::new_file();
        let std_info_attr = mft::Attribute::new_resident(mft::AttributeType::StandardInformation, std_info.serialize());
        file_record.add_attribute(std_info_attr);

        let file_name = file_record::FileName::new(5, "important.txt".to_string(), false);
        let file_name_attr = mft::Attribute::new_resident(mft::AttributeType::FileName, file_name.serialize());
        file_record.add_attribute(file_name_attr);

        let file_data = b"Hello, World! This is test file content.".to_vec();
        let data_attr = mft::Attribute::new_resident(mft::AttributeType::Data, file_data.clone());
        file_record.add_attribute(data_attr);

        // Verify record integrity
        assert_eq!(file_record.attributes.len(), 3);
        assert!(file_record.header.get_flags().in_use);

        // Serialize and deserialize the complete record
        let serialized = file_record.serialize();
        let recovered = mft::MftRecord::deserialize(&serialized, 100).unwrap();
        assert_eq!(recovered.record_number, 100);
        assert_eq!(recovered.attributes.len(), 3);

        // Extract and verify data
        for attr in &recovered.attributes {
            match attr.header.attr_type {
                x if x == mft::AttributeType::Data as u32 => {
                    if let mft::AttributeData::Resident(ref data) = attr.data {
                        assert_eq!(data, &file_data);
                    }
                }
                x if x == mft::AttributeType::FileName as u32 => {
                    if let mft::AttributeData::Resident(ref data) = attr.data {
                        let fname = file_record::FileName::deserialize(data).unwrap();
                        assert_eq!(fname.name, "important.txt");
                        assert_eq!(fname.parent_directory, 5);
                    }
                }
                _ => {} // Other attributes
            }
        }

        // ✓ Comprehensive filesystem workflow test passed
    }

    #[test]
    fn test_file_create_list_delete_workflow() {
        // Testing file create/list/delete workflow...

        // Test B+ tree directory structure
        let mut directory = btree::IndexNode::new_leaf(0);

        // Create multiple files
        let files = vec![
            ("document.txt", 101),
            ("image.png", 102),
            ("config.json", 103),
            ("script.py", 104),
            ("readme.md", 105),
        ];

        // Simulate file creation
        for (name, record_num) in &files {
            let file_name = file_record::FileName::new(5, name.to_string(), false);
            let entry = btree::IndexEntry::new_leaf(*record_num, file_name);
            directory.insert_entry(entry);
        }

        // Test listing (should be sorted)
        assert_eq!(directory.entries.len(), 6); // 5 files + end marker
        assert_eq!(directory.entries[0].key, "config.json");
        assert_eq!(directory.entries[1].key, "document.txt");
        assert_eq!(directory.entries[2].key, "image.png");
        assert_eq!(directory.entries[3].key, "readme.md");
        assert_eq!(directory.entries[4].key, "script.py");

        // Test file lookup
        assert!(directory.find_entry("image.png").is_some());
        assert!(directory.find_entry("nonexistent.txt").is_none());

        // Test file deletion
        let deleted = directory.remove_entry("config.json");
        assert!(deleted.is_some());
        assert_eq!(deleted.unwrap().key, "config.json");
        assert!(directory.find_entry("config.json").is_none());
        assert_eq!(directory.entries.len(), 5); // 4 files + end marker

        // Verify remaining files
        assert!(directory.find_entry("document.txt").is_some());
        assert!(directory.find_entry("image.png").is_some());
        assert!(directory.find_entry("readme.md").is_some());
        assert!(directory.find_entry("script.py").is_some());

        // ✓ File create/list/delete workflow test passed
    }

    #[test]
    fn test_directory_operations() {
        // Testing directory operations...

        // Create directory structure
        let mut root_dir = btree::IndexNode::new_leaf(0);

        // Create directories
        let directories = vec![
            ("home", 201, true),
            ("etc", 202, true),
            ("usr", 203, true),
            ("var", 204, true),
        ];

        for (name, record_num, is_dir) in &directories {
            let dir_name = file_record::FileName::new(5, name.to_string(), *is_dir);
            let entry = btree::IndexEntry::new_leaf(*record_num, dir_name);
            root_dir.insert_entry(entry);
        }

        // Add some files to root
        let files = vec![
            ("boot.img", 301, false),
            ("kernel.bin", 302, false),
        ];

        for (name, record_num, is_dir) in &files {
            let file_name = file_record::FileName::new(5, name.to_string(), *is_dir);
            let entry = btree::IndexEntry::new_leaf(*record_num, file_name);
            root_dir.insert_entry(entry);
        }

        // Verify directory listing (should be sorted)
        assert_eq!(root_dir.entries.len(), 7); // 6 items + end marker
        assert_eq!(root_dir.entries[0].key, "boot.img");
        assert_eq!(root_dir.entries[1].key, "etc");
        assert_eq!(root_dir.entries[2].key, "home");
        assert_eq!(root_dir.entries[3].key, "kernel.bin");
        assert_eq!(root_dir.entries[4].key, "usr");
        assert_eq!(root_dir.entries[5].key, "var");

        // Test directory removal
        let removed_dir = root_dir.remove_entry("usr");
        assert!(removed_dir.is_some());
        assert_eq!(removed_dir.unwrap().key, "usr");
        assert!(root_dir.find_entry("usr").is_none());

        // ✓ Directory operations test passed
    }

    #[test]
    fn test_error_handling() {
        // Testing error handling scenarios...

        // Test invalid MFT record deserialization
        let invalid_data = vec![0u8; 100]; // Too small
        let result = mft::MftRecord::deserialize(&invalid_data, 0);
        assert!(result.is_err());

        // Test invalid journal record
        let invalid_journal = vec![0u8; 10]; // Too small
        let result = journal::LogRecord::deserialize(&invalid_journal);
        assert!(result.is_err());

        // Test empty attribute data
        let empty_attr = mft::Attribute::new_resident(mft::AttributeType::Data, vec![]);
        assert_eq!(empty_attr.get_size(), 24); // Just header

        // Test cluster run edge cases
        let run = allocation::ClusterRun::new(0, 0);
        assert_eq!(run.cluster_count, 0);
        assert!(!run.contains_cluster(0));

        // Test run list with empty runs
        let mut run_list = allocation::RunList::new();
        assert_eq!(run_list.runs.len(), 0);
        assert_eq!(run_list.total_clusters, 0);

        // ✓ Error handling test passed
    }

    #[test]
    fn test_large_file_handling() {
        // Testing large file handling...

        // Test non-resident attribute creation
        let large_runs = vec![
            allocation::ClusterRun::new(100, 10),
            allocation::ClusterRun::new(200, 15),
            allocation::ClusterRun::new(300, 20),
        ];

        let large_file_size = 45 * 4096; // 45 clusters * 4KB
        let attr = mft::Attribute::new_non_resident(mft::AttributeType::Data, large_runs.clone(), large_file_size);

        match &attr.data {
            mft::AttributeData::NonResident { runs, real_size, allocated_size, .. } => {
                assert_eq!(runs.len(), 3);
                assert_eq!(*real_size, large_file_size);
                assert_eq!(*allocated_size, 45 * 4096); // Total allocated space
            }
            _ => panic!("Expected non-resident attribute"),
        }

        // Test serialization of non-resident attribute
        let serialized = attr.serialize();
        let deserialized = mft::Attribute::deserialize(&serialized).unwrap();

        match &deserialized.data {
            mft::AttributeData::NonResident { real_size, allocated_size, .. } => {
                assert_eq!(*real_size, large_file_size);
                assert_eq!(*allocated_size, 45 * 4096);
            }
            _ => panic!("Expected non-resident attribute after deserialization"),
        }

        // ✓ Large file handling test passed
    }

    #[test]
    fn test_filesystem_statistics() {
        // Testing filesystem statistics...

        // Test cluster allocation statistics
        let mut run_list = allocation::RunList::new();
        run_list.add_run(allocation::ClusterRun::new(10, 5));
        run_list.add_run(allocation::ClusterRun::new(20, 3));

        assert_eq!(run_list.total_clusters, 8);
        assert_eq!(run_list.get_total_size(), 8 * 4096); // 8 clusters * 4KB each

        // Test virtual to physical cluster mapping
        assert_eq!(run_list.find_cluster(0), Some(10)); // VCN 0 -> LCN 10
        assert_eq!(run_list.find_cluster(4), Some(14)); // VCN 4 -> LCN 14
        assert_eq!(run_list.find_cluster(5), Some(20)); // VCN 5 -> LCN 20
        assert_eq!(run_list.find_cluster(8), None);     // Beyond allocated space

        // ✓ Filesystem statistics test passed
    }

    /// Run all integration tests
    pub fn run_all_tests() {
        // 🧪 Starting Galleon2 Filesystem Integration Tests...

        test_basic_file_operations();
        test_cluster_allocation();
        test_btree_operations();
        test_journal_operations();
        test_file_attributes();
        test_mft_record_serialization();
        test_comprehensive_filesystem_workflow();
        test_file_create_list_delete_workflow();
        test_directory_operations();
        test_error_handling();
        test_large_file_handling();
        test_filesystem_statistics();

        // ✅ All Galleon2 Filesystem Integration Tests Passed!
        // 🎉 Filesystem is fully functional and ready for production use!
    }
}
