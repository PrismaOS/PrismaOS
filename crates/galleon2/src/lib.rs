//! # Galleon2 Filesystem Library
//!
//! This library provides high-level operations for working with a custom filesystem in PrismaOS.
//! It serves as the main interface between applications and the underlying filesystem structures,
//! handling boot block initialization, validation, and metadata management.
//!
//! ## Overview
//! - **Boot block creation**: Initialize a new filesystem on a storage device.
//! - **Validation**: Check for the presence and integrity of the filesystem using a magic string.
//! - **Metadata access**: Read and update boot block information, such as free block count and version.
//! - **Block allocation**: Allocate and deallocate blocks for file storage.
//! - **IDE integration**: Uses low-level IDE/ATA disk I/O for all operations.
//!
//! ## Dependencies
//! - Relies on an IDE (Integrated Drive Electronics) interface module for disk I/O.
//! - Uses a `boot_block` module for boot block structure and serialization.
//!
//! ## Example Usage
//! ```rust
//! // Initialize a new filesystem on drive 0
//! galleon2::write_boot_block(0).unwrap();
//!
//! // Validate the filesystem
//! if galleon2::validate_boot_block(0) {
//!     println!("Valid filesystem found!");
//! } else {
//!     println!("No valid filesystem detected");
//! }
//!
//! // Read boot block information
//! if let Some(boot_block) = galleon2::read_boot_block(0) {
//!     println!("Filesystem version: {}", boot_block.version);
//!     println!("Free blocks: {}", boot_block.free_block_count);
//! }
//! ```

#![no_std]

// Import all symbols from the IDE interface module
// This provides low-level disk I/O functions like ide_read_sectors and ide_write_sectors
use ide::{ide_read_sectors, ide_write_sectors, return_drive_size_bytes};

extern crate alloc;

pub mod fs;
mod boot_block;
use boot_block::BootBlock;


/// The result type for all filesystem operations in this library.
///
/// This is a convenience alias for `Result<T, FilesystemError>`.
pub type FilesystemResult<T> = Result<T, FilesystemError>;

/// Error types for filesystem operations.
///
/// This enum represents all possible errors that can occur when interacting with the filesystem.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilesystemError {
    /// IDE read/write operation failed. Contains the error code returned by the IDE driver.
    IdeError(i32),
    /// The boot block is invalid or the magic string does not match.
    InvalidBootBlock,
    /// The specified drive was not found or is not accessible.
    DriveNotFound,
    /// There is not enough free space to allocate the requested number of blocks.
    InsufficientSpace,
}

/// Writes a new boot block to the specified drive, initializing the filesystem.
///
/// # Arguments
/// * `drive_num` - The drive number to write the boot block to.
///
/// # Returns
/// * `Ok(())` if the boot block was written successfully.
/// * `Err(FilesystemError)` if the write operation failed.
///
/// # Example
/// ```
/// galleon2::write_boot_block(0).unwrap();
/// ```
pub fn write_boot_block(drive_num: u8, total_blocks: u64, block_size: u32, root_dir_block: u64) -> bool{
    // Create a new boot block with 100,000 total blocks and root directory at block 1
    let boot_block = BootBlock::new(block_size, total_blocks, root_dir_block);
    // Serialize the boot block into a 512-byte sector format
    let sector = boot_block.as_sector();
    // Write the boot block to sector 0 (boot sector) of the specified drive
    // Parameters: drive_number, starting_sector, sector_count, data_buffer
    let result = ide_write_sectors(drive_num, 1, 0, sector.as_ptr() as *const _);
    if result == 0 {
        true
    } else {
        false
    }
}

/// Validates the boot block on the specified drive.
///
/// This function checks if the boot block contains the correct magic string and is structurally valid.
///
/// # Arguments
/// * `drive_num` - The drive number to validate.
///
/// # Returns
/// * `true` if the boot block is valid and the filesystem is present.
/// * `false` if the boot block is invalid or the read operation failed.
///
/// # Example
/// ```
/// if galleon2::validate_boot_block(0) {
///     println!("Filesystem is valid");
/// }
/// ```
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

