//! Filesystem Library
//! 
//! This library provides high-level operations for working with a custom filesystem.
//! It serves as the main interface between applications and the underlying filesystem
//! structures, handling boot block initialization and validation operations.
//! 
//! The library depends on an IDE (Integrated Drive Electronics) interface module
//! for low-level disk I/O operations and uses the boot block module for filesystem
//! metadata management.
//! 
//! # Features
//! - Boot block creation and writing to storage devices
//! - Filesystem validation through magic string verification
//! - Integration with IDE/ATA storage interface
//! - Boot block metadata reading and manipulation
//! 
//! # Usage
//! ```rust
//! // Initialize a new filesystem on drive 0
//! write_boot_block(0);
//! 
//! // Later, validate that the filesystem is present and valid
//! if validate_boot_block(0) {
//!     println!("Valid filesystem found!");
//! } else {
//!     println!("No valid filesystem detected");
//! }
//! 
//! // Read boot block information
//! if let Some(boot_block) = read_boot_block(0) {
//!     println!("Filesystem version: {}", boot_block.version);
//!     println!("Free blocks: {}", boot_block.free_block_count);
//! }
//! ```

#![no_std]

// Import all symbols from the IDE interface module
// This provides low-level disk I/O functions like ide_read_sectors and ide_write_sectors
use ide::*;

extern crate alloc;

mod boot_block;
mod fs;
use boot_block::BootBlock;

/// Result type for filesystem operations
pub type FilesystemResult<T> = Result<T, FilesystemError>;

/// Error types for filesystem operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilesystemError {
    /// IDE read/write operation failed
    IdeError(i32),
    /// Invalid boot block or magic string mismatch
    InvalidBootBlock,
    /// Drive not found or not accessible
    DriveNotFound,
    /// Insufficient free blocks for allocation
    InsufficientSpace,
}


pub fn write_boot_block(drive_num: u8) -> FilesystemResult<()> {
    // Create a new boot block with 100,000 total blocks and root directory at block 1
    let boot_block = BootBlock::new(100_000, 1);
    
    // Serialize the boot block into a 512-byte sector format
    let sector = boot_block.as_sector();
    
    // Write the boot block to sector 0 (boot sector) of the specified drive
    // Parameters: drive_number, starting_sector, sector_count, data_buffer
    let result = ide_write_sectors(drive_num, 1, 0, sector.as_ptr() as *const _);
    
    if result == 0 {
        Ok(())
    } else {
        Err(FilesystemError::IdeError(result as i32))
    }
}

pub fn validate_boot_block(drive_num: u8) -> bool {
    // Allocate buffer for reading the boot sector
    let mut sector = [0u8; 512];
    
    // Read sector 0 (boot sector) from the specified drive
    // IDE driver signature: ide_read_sectors(drive, numsects, lba, buf)
    // Fixed parameter order to match your IDE driver
    let result = ide_read_sectors(drive_num, 1, 0, sector.as_mut_ptr() as *mut _);
    if result != 0 {
        // Read failed, return false
        return false;
    }
    
    // Validate the magic string in the boot block
    BootBlock::is_valid(&sector)
}

pub fn read_boot_block(drive_num: u8) -> Option<BootBlock> {
    // Allocate buffer for reading the boot sector
    let mut sector = [0u8; 512];
    
    // Read sector 0 (boot sector) from the specified drive
    // IDE driver signature: ide_read_sectors(drive, numsects, lba, buf)
    let result = ide_read_sectors(drive_num, 1, 0, sector.as_mut_ptr() as *mut _);
    if result != 0 {
        // Read failed, return None
        return None;
    }
    
    // Check if it's valid first
    if BootBlock::is_valid(&sector) {
        // Deserialize the sector into a BootBlock structure
        Some(BootBlock::from_sector(&sector))
    } else {
        None
    }
}

pub fn read_boot_block_result(drive_num: u8) -> FilesystemResult<BootBlock> {
    // Allocate buffer for reading the boot sector
    let mut sector = [0u8; 512];
    
    // Read sector 0 (boot sector) from the specified drive
    let result = ide_read_sectors(drive_num, 0, 1, sector.as_mut_ptr() as *mut _);
    if result != 0 {
        return Err(FilesystemError::IdeError(result as i32));
    }
    
    // Check if it's valid
    if BootBlock::is_valid(&sector) {
        // Deserialize the sector into a BootBlock structure
        Ok(BootBlock::from_sector(&sector))
    } else {
        Err(FilesystemError::InvalidBootBlock)
    }
}

pub fn update_free_block_count(drive_num: u8, new_free_count: u64) -> FilesystemResult<()> {
    // Read current boot block
    let mut boot_block = read_boot_block_result(drive_num)?;
    
    // Update free block count
    boot_block.set_free_block_count(new_free_count);
    
    // Write back to drive
    let sector = boot_block.as_sector();
    let result = ide_write_sectors(drive_num, 0, 1, sector.as_ptr() as *const _);
    
    if result == 0 {
        Ok(())
    } else {
        Err(FilesystemError::IdeError(result as i32))
    }
}

pub fn allocate_blocks(drive_num: u8, block_count: u64) -> FilesystemResult<()> {
    // Read current boot block
    let mut boot_block = read_boot_block_result(drive_num)?;
    
    // Try to allocate blocks
    if !boot_block.allocate_blocks(block_count) {
        return Err(FilesystemError::InsufficientSpace);
    }
    
    // Write back to drive
    let sector = boot_block.as_sector();
    let result = ide_write_sectors(drive_num, 0, 1, sector.as_ptr() as *const _);
    
    if result == 0 {
        Ok(())
    } else {
        Err(FilesystemError::IdeError(result as i32))
    }
}

pub fn deallocate_blocks(drive_num: u8, block_count: u64) -> FilesystemResult<()> {
    // Read current boot block
    let mut boot_block = read_boot_block_result(drive_num)?;
    
    // Deallocate blocks
    boot_block.deallocate_blocks(block_count);
    
    // Write back to drive
    let sector = boot_block.as_sector();
    let result = ide_write_sectors(drive_num, 0, 1, sector.as_ptr() as *const _);
    
    if result == 0 {
        Ok(())
    } else {
        Err(FilesystemError::IdeError(result as i32))
    }
}

pub fn get_filesystem_info(drive_num: u8) -> Option<(u32, u64, u64, u32)> {
    read_boot_block(drive_num).map(|bb| (bb.version, bb.total_blocks, bb.free_block_count, bb.block_size))
}

pub fn has_filesystem(drive_num: u8) -> bool {
    validate_boot_block(drive_num)
}