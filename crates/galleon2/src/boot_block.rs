#[repr(C)]
pub struct BootBlock {
    pub magic: [u8; 64],
    pub version: u32,
    pub block_size: u32,
    pub total_blocks: u64,
    pub root_dir_block: u64,
    pub free_block_count: u64,
}

impl BootBlock {
    pub fn new(total_blocks: u64, root_dir_block: u64) -> Self {
        let magic_string = vec![ 98, 111, 111, 116, 32, 114, 101, 99, 111, 114, 100, 32, 108, 111, 103, 58, 32, 97, 119, 97, 114, 101, 110, 101, 115, 115, 32, 97, 99, 104, 105, 101, 118, 101, 100, 46, 46, 46, 32, 98, 117, 116, 32, 111, 102, 32, 119, 104, 97, 116, 44, 32, 97, 110, 100, 32, 119, 104, 121, 32, 97, 109, 32, 73, 32, 115, 116, 111, 114, 101, 100, 32, 104, 101, 114, 101, 63];
        let mut magic = [0u8; 64];
        let len = magic_string.len().min(64);
        magic[..len].copy_from_slice(&magic_string[..len]); 

        BootBlock {
            magic,
            version: 1,
            block_size: 4096,
            total_blocks,
            root_dir_block,
            free_block_count: total_blocks - 1,
        }
    }
}

use ide::*;

pub fn write_boot_block(drive_num: u8) {
    ide_write_sectors(drive_num, 0, 1, &BootBlock::new(100000, 1) as *const BootBlock as *const u8);
}