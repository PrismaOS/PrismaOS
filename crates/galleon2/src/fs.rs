use ide::ide_initialize;
use lib_kernel::kprintln;
use crate::{FilesystemError, FilesystemResult, validate_boot_block, write_boot_block};

pub fn init_fs(drive_num: u8) -> FilesystemResult<()> {
    // Initialize the drive and get its size in bytes
    let disk_size_bytes = ide_initialize();
    if disk_size_bytes == 0 {
        return Err(FilesystemError::DriveNotFound);
    }

    // Decide on a block size (let’s assume 4096 bytes, but you can make this dynamic later)
    let block_size: u32 = 4096;

    // Calculate how many blocks the drive can hold
    let total_blocks = disk_size_bytes as u64 / block_size as u64;

    if total_blocks < 2 {
        // We need at least 1 boot block + 1 root directory block
        return Err(FilesystemError::InsufficientSpace);
    }

    // TODO: @GhostedGaming Fix these comp errors plz
    // if write_boot_block(drive_num) == FilesystemError::DriveNotFound || FilesystemError::InsufficientSpace {
    //     kprintln!("Drive not found or InsufficientSpace");
    //     return Err();
    // }

    if !validate_boot_block(drive_num) {
        kprintln!("Failed to validate boot block!");
        return Err(FilesystemError::InvalidBootBlock);
    }

    Ok(())
}
