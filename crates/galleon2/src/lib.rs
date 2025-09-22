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

extern crate alloc;

// Import all symbols from the IDE interface module
// This provides low-level disk I/O functions like ide_read_sectors and ide_write_sectors
use ide::*;

mod boot_block;
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
/// # Returns
/// - `Ok(())`: Boot block was successfully written
/// - `Err(FilesystemError::IdeError)`: IDE write operation failed
/// 
/// # Safety
/// This function will overwrite the boot sector of the specified drive, destroying
/// any existing filesystem or data. Use with caution and ensure the correct drive
/// number is specified.
/// 
/// # Example
/// ```rust
/// // Initialize filesystem on the primary IDE drive
/// match write_boot_block(0) {
///     Ok(()) => println!("Filesystem initialized successfully"),
///     Err(e) => println!("Failed to initialize filesystem: {:?}", e),
/// }
/// ```
pub fn write_boot_block(drive_num: u8) -> FilesystemResult<()> {
    // Create a new boot block with 100,000 total blocks and root directory at block 1
    let boot_block = BootBlock::new(100_000, 1);
    
    // Serialize the boot block into a 512-byte sector format
    let sector = boot_block.as_sector();
    
    // Write the boot block to sector 0 (boot sector) of the specified drive
    // Parameters: drive_number, starting_sector, sector_count, data_buffer
    let result = ide_write_sectors(drive_num, 0, 1, sector.as_ptr() as *const _);
    
    if result == 0 {
        Ok(())
    } else {
        Err(FilesystemError::IdeError(result))
    }
}

/// Initializes a custom filesystem with specified parameters
/// 
/// Creates a new filesystem boot block with custom total blocks and root directory
/// location, then writes it to the specified drive.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number to initialize
/// - `total_blocks`: Total number of blocks in the filesystem
/// - `root_dir_block`: Block number where the root directory will be located
/// 
/// # Returns
/// - `Ok(())`: Boot block was successfully written
/// - `Err(FilesystemError::IdeError)`: IDE write operation failed
/// 
/// # Example
/// ```rust
/// // Create a smaller filesystem with 50,000 blocks
/// match write_custom_boot_block(0, 50_000, 1) {
///     Ok(()) => println!("Custom filesystem initialized"),
///     Err(e) => println!("Failed: {:?}", e),
/// }
/// ```
pub fn write_custom_boot_block(drive_num: u8, total_blocks: u64, root_dir_block: u64) -> FilesystemResult<()> {
    // Create a new boot block with custom parameters
    let boot_block = BootBlock::new(total_blocks, root_dir_block);
    
    // Serialize the boot block into a 512-byte sector format
    let sector = boot_block.as_sector();
    
    // Write the boot block to sector 0 (boot sector) of the specified drive
    let result = ide_write_sectors(drive_num, 0, 1, sector.as_ptr() as *const _);
    
    if result == 0 {
        Ok(())
    } else {
        Err(FilesystemError::IdeError(result as u32))
    }
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
///            the magic string doesn't match, or the read operation failed
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
    // Check if the read operation succeeded (assuming 0 means success)
    let result = ide_read_sectors(drive_num, 0, 1, sector.as_mut_ptr() as *mut _);
    if result != 0 {
        // Read failed, return false
        return false;
    }
    
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
///     println!("Magic message: {}", boot_block.magic_as_string());
/// } else {
///     println!("No valid filesystem found");
/// }
/// ```
pub fn read_boot_block(drive_num: u8) -> Option<BootBlock> {
    // Allocate buffer for reading the boot sector
    let mut sector = [0u8; 512];
    
    // Read sector 0 (boot sector) from the specified drive
    // Check if the read operation succeeded
    let result = ide_read_sectors(drive_num, 0, 1, sector.as_mut_ptr() as *mut _);
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

/// Reads the boot block with error information
/// 
/// Similar to read_boot_block but returns detailed error information
/// instead of None on failure.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number to read from
/// 
/// # Returns
/// - `Ok(BootBlock)`: Successfully read and validated boot block
/// - `Err(FilesystemError)`: Specific error that occurred
/// 
/// # Example
/// ```rust
/// match read_boot_block_result(0) {
///     Ok(boot_block) => {
///         println!("Filesystem info:");
///         println!("  Version: {}", boot_block.version);
///         println!("  Blocks: {}", boot_block.total_blocks);
///         println!("  Free: {}", boot_block.free_block_count);
///     }
///     Err(FilesystemError::IdeError(code)) => {
///         println!("IDE error: {}", code);
///     }
///     Err(FilesystemError::InvalidBootBlock) => {
///         println!("Invalid or missing filesystem");
///     }
///     Err(e) => println!("Other error: {:?}", e),
/// }
/// ```
pub fn read_boot_block_result(drive_num: u8) -> FilesystemResult<BootBlock> {
    // Allocate buffer for reading the boot sector
    let mut sector = [0u8; 512];
    
    // Read sector 0 (boot sector) from the specified drive
    let result = ide_read_sectors(drive_num, 0, 1, sector.as_mut_ptr() as *mut _);
    if result != 0 {
        return Err(FilesystemError::IdeError(result));
    }
    
    // Check if it's valid
    if BootBlock::is_valid(&sector) {
        // Deserialize the sector into a BootBlock structure
        Ok(BootBlock::from_sector(&sector))
    } else {
        Err(FilesystemError::InvalidBootBlock)
    }
}

/// Updates the free block count in the boot block
/// 
/// Reads the current boot block, updates the free block count, and writes
/// it back to the drive. This is useful for maintaining filesystem metadata.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number to update
/// - `new_free_count`: The new free block count
/// 
/// # Returns
/// - `Ok(())`: Free block count was successfully updated
/// - `Err(FilesystemError)`: Operation failed
/// 
/// # Example
/// ```rust
/// // Update free block count after allocating/deallocating blocks
/// match update_free_block_count(0, 99_500) {
///     Ok(()) => println!("Free block count updated"),
///     Err(e) => println!("Failed to update: {:?}", e),
/// }
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
        Err(FilesystemError::IdeError(result))
    }
}