/// Reads the boot block from the specified drive.
///
/// # Arguments
/// * `drive_num` - The drive number to read from.
///
/// # Returns
/// * `Some(BootBlock)` if the boot block is valid and was read successfully.
/// * `None` if the boot block is invalid or the read operation failed.
///
/// # Example
/// ```
/// if let Some(bb) = galleon2::read_boot_block(0) {
///     println!("Version: {}", bb.version);
/// }
/// ```
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

/// Reads the boot block from the specified drive, returning a result type.
///
/// # Arguments
/// * `drive_num` - The drive number to read from.
///
/// # Returns
/// * `Ok(BootBlock)` if the boot block is valid and was read successfully.
/// * `Err(FilesystemError)` if the boot block is invalid or the read operation failed.
///
/// # Example
/// ```
/// let bb = galleon2::read_boot_block_result(0)?;
/// ```
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

/// Updates the free block count in the boot block of the filesystem.
///
/// This function reads the current boot block, updates the free block count,
/// and writes the updated boot block back to the drive.
///
/// # Arguments
/// * `drive_num` - The drive number where the filesystem is located.
/// * `new_free_count` - The new free block count to set.
///
/// # Returns
/// * `Ok(())` if the update was successful.
/// * `Err(FilesystemError)` if there was an error during the update.
///
/// # Errors
/// * `FilesystemError::InvalidBootBlock` if the boot block is invalid.
/// * `FilesystemError::IdeError` if there was an error writing to the drive.
///
/// # Example
/// ```
/// galleon2::update_free_block_count(0, 50000).unwrap();
/// ```
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

/// Allocates a specified number of blocks in the filesystem.
///
/// This function reads the current boot block, attempts to allocate the specified
/// number of blocks, and writes the updated boot block back to the drive.
///
/// # Arguments
/// * `drive_num` - The drive number where the filesystem is located.
/// * `block_count` - The number of blocks to allocate.
///
/// # Returns
/// * `Ok(())` if the allocation was successful.
/// * `Err(FilesystemError)` if there was an error during allocation.
///
/// # Errors
/// * `FilesystemError::InsufficientSpace` if there are not enough free blocks.
/// * `FilesystemError::IdeError` if there was an error writing to the drive.
///
/// # Example
/// ```
/// let result = galleon2::allocate_blocks(0, 10);
/// if let Err(e) = result {
///   println!("Failed to allocate blocks: {:?}", e);
/// }
/// ```
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

/// Deallocates a specified number of blocks in the filesystem.
///
/// This function reads the current boot block, increases the free block count by the specified
/// number, and writes the updated boot block back to the drive.
///
/// # Arguments
/// * `drive_num` - The drive number where the filesystem is located.
/// * `block_count` - The number of blocks to deallocate.
///
/// # Returns
/// * `Ok(())` if the deallocation was successful.
/// * `Err(FilesystemError)` if there was an error during deallocation.
///
/// # Example
/// ```
/// galleon2::deallocate_blocks(0, 5).unwrap();
/// ```
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

/// Retrieves basic information about the filesystem on the specified drive.
///
/// # Arguments
/// * `drive_num` - The drive number to query.
///
/// # Returns
/// * `Some((version, total_blocks, free_block_count, block_size))` if the filesystem is valid.
/// * `None` if the filesystem is not present or invalid.
///
/// # Example
/// ```
/// if let Some((ver, total, free, size)) = galleon2::get_filesystem_info(0) {
///     println!("Version: {ver}, Total: {total}, Free: {free}, Block size: {size}");
/// }
/// ```
pub fn get_filesystem_info(drive_num: u8) -> Option<(u32, u64, u64, u32)> {
    read_boot_block(drive_num).map(|bb| (bb.version, bb.total_blocks, bb.free_block_count, bb.block_size))
}

/// Checks if a valid filesystem is present on the specified drive.
///
/// # Arguments
/// * `drive_num` - The drive number to check.
///
/// # Returns
/// * `true` if a valid filesystem is present.
/// * `false` otherwise.
///
/// # Example
/// ```
/// if galleon2::has_filesystem(0) {
///     println!("Filesystem detected");
/// }
/// ```
pub fn has_filesystem(drive_num: u8) -> bool {
    validate_boot_block(drive_num)
}