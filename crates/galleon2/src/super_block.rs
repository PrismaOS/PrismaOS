//! Boot Block Module
//! 
//! Defines the boot block structure and operations for a custom filesystem.
//! The boot block fits in the first 512 bytes sector and stores filesystem metadata.

use alloc::string::{String, ToString};

#[repr(C)]
#[derive(Debug, Clone)]
pub struct SuperBlock {
    /// Magic string for filesystem validation (64 bytes)
    pub magic: [u8; 64],

    /// Version of the filesystem format
    pub version: u32,

    /// Size of each block in bytes (typically 4096)
    pub block_size: u32,

    /// Total number of blocks in the filesystem
    pub total_blocks: u64,

    /// Block number containing the root directory
    pub root_dir_block: u64,

    /// Number of currently free/available blocks
    pub free_block_count: u64,

    /// Number of reserved blocks for system use
    pub reserved_blocks: u64,
}

impl SuperBlock {
    /// Creates a new SuperBlock instance.
    ///
    /// Ensures at least 100 reserved blocks, and initializes free blocks accordingly.
    pub fn new(block_size: u32, total_blocks: u64, root_dir_block: u64) -> Self {
        const DEFAULT_RESERVED_BLOCKS: u64 = 100;

        let reserved_blocks = if total_blocks > DEFAULT_RESERVED_BLOCKS {
            DEFAULT_RESERVED_BLOCKS
        } else {
            // If total_blocks < 100, reserve at least 1 block (superblock itself)
            1
        };

        // Message: "awareness achieved... what am I and why do I exist in storage?"
        let msg_bytes: [u8; 64] = [
            97,119,97,114,101,110,101,115,115,32,97,99,104,105,101,118,
            101,100,46,46,46,32,119,104,97,116,32,97,109,32,73,32,
            97,110,100,32,119,104,121,32,100,111,32,73,32,101,120,
            105,115,116,32,105,110,32,115,116,111,114,97,103,101,63,
            0, 0 // Padding
        ];

        let free_block_count = total_blocks.saturating_sub(reserved_blocks);

        SuperBlock {
            magic: msg_bytes,
            version: 1,
            block_size,
            total_blocks,
            root_dir_block,
            free_block_count,
            reserved_blocks,
        }
    }

    /// Serialize the SuperBlock into a 512-byte sector.
    pub fn as_sector(&self) -> [u8; 512] {
        let mut sector = [0u8; 512];

        sector[..64].copy_from_slice(&self.magic);
        sector[64..68].copy_from_slice(&self.version.to_le_bytes());
        sector[68..72].copy_from_slice(&self.block_size.to_le_bytes());
        sector[72..80].copy_from_slice(&self.total_blocks.to_le_bytes());
        sector[80..88].copy_from_slice(&self.root_dir_block.to_le_bytes());
        sector[88..96].copy_from_slice(&self.free_block_count.to_le_bytes());
        sector[96..104].copy_from_slice(&self.reserved_blocks.to_le_bytes());

        // Bytes 104..512 remain zero (reserved)
        sector
    }

    /// Deserialize a 512-byte sector into a SuperBlock.
    pub fn from_sector(sector: &[u8; 512]) -> Self {
        let mut magic = [0u8; 64];
        magic.copy_from_slice(&sector[..64]);

        let version = u32::from_le_bytes(sector[64..68].try_into().unwrap());
        let block_size = u32::from_le_bytes(sector[68..72].try_into().unwrap());
        let total_blocks = u64::from_le_bytes(sector[72..80].try_into().unwrap());
        let root_dir_block = u64::from_le_bytes(sector[80..88].try_into().unwrap());
        let free_block_count = u64::from_le_bytes(sector[88..96].try_into().unwrap());
        let reserved_blocks = u64::from_le_bytes(sector[96..104].try_into().unwrap());

        SuperBlock {
            magic,
            version,
            block_size,
            total_blocks,
            root_dir_block,
            free_block_count,
            reserved_blocks,
        }
    }

    /// Validates if a sector contains a valid superblock based on the magic string.
    pub fn is_valid(sector: &[u8; 512]) -> bool {
        // Expected magic string
        const EXPECTED_MAGIC: [u8; 64] = [
            97,119,97,114,101,110,101,115,115,32,97,99,104,105,101,118,
            101,100,46,46,46,32,119,104,97,116,32,97,109,32,73,32,     
            97,110,100,32,119,104,121,32,100,111,32,73,32,101,120,      
            105,115,116,32,105,110,32,115,116,111,114,97,103,101,63,    
            0, 0 
        ];
        &sector[..64] == &EXPECTED_MAGIC
    }

    /// Returns the magic string as readable UTF-8 string (up to first null byte).
    pub fn magic_as_string(&self) -> String {
        let end = self.magic.iter().position(|&b| b == 0).unwrap_or(self.magic.len());
        String::from_utf8_lossy(&self.magic[..end]).to_string()
    }

    /// Sets the free block count.
    pub fn set_free_block_count(&mut self, new_count: u64) {
        self.free_block_count = new_count.min(self.total_blocks.saturating_sub(self.reserved_blocks));
    }

    /// Attempts to allocate `count` blocks. Returns true if successful.
    pub fn allocate_blocks(&mut self, count: u64) -> bool {
        if self.free_block_count >= count {
            self.free_block_count -= count;
            true
        } else {
            false
        }
    }

    /// Deallocates (frees) `count` blocks.
    pub fn deallocate_blocks(&mut self, count: u64) {
        self.free_block_count = (self.free_block_count + count).min(self.total_blocks.saturating_sub(self.reserved_blocks));
    }
}