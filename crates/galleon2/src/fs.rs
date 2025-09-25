use core::ffi::c_void;

use ide::{ide_write_sectors, IdeResult};
use lib_kernel::kprintln;

use crate::{
    return_drive_size_bytes, validate_super_block, 
    write_super_block, FilesystemError, FilesystemResult
};

pub fn zero_out_sector(drive: u8, sector: u64) -> IdeResult<()> {
    let zero_buf = [0u8; 512];
    let res = ide_write_sectors(drive, 1, sector as u32, zero_buf.as_ptr() as *const c_void);
    match &res {
        Ok(_) => kprintln!("Zeroed sector {} on drive {}", sector, drive),
        Err(e) => kprintln!("Failed to zero sector {} on drive {}: {:?}", sector, drive, e),
    }
    res
}

/// Initialize filesystem on a drive with proper error handling
pub fn init_fs(drive: u8) -> FilesystemResult<()> {
    // Get disk size with error handling
    let disk_size_bytes = return_drive_size_bytes(drive)?;

    if disk_size_bytes == 0 {
        kprintln!("Drive size is 0 bytes");
        return Err(FilesystemError::DriveNotFound);
    }

    let disk_sectors = disk_size_bytes / 512;
    for sector in 0..disk_sectors {
        let res = zero_out_sector(drive, sector);
        match res {
            Ok(_) => {} // already printed success inside zero_out_sector
            Err(e) => {
                kprintln!("Error zeroing sector {}: {:?}", sector, e);
                // Depending on behavior, could return Err here or continue
                return Err(FilesystemError::WriteError);
            }
        }
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