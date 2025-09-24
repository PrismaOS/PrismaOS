#![no_std]

pub use ide::{ide_write_sectors, ide_read_sectors, return_drive_size_bytes, IdeError, IdeResult};

extern crate alloc;

pub mod fs;
mod file;
mod super_block;
use super_block::SuperBlock;

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
    root_dir_block: u64
) -> FilesystemResult<()> {
    let super_block = SuperBlock::new(block_size, total_blocks, root_dir_block);
    let sector = super_block.as_sector();
    
    // Write to sector 0 (boot sector)
    ide_write_sectors(drive_num, 1, 0, sector.as_ptr() as *const _)?;
    Ok(())
}

/// Validate the super block on the specified drive with proper error handling
pub fn validate_super_block(drive_num: u8) -> FilesystemResult<()> {
    let mut sector = [0u8; 512];
    
    // Read sector 0 (boot sector)
    ide_read_sectors(drive_num, 1, 0, sector.as_mut_ptr() as *mut _)?;
    
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
    ide_read_sectors(drive_num, 1, 0, sector.as_mut_ptr() as *mut _)?;
    
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
    ide_write_sectors(drive_num, 1, 0, sector.as_ptr() as *const _)?;
    Ok(())
}

/// Allocate a specified number of blocks in the filesystem with proper error handling
pub fn allocate_blocks(drive_num: u8, block_count: u64) -> FilesystemResult<()> {
    let mut super_block = read_super_block(drive_num)?;
    
    if !super_block.allocate_blocks(block_count) {
        return Err(FilesystemError::InsufficientSpace);
    }
    
    let sector = super_block.as_sector();
    ide_write_sectors(drive_num, 1, 0, sector.as_ptr() as *const _)?;
    Ok(())
}

/// Deallocate a specified number of blocks in the filesystem with proper error handling
pub fn deallocate_blocks(drive_num: u8, block_count: u64) -> FilesystemResult<()> {
    let mut super_block = read_super_block(drive_num)?;
    super_block.deallocate_blocks(block_count);
    
    let sector = super_block.as_sector();
    ide_write_sectors(drive_num, 1, 0, sector.as_ptr() as *const _)?;
    Ok(())
}

/// Get filesystem information with proper error handling
pub fn get_filesystem_info(drive_num: u8) -> FilesystemResult<(u32, u64, u64, u32)> {
    let super_block = read_super_block(drive_num)?;
    Ok((super_block.version, super_block.total_blocks, super_block.free_block_count, super_block.block_size))
}

/// Check if a valid filesystem is present on the specified drive with proper error handling
pub fn has_filesystem(drive_num: u8) -> bool {
    validate_super_block(drive_num).is_ok()
}