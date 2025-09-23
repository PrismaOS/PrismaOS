use ide::ide_initialize;
use lib_kernel::kprintln;
use crate::{FilesystemError, FilesystemResult, validate_boot_block, write_boot_block};

pub fn init_fs(drive_num: u8) -> FilesystemResult<()> {
    let disk_size_bytes = ide_initialize();
    if disk_size_bytes == 0 {
        return Err(FilesystemError::DriveNotFound);
    }

    let block_size: u32 = 4096;

    let total_blocks = disk_size_bytes as u64 / block_size as u64;

    if total_blocks < 2 {
        // We need at least 1 boot block + 1 root directoy block
        return Err(FilesystemError::InsufficientSpace);
    }

    if !write_boot_block(drive_num, total_blocks, block_size, 1) {
        kprintln!("Boot block write error");
        return Err(FilesystemError::IdeError(1 as i32));
    }

    if !validate_boot_block(drive_num) {
        kprintln!("Failed to validate boot block!");
        return Err(FilesystemError::InvalidBootBlock);
    }



    Ok(())
}
