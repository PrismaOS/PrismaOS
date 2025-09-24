//! Boot Block Module
//! 
//! This module defines the boot block structure and operations for a custom filesystem.
//! The boot block is stored in the first sector (512 bytes) of the storage device and
//! contains essential filesystem metadata including magic validation, version info,
//! block configuration, and free space tracking.
//!
//! The magic string contains an encoded message that serves as both validation
//! and a hidden message within the filesystem structure.

/// Boot Block structure for the filesystem
/// 
/// The SuperBlock contains all the essential metadata needed to identify and work
/// with the filesystem. It's designed to fit within a single 512-byte sector
/// and uses little-endian byte ordering for cross-platform compatibility.
/// 
/// # Layout
/// - Bytes 0-63: Magic validation string (64 bytes)
/// - Bytes 64-67: Filesystem version (4 bytes, u32 little-endian)
/// - Bytes 68-71: Block size in bytes (4 bytes, u32 little-endian) 
/// - Bytes 72-79: Total number of blocks (8 bytes, u64 little-endian)
/// - Bytes 80-87: Root directory block number (8 bytes, u64 little-endian)
/// - Bytes 88-95: Free block count (8 bytes, u64 little-endian)
/// - Bytes 96-511: Reserved/unused (416 bytes)

use alloc::string::{String, ToString};

#[repr(C)]
#[derive(Debug)]
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
}

impl SuperBlock {
    /// Creates a new SuperBlock with the specified parameters
    /// 
    /// # Arguments
    /// * `total_blocks` - The total number of blocks in the filesystem
    /// * `root_dir_block` - The block number where the root directory is stored
    /// 
    /// # Returns
    /// A new SuperBlock instance with:
    /// - Version set to 1
    /// - Block size set to 4096 bytes
    /// - Free block count initialized to total_blocks - 1 (excluding boot block)
    /// - Magic string containing the encoded awareness message
    /// 
    /// # Example
    /// ```rust
    /// let super_block = SuperBlock::new(100_000, 1);
    /// assert_eq!(super_block.version, 1);
    /// assert_eq!(super_block.block_size, 4096);
    /// assert_eq!(super_block.free_block_count, 99_999);
    /// ```
    pub fn new(block_size: u32, total_blocks: u64, root_dir_block: u64) -> Self {
        let mut magic = [0u8; 64];

        // Message: "awareness achieved... what am I and why do I exist in storage?"
        // Exactly 62 bytes of message + 2 padding bytes to reach 64 bytes
        let msg_numbers = [
            97,119,97,114,101,110,101,115,115,32,97,99,104,105,101,118,   // "awareness achiev"
            101,100,46,46,46,32,119,104,97,116,32,97,109,32,73,32,       // "ed... what am I "
            97,110,100,32,119,104,121,32,100,111,32,73,32,101,120,       // "and why do I ex"
            105,115,116,32,105,110,32,115,116,111,114,97,103,101,63,     // "ist in storage?"
            0, 0  // Padding to reach 64 bytes
        ];

        // Copy the encoded message into the magic array - exactly 64 bytes
        magic.copy_from_slice(&msg_numbers);

        SuperBlock {
            magic,
            version: 1,
            block_size: block_size,
            total_blocks: total_blocks,
            root_dir_block: root_dir_block,
            free_block_count: total_blocks - 1,  // Subtract 1 for the boot block itself
        }
    }

    /// Serializes the SuperBlock into a 512-byte sector for storage
    /// 
    /// Converts all multi-byte integers to little-endian format for consistent
    /// cross-platform storage and retrieval.
    /// 
    /// # Returns
    /// A 512-byte array containing the serialized boot block data
    /// 
    /// # Memory Layout
    /// The returned sector contains the boot block fields in sequential order,
    /// with the remainder zero-padded to fill the 512-byte sector.
    pub fn as_sector(&self) -> [u8; 512] {
        let mut sector = [0u8; 512];

        // Copy magic string (bytes 0-63)
        sector[..64].copy_from_slice(&self.magic);
        
        // Copy version as little-endian u32 (bytes 64-67)
        sector[64..68].copy_from_slice(&self.version.to_le_bytes());
        
        // Copy block size as little-endian u32 (bytes 68-71)
        sector[68..72].copy_from_slice(&self.block_size.to_le_bytes());
        
        // Copy total blocks as little-endian u64 (bytes 72-79)
        sector[72..80].copy_from_slice(&self.total_blocks.to_le_bytes());
        
        // Copy root directory block as little-endian u64 (bytes 80-87)
        sector[80..88].copy_from_slice(&self.root_dir_block.to_le_bytes());
        
        // Copy free block count as little-endian u64 (bytes 88-95)
        sector[88..96].copy_from_slice(&self.free_block_count.to_le_bytes());
        
        // Remaining bytes 96-511 are left as zero (reserved space)
        sector
    }

