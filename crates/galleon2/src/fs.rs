use ide::{ide_write_sectors, IdeResult};
use lib_kernel::kprintln;

use crate::{
    return_drive_size_bytes, validate_super_block, 
    write_super_block, FilesystemError, FilesystemResult
};

pub fn zero_out_sectors(drive: u8, start_sector: u64, sector_count: u8) -> IdeResult<()> {
    // Create buffer for 50 sectors (50 * 512 = 25,600 bytes)
    let zero_buf = [0u8; 50 * 512];
    let res = ide_write_sectors(drive, sector_count, start_sector as u32, &zero_buf);
    match &res {
        Ok(_) => {},
        Err(e) => kprintln!("Failed to zero {} sectors starting at sector {} on drive {}: {:?}", sector_count, start_sector, drive, e),
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
    let mut sector = 0;
    
    //while sector < disk_sectors {
    //    let sectors_remaining = disk_sectors - sector;
    //    let sectors_to_zero = if sectors_remaining >= 50 { 50 } else { sectors_remaining as u8 };
    //    
    //    let res = zero_out_sectors(drive, sector, sectors_to_zero);
    //    match res {
    //        Ok(_) => {} // already printed success inside zero_out_sectors
    //        Err(e) => {
    //            kprintln!("Error zeroing sectors starting at {}: {:?}", sector, e);
    //            return Err(FilesystemError::WriteError);
    //        }
    //    }
    //    
    //    sector += sectors_to_zero as u64;
    //}

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