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
//! ```

#![no_std]

// Import all symbols from the IDE interface module
// This provides low-level disk I/O functions like ide_read_sectors and ide_write_sectors
use ide::*;

mod boot_block;
use boot_block::BootBlock;

// Import the BootBlock module (assumed to be in the same crate)
// This should be: use crate::boot_block::BootBlock; or similar depending on module structure
// For now, assuming BootBlock is available in scope

/// Initializes and writes a new boot block to the specified drive
/// 
/// Creates a new filesystem boot block with default parameters and writes it to
/// sector 0 (the boot sector) of the specified storage device. This effectively
/// formats the drive with the custom filesystem.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number to initialize (typically 0 for primary master, 1 for primary slave)
/// 
/// # Filesystem Parameters
/// The created filesystem will have:
/// - Total blocks: 100,000 (approximately 400MB with 4KB blocks)
/// - Root directory: Located at block 1
/// - Block size: 4096 bytes (4KB)
/// - Version: 1
/// 
/// # Safety
/// This function will overwrite the boot sector of the specified drive, destroying
/// any existing filesystem or data. Use with caution and ensure the correct drive
/// number is specified.
/// 
/// # Example
/// ```rust
/// // Initialize filesystem on the primary IDE drive
/// write_boot_block(0);
/// ```
/// 
/// # Panics
/// May panic if the IDE write operation fails due to hardware issues or
/// invalid drive numbers.
pub fn write_boot_block(drive_num: u8) {
    // Create a new boot block with 100,000 total blocks and root directory at block 1
    let boot_block = BootBlock::new(100_000, 1);
    
    // Serialize the boot block into a 512-byte sector format
    let sector = boot_block.as_sector();
    
    // Write the boot block to sector 0 (boot sector) of the specified drive
    // Parameters: drive_number, starting_sector, sector_count, data_buffer
    // Fixed: Pass pointer to the array data, not pointer to the array itself
    ide_write_sectors(drive_num, 0, 1, sector.as_ptr() as *const _);
}

/// Validates that a drive contains a valid boot block for this filesystem
/// 
/// Reads the boot sector (sector 0) from the specified drive and checks if it
/// contains the expected magic string that identifies this filesystem format.
/// This is typically used during filesystem mounting to verify compatibility.
/// 
/// # Parameters  
/// - `drive_num`: The IDE drive number to validate
/// 
/// # Returns
/// - `true`: The drive contains a valid boot block with the correct magic string
/// - `false`: The drive either has no boot block, contains a different filesystem,
///            or the magic string doesn't match
/// 
/// # Example
/// ```rust
/// // Check if drive 0 has our filesystem
/// if validate_boot_block(0) {
///     println!("Filesystem is valid and ready for use");
///     // Proceed with filesystem operations
/// } else {
///     println!("Drive needs to be formatted or contains different filesystem");
///     // Consider calling write_boot_block() to initialize
/// }
/// ```
/// 
/// # Error Handling
/// This function handles read errors gracefully by returning `false`. If the
/// IDE read operation fails, the validation will fail and the function returns
/// `false` rather than panicking.
pub fn validate_boot_block(drive_num: u8) -> bool {
    // Allocate buffer for reading the boot sector
    let mut sector = [0u8; 512];
    
    // Read sector 0 (boot sector) from the specified drive
    // Parameters: drive_number, starting_sector, sector_count, buffer
    ide_read_sectors(drive_num, 0, 1, sector.as_mut_ptr() as *mut _);
    
    // Validate the magic string in the boot block
    BootBlock::is_valid(&sector)
}

/// Reads and parses the boot block from a drive
/// 
/// Reads the boot sector and deserializes it into a BootBlock structure,
/// allowing access to filesystem metadata like version, block count, etc.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number to read from
/// 
/// # Returns
/// - `Some(BootBlock)`: If the drive contains a valid boot block
/// - `None`: If the drive has no valid boot block or read fails
/// 
/// # Example
/// ```rust
/// if let Some(boot_block) = read_boot_block(0) {
///     println!("Filesystem version: {}", boot_block.version);
///     println!("Total blocks: {}", boot_block.total_blocks);
///     println!("Free blocks: {}", boot_block.free_block_count);
/// } else {
///     println!("No valid filesystem found");
/// }
/// ```
pub fn read_boot_block(drive_num: u8) -> Option<BootBlock> {
    // Allocate buffer for reading the boot sector
    let mut sector = [0u8; 512];
    
    // Read sector 0 (boot sector) from the specified drive
    ide_read_sectors(drive_num, 0, 1, sector.as_mut_ptr() as *mut _);
    
    // Check if it's valid first
    if BootBlock::is_valid(&sector) {
        // Deserialize the sector into a BootBlock structure
        Some(BootBlock::from_sector(&sector))
    } else {
        None
    }
}

// Additional helper functions could be added here for:
// - Reading boot block metadata (version, block count, etc.)
// - Updating free block count
// - Filesystem health checks
// - Drive enumeration and detection