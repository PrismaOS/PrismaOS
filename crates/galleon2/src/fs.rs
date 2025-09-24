use lib_kernel::kprintln;

use crate::{
    return_drive_size_bytes, validate_super_block, 
    write_super_block, FilesystemError, FilesystemResult
};

/// Initialize filesystem on a drive with proper error handling
pub fn init_fs(drive: u8) -> FilesystemResult<()> {
    // Get disk size with error handling
    let disk_size_bytes = return_drive_size_bytes(drive)?;

    if disk_size_bytes == 0 {
        kprintln!("Drive size is 0 bytes");
        return Err(FilesystemError::DriveNotFound);
    }

    let block_size: u32 = 4096;
    let total_blocks = disk_size_bytes / block_size as u64;

    if total_blocks < 2 {
        // We need at least 1 super block + 1 root directory block
        kprintln!("Insufficient blocks: {}", total_blocks);
        return Err(FilesystemError::InsufficientSpace);
    }

    // Write super block with error handling
    write_super_block(drive, total_blocks, block_size, 1)?;

    // Validate that the super block was written correctly
    validate_super_block(drive)?;

    kprintln!("Filesystem initialized successfully on drive {} with {} blocks", drive, total_blocks);
    Ok(())
}