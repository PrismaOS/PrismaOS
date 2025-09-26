//! High-level file operations using the new Galleon filesystem
//!
//! This module provides convenient file operations built on top of the
//! new MFT-based filesystem architecture.

use crate::{
    FilesystemResult, FilesystemError,
    galleon_fs::GalleonFilesystem,
    mft::FileRecordNumber,
    types::pathbuf::PathBuf,
};
use alloc::{string::String, vec::Vec};

/// High-level file manager wrapping the Galleon filesystem
pub struct FileManager {
    filesystem: GalleonFilesystem,
    current_directory: FileRecordNumber,
}

impl FileManager {
    /// Create a new file manager with an existing filesystem
    pub fn new(filesystem: GalleonFilesystem) -> Self {
        Self {
            filesystem,
            current_directory: 5, // Root directory record number
        }
    }

    /// Format a new filesystem on the specified drive
    pub fn format_drive(drive: u8) -> FilesystemResult<Self> {
        let filesystem = GalleonFilesystem::format(drive)?;
        Ok(Self::new(filesystem))
    }

    /// Mount an existing filesystem from the specified drive
    pub fn mount_drive(drive: u8) -> FilesystemResult<Self> {
        let filesystem = GalleonFilesystem::mount(drive)?;
        Ok(Self::new(filesystem))
    }

    /// Create a new file in the current directory
    pub fn create_file(&mut self, name: String, contents: Option<String>) -> FilesystemResult<FileRecordNumber> {
        let data = contents.map(|s| s.into_bytes());
        self.filesystem.create_file(self.current_directory, name, data)
    }

    /// Create a new file with binary data
    pub fn create_file_binary(&mut self, name: String, data: Vec<u8>) -> FilesystemResult<FileRecordNumber> {
        self.filesystem.create_file(self.current_directory, name, Some(data))
    }

    /// Create a new directory in the current directory
    pub fn create_directory(&mut self, name: String) -> FilesystemResult<FileRecordNumber> {
        self.filesystem.create_directory(self.current_directory, name)
    }

    /// Read file contents as string
    pub fn read_file_text(&mut self, file_record: FileRecordNumber) -> FilesystemResult<String> {
        let data = self.filesystem.read_file(file_record)?;
        String::from_utf8(data).map_err(|_| FilesystemError::InvalidParameter)
    }

    /// Read file contents as binary data
    pub fn read_file_binary(&mut self, file_record: FileRecordNumber) -> FilesystemResult<Vec<u8>> {
        self.filesystem.read_file(file_record)
    }

    /// Write text data to a file
    pub fn write_file_text(&mut self, file_record: FileRecordNumber, content: String) -> FilesystemResult<()> {
        self.filesystem.write_file(file_record, content.into_bytes())
    }

    /// Write binary data to a file
    pub fn write_file_binary(&mut self, file_record: FileRecordNumber, data: Vec<u8>) -> FilesystemResult<()> {
        self.filesystem.write_file(file_record, data)
    }

    /// Delete a file by name
    pub fn delete_file(&mut self, name: &str) -> FilesystemResult<()> {
        // Find the file first
        if let Some(file_record) = self.filesystem.find_file(name)? {
            self.filesystem.delete_file(file_record, name)
        } else {
            Err(FilesystemError::InvalidParameter) // File not found
        }
    }

    /// List files in the current directory
    pub fn list_files(&self) -> FilesystemResult<Vec<(String, FileRecordNumber)>> {
        self.filesystem.list_directory()
    }

    /// Find a file by name in the current directory
    pub fn find_file(&self, name: &str) -> FilesystemResult<Option<FileRecordNumber>> {
        self.filesystem.find_file(name)
    }

    /// Change current directory (simplified - assumes directory name in current dir)
    pub fn change_directory(&mut self, name: &str) -> FilesystemResult<()> {
        if let Some(dir_record) = self.filesystem.find_file(name)? {
            self.current_directory = dir_record;
            Ok(())
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }

    /// Get current directory record number
    pub fn get_current_directory(&self) -> FileRecordNumber {
        self.current_directory
    }

    /// Move to parent directory (simplified)
    pub fn move_to_parent(&mut self) -> FilesystemResult<()> {
        // In a full implementation, we'd track parent relationships
        // For now, just reset to root
        self.current_directory = 5; // Root directory
        Ok(())
    }

    /// Get filesystem statistics
    pub fn get_stats(&mut self) -> FilesystemResult<crate::galleon_fs::FilesystemStats> {
        self.filesystem.get_stats()
    }

    /// Defragment the filesystem
    pub fn defragment(&mut self) -> FilesystemResult<()> {
        self.filesystem.defragment()
    }

    /// Sync all changes to disk
    pub fn sync(&mut self) -> FilesystemResult<()> {
        self.filesystem.sync()
    }

    /// Copy a file within the filesystem
    pub fn copy_file(&mut self, source_name: &str, dest_name: String) -> FilesystemResult<FileRecordNumber> {
        // Find source file
        let source_record = self.find_file(source_name)?
            .ok_or(FilesystemError::InvalidParameter)?;

        // Read source data
        let data = self.read_file_binary(source_record)?;

        // Create new file with the data
        self.create_file_binary(dest_name, data)
    }

    /// Move (rename) a file
    pub fn move_file(&mut self, old_name: &str, new_name: String) -> FilesystemResult<()> {
        // Copy to new name
        let _new_record = self.copy_file(old_name, new_name)?;

        // Delete old file
        self.delete_file(old_name)?;

        Ok(())
    }

    /// Get file size
    pub fn get_file_size(&mut self, file_record: FileRecordNumber) -> FilesystemResult<u64> {
        let data = self.read_file_binary(file_record)?;
        Ok(data.len() as u64)
    }

    /// Check if a file exists
    pub fn file_exists(&self, name: &str) -> FilesystemResult<bool> {
        Ok(self.find_file(name)?.is_some())
    }

    /// Append data to an existing file
    pub fn append_file_text(&mut self, file_record: FileRecordNumber, additional_content: String) -> FilesystemResult<()> {
        let mut existing_data = self.read_file_binary(file_record)?;
        existing_data.extend_from_slice(additional_content.as_bytes());
        self.write_file_binary(file_record, existing_data)
    }

    /// Append binary data to an existing file
    pub fn append_file_binary(&mut self, file_record: FileRecordNumber, additional_data: Vec<u8>) -> FilesystemResult<()> {
        let mut existing_data = self.read_file_binary(file_record)?;
        existing_data.extend_from_slice(&additional_data);
        self.write_file_binary(file_record, existing_data)
    }

    /// Truncate a file to a specific size
    pub fn truncate_file(&mut self, file_record: FileRecordNumber, new_size: usize) -> FilesystemResult<()> {
        let mut data = self.read_file_binary(file_record)?;
        if new_size < data.len() {
            data.truncate(new_size);
            self.write_file_binary(file_record, data)?;
        }
        Ok(())
    }
}

/// Path-based file operations (convenience functions)
impl FileManager {
    /// Create file using path-like interface
    pub fn create_file_at_path(&mut self, _path: PathBuf, name: String, contents: Option<String>) -> FilesystemResult<FileRecordNumber> {
        // For now, ignore path and create in current directory
        // Full implementation would parse path and navigate directories
        self.create_file(name, contents)
    }

    /// List files at a specific path
    pub fn list_files_at_path(&self, _path: PathBuf) -> FilesystemResult<Vec<(String, FileRecordNumber)>> {
        // For now, list current directory
        // Full implementation would navigate to path first
        self.list_files()
    }

    /// Delete file at path
    pub fn delete_file_at_path(&mut self, _path: PathBuf, name: &str) -> FilesystemResult<()> {
        // For now, delete from current directory
        self.delete_file(name)
    }
}

// Legacy compatibility functions for the old interface
/// Create a file using the legacy interface (compatibility)
pub fn create_file(drive: u8, name: String, contents: Option<String>) -> FilesystemResult<()> {
    let mut manager = FileManager::mount_drive(drive)?;
    let _file_record = manager.create_file(name, contents)?;
    manager.sync()?;
    Ok(())
}

/// List files using legacy interface (compatibility)
pub fn list_files(_drive: u8, _path: PathBuf) -> FilesystemResult<Vec<String>> {
    // This function signature doesn't return filesystem results in the original
    // We'll need to modify callers to use the new FileManager interface
    Ok(Vec::new()) // Stub for compatibility
}

/// Delete file using legacy interface (compatibility)
pub fn delete_file(drive: u8, name: &str) -> FilesystemResult<()> {
    let mut manager = FileManager::mount_drive(drive)?;
    manager.delete_file(name)?;
    manager.sync()?;
    Ok(())
}