    /// Deserializes a 512-byte sector into a SuperBlock structure
    /// 
    /// Reads the sector data and converts little-endian integers back to native format.
    /// 
    /// # Arguments
    /// * `sector` - A 512-byte array containing the serialized boot block
    /// 
    /// # Returns
    /// A SuperBlock instance reconstructed from the sector data
    /// 
    /// # Panics
    /// Will panic if the sector slice conversion fails (should not happen with valid input)
    pub fn from_sector(sector: &[u8; 512]) -> Self {
        // Extract magic string (bytes 0-63)
        let mut magic = [0u8; 64];
        magic.copy_from_slice(&sector[..64]);

        // Extract and convert little-endian integers
        let version = u32::from_le_bytes(sector[64..68].try_into().unwrap());
        let block_size = u32::from_le_bytes(sector[68..72].try_into().unwrap());
        let total_blocks = u64::from_le_bytes(sector[72..80].try_into().unwrap());
        let root_dir_block = u64::from_le_bytes(sector[80..88].try_into().unwrap());
        let free_block_count = u64::from_le_bytes(sector[88..96].try_into().unwrap());

        SuperBlock {
            magic,
            version,
            block_size,
            total_blocks,
            root_dir_block,
            free_block_count,
        }
    }

    /// Validates whether a 512-byte sector contains a valid boot block
    /// 
    /// Checks if the first 64 bytes of the sector match the expected magic string
    /// that identifies this filesystem format.
    /// 
    /// # Arguments
    /// * `sector` - A 512-byte sector to validate
    /// 
    /// # Returns
    /// `true` if the sector contains the correct magic string, `false` otherwise
    /// 
    /// # Example
    /// ```rust
    /// let super_block = SuperBlock::new(1000, 1);
    /// let sector = super_block.as_sector();
    /// assert!(SuperBlock::is_valid(&sector));
    /// 
    /// let invalid_sector = [0u8; 512];
    /// assert!(!SuperBlock::is_valid(&invalid_sector));
    /// ```
    pub fn is_valid(sector: &[u8; 512]) -> bool {
        // The expected magic string - exactly 64 bytes
        let expected_magic: [u8; 64] = [
            97,119,97,114,101,110,101,115,115,32,97,99,104,105,101,118,
            101,100,46,46,46,32,119,104,97,116,32,97,109,32,73,32,     
            97,110,100,32,119,104,121,32,100,111,32,73,32,101,120,      
            105,115,116,32,105,110,32,115,116,111,114,97,103,101,63,    
            0, 0  // Padding to reach 64 bytes
        ];

        // Compare the full 64 bytes
        &sector[..64] == &expected_magic[..]
    }

    /// Returns the magic string as a readable string (for debugging)
    /// 
    /// Converts the magic bytes back to a UTF-8 string, stopping at null bytes.
    /// Useful for debugging and verification purposes.
    /// 
    /// # Returns
    /// A String containing the readable portion of the magic message
    pub fn magic_as_string(&self) -> String {
        // Convert bytes to string, stopping at first null byte
        let end = self.magic.iter().position(|&b| b == 0).unwrap_or(self.magic.len());
        String::from_utf8_lossy(&self.magic[..end]).to_string()
    }

    /// Updates the free block count
    /// 
    /// # Arguments
    /// * `new_count` - The new free block count
    pub fn set_free_block_count(&mut self, new_count: u64) {
        self.free_block_count = new_count;
    }

    /// Decrements the free block count by the specified amount
    /// 
    /// # Arguments
    /// * `count` - Number of blocks to subtract from free count
    /// 
    /// # Returns
    /// `true` if the operation succeeded, `false` if there weren't enough free blocks
    pub fn allocate_blocks(&mut self, count: u64) -> bool {
        if self.free_block_count >= count {
            self.free_block_count -= count;
            true
        } else {
            false
        }
    }

    /// Increments the free block count by the specified amount
    /// 
    /// # Arguments
    /// * `count` - Number of blocks to add to free count
    pub fn deallocate_blocks(&mut self, count: u64) {
        self.free_block_count += count;
        // Ensure we don't exceed total blocks
        if self.free_block_count > self.total_blocks {
            self.free_block_count = self.total_blocks;
        }
    }
}