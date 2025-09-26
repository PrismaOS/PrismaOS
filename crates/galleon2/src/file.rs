use crate::{FilesystemResult, ide_read_sectors, ide_write_sectors};
use alloc::string::String;
use lib_kernel::kprintln;

pub fn create_file(drive: u8, name: String, contents: Option<String>) -> FilesystemResult<()> {
    kprintln!("Creating file: {}", name);

    match crate::read_super_block(drive) {
        Ok(super_block) => {
            kprintln!("Free blocks available: {}", super_block.free_block_count);

            let used_blocks = super_block.total_blocks - super_block.free_block_count;
            let next_block = used_blocks + 1;

            let sectors_per_block = super_block.block_size / 512;
            let next_sector = next_block * sectors_per_block as u64;

            let mut sector_buffer = [0u8; 512];
            ide_read_sectors(drive, 1, next_sector as u32, &mut sector_buffer)?;

            let is_zeroed = sector_buffer.iter().all(|&b| b == 0);
            kprintln!("Sector {} is zeroed: {}", next_sector, is_zeroed);

            let mut write_buffer = [0u8; 512];
            let name_bytes = name.as_bytes();
            let name_len = name_bytes.len().min(255);

            write_buffer[0] = name_len as u8;
            write_buffer[1..1 + name_len].copy_from_slice(&name_bytes[..name_len]);

            if let Some(content) = contents {
                let content_bytes = content.as_bytes();
                let content_len = content_bytes.len().min(512 - name_len - 2);
                write_buffer[1 + name_len] = content_len as u8;
                write_buffer[2 + name_len..2 + name_len + content_len]
                    .copy_from_slice(&content_bytes[..content_len]);
            }

            kprintln!("Writing file data");
            ide_write_sectors(drive, 1, next_sector as u32, &write_buffer)?;
            kprintln!("File written successfully");

            Ok(())
        }
        Err(e) => {
            kprintln!("Error reading superblock: {:?}", e);
            Err(e)
        }
    }
}

pub fn list_files(drive: u8, path: crate::types::pathbuf::PathBuf) {}

pub fn delete_file() {
    todo!();
}