/// Allocates blocks and updates the boot block
/// 
/// Decrements the free block count by the specified amount and writes
/// the updated boot block back to the drive.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number
/// - `block_count`: Number of blocks to allocate
/// 
/// # Returns
/// - `Ok(())`: Blocks were successfully allocated
/// - `Err(FilesystemError::InsufficientSpace)`: Not enough free blocks
/// - `Err(FilesystemError)`: Other operation failed
/// 
/// # Example
/// ```rust
/// // Allocate 10 blocks for a file
/// match allocate_blocks(0, 10) {
///     Ok(()) => println!("Blocks allocated successfully"),
///     Err(FilesystemError::InsufficientSpace) => println!("Not enough free space"),
///     Err(e) => println!("Allocation failed: {:?}", e),
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
        Err(FilesystemError::IdeError(result))
    }
}

/// Deallocates blocks and updates the boot block
/// 
/// Increments the free block count by the specified amount and writes
/// the updated boot block back to the drive.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number
/// - `block_count`: Number of blocks to deallocate
/// 
/// # Returns
/// - `Ok(())`: Blocks were successfully deallocated
/// - `Err(FilesystemError)`: Operation failed
/// 
/// # Example
/// ```rust
/// // Free 5 blocks after deleting a file
/// match deallocate_blocks(0, 5) {
///     Ok(()) => println!("Blocks freed successfully"),
///     Err(e) => println!("Deallocation failed: {:?}", e),
/// }
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
        Err(FilesystemError::IdeError(result))
    }
}

/// Gets filesystem information without full boot block parsing
/// 
/// Returns basic filesystem information for quick status checks.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number
/// 
/// # Returns
/// A tuple containing (version, total_blocks, free_blocks, block_size)
/// or None if the filesystem is invalid or unreadable.
/// 
/// # Example
/// ```rust
/// if let Some((version, total, free, block_size)) = get_filesystem_info(0) {
///     println!("FS v{}: {}/{} blocks free ({} bytes per block)", 
///              version, free, total, block_size);
/// }
/// ```
pub fn get_filesystem_info(drive_num: u8) -> Option<(u32, u64, u64, u32)> {
    read_boot_block(drive_num).map(|bb| (bb.version, bb.total_blocks, bb.free_block_count, bb.block_size))
}

/// Checks if a drive contains our filesystem format
/// 
/// Quick check to see if a drive has been formatted with our filesystem.
/// More efficient than reading the full boot block if you only need validation.
/// 
/// # Parameters
/// - `drive_num`: The IDE drive number to check
/// 
/// # Returns
/// `true` if the drive contains our filesystem format, `false` otherwise
/// 
/// # Example
/// ```rust
/// for drive in 0..4 {
///     if has_filesystem(drive) {
///         println!("Found filesystem on drive {}", drive);
///     }
/// }
/// ```
pub fn has_filesystem(drive_num: u8) -> bool {
    validate_boot_block(drive_num)
}