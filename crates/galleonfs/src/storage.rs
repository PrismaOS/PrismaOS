//! Storage backend abstraction for GalleonFS (no_std compatible)
//! 
//! This module provides pluggable storage backends including:
//! - Memory storage for testing
//! - Platform storage for embedded systems
//! - Network storage for distributed systems
//! - Hybrid storage with caching

// #![no_std] // Only at crate root

extern crate alloc;

use alloc::{boxed::Box, vec::Vec, collections::BTreeMap, string::String};
use core::{future::Future, pin::Pin};
use super::{Result, ObjectId, Inode, FilesystemStats, Transaction, GalleonError};

/// Storage backend trait - allows different storage implementations
pub trait StorageBackend: Send + Sync {
    /// Check if an object exists
    fn exists(&self, id: ObjectId) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>>;

    /// Read an inode from storage
    fn read_inode(&self, id: ObjectId) -> Pin<Box<dyn Future<Output = Result<Inode>> + Send + '_>>;

    /// Write an inode to storage
    fn write_inode(&self, inode: &Inode, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Delete an inode from storage
    fn delete_inode(&self, id: ObjectId, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Read data from storage
    fn read_data(&self, id: ObjectId, offset: u64, length: u64) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>>;

    /// Write data to storage
    fn write_data(&self, id: ObjectId, offset: u64, data: &[u8], transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>>;

    /// Truncate data
    fn truncate(&self, id: ObjectId, size: u64, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Get storage statistics
    fn stats(&self) -> Pin<Box<dyn Future<Output = Result<FilesystemStats>> + Send + '_>>;

    /// Sync all pending operations
    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Check storage integrity
    fn check_integrity(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>>;

    /// Allocate space for data
    fn allocate(&self, size: u64, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>>;

    /// Deallocate space
    fn deallocate(&self, offset: u64, size: u64, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Get backend capabilities
    fn capabilities(&self) -> StorageCapabilities;
}

/// Storage backend capabilities
#[derive(Debug, Clone)]
pub struct StorageCapabilities {
    pub supports_transactions: bool,
    pub supports_compression: bool,
    pub supports_encryption: bool,
    pub supports_snapshots: bool,
    pub supports_deduplication: bool,
    pub supports_async_io: bool,
    pub max_file_size: u64,
    pub max_filename_length: u32,
    pub block_size: u32,
}

impl Default for StorageCapabilities {
    fn default() -> Self {
        Self {
            supports_transactions: true,
            supports_compression: false,
            supports_encryption: false,
            supports_snapshots: false,
            supports_deduplication: false,
            supports_async_io: true,
            max_file_size: u64::MAX,
            max_filename_length: 255,
            block_size: 4096,
        }
    }
}

/// In-memory storage backend for testing and temporary filesystems
pub struct MemoryStorage {
    inodes: spin::Mutex<BTreeMap<ObjectId, Inode>>,
    data: spin::Mutex<BTreeMap<ObjectId, Vec<u8>>>,
    capabilities: StorageCapabilities,
    total_space: u64,
}

impl MemoryStorage {
    pub fn new(total_space: u64) -> Self {
        Self {
            inodes: spin::Mutex::new(BTreeMap::new()),
            data: spin::Mutex::new(BTreeMap::new()),
            capabilities: StorageCapabilities::default(),
            total_space,
        }
    }

    fn used_space(&self) -> u64 {
        let data = self.data.lock();
        data.values().map(|v| v.len() as u64).sum()
    }
}

impl StorageBackend for MemoryStorage {
    fn exists(&self, id: ObjectId) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        Box::pin(async move {
            let inodes = self.inodes.lock();
            Ok(inodes.contains_key(&id))
        })
    }

    fn read_inode(&self, id: ObjectId) -> Pin<Box<dyn Future<Output = Result<Inode>> + Send + '_>> {
        Box::pin(async move {
            let inodes = self.inodes.lock();
            inodes.get(&id)
                .cloned()
                .ok_or(GalleonError::NotFound)
        })
    }

    fn write_inode(&self, inode: &Inode, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let inode = inode.clone();
        Box::pin(async move {
            let mut inodes = self.inodes.lock();
            inodes.insert(inode.id(), inode);
            Ok(())
        })
    }

    fn delete_inode(&self, id: ObjectId, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            let mut inodes = self.inodes.lock();
            let mut data = self.data.lock();
            
            inodes.remove(&id);
            data.remove(&id);
            
            Ok(())
        })
    }

    fn read_data(&self, id: ObjectId, offset: u64, length: u64) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>> {
        Box::pin(async move {
            let data = self.data.lock();
            if let Some(file_data) = data.get(&id) {
                let start = offset as usize;
                let end = ((offset + length) as usize).min(file_data.len());
                
                if start >= file_data.len() {
                    Ok(Vec::new())
                } else {
                    Ok(file_data[start..end].to_vec())
                }
            } else {
                Err(GalleonError::NotFound)
            }
        })
    }

    fn write_data(&self, id: ObjectId, offset: u64, new_data: &[u8], _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>> {
        let new_data = new_data.to_vec();
        Box::pin(async move {
            let mut data = self.data.lock();
            
            // Check space constraints
            let current_size = data.get(&id).map(|v| v.len()).unwrap_or(0);
            let new_size = (offset as usize + new_data.len()).max(current_size);
            let size_increase = new_size.saturating_sub(current_size) as u64;
            
            if self.used_space() + size_increase > self.total_space {
                return Err(GalleonError::NoSpace);
            }

            let file_data = data.entry(id).or_insert_with(Vec::new);
            
            // Extend file if necessary
            if offset as usize + new_data.len() > file_data.len() {
                file_data.resize(offset as usize + new_data.len(), 0);
            }
            
            // Write data
            file_data[offset as usize..offset as usize + new_data.len()]
                .copy_from_slice(&new_data);
            
            Ok(new_data.len() as u64)
        })
    }

    fn truncate(&self, id: ObjectId, size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            let mut data = self.data.lock();
            if let Some(file_data) = data.get_mut(&id) {
                file_data.resize(size as usize, 0);
                Ok(())
            } else {
                Err(GalleonError::NotFound)
            }
        })
    }

    fn stats(&self) -> Pin<Box<dyn Future<Output = Result<FilesystemStats>> + Send + '_>> {
        Box::pin(async move {
            let used = self.used_space();
            let inodes = self.inodes.lock();
            
            Ok(FilesystemStats {
                total_space: self.total_space,
                free_space: self.total_space.saturating_sub(used),
                used_space: used,
                total_inodes: 1024 * 1024, // Arbitrary limit for memory FS
                free_inodes: (1024 * 1024) - inodes.len() as u64,
                block_size: 4096,
                fragment_size: 4096,
                max_filename_length: 255,
            })
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // Memory storage is always synced
            Ok(())
        })
    }

    fn check_integrity(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement integrity checks
            Ok(Vec::new())
        })
    }

    fn allocate(&self, size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>> {
        Box::pin(async move {
            if self.used_space() + size > self.total_space {
                Err(GalleonError::NoSpace)
            } else {
                // For memory storage, we don't pre-allocate
                Ok(0)
            }
        })
    }

    fn deallocate(&self, _offset: u64, _size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // For memory storage, deallocation is automatic
            Ok(())
        })
    }

    fn capabilities(&self) -> StorageCapabilities {
        self.capabilities.clone()
    }
}

/// Platform storage backend for embedded systems
pub struct PlatformStorage {
    device: Box<dyn super::platform::StorageDevice + Send + Sync>,
    capabilities: StorageCapabilities,
    block_cache: spin::Mutex<BTreeMap<u64, Vec<u8>>>,
    cache_size: usize,
}

impl PlatformStorage {
    pub fn new(device: Box<dyn super::platform::StorageDevice + Send + Sync>, cache_size: usize) -> Self {
unsafe impl Send for PlatformStorage {}
unsafe impl Sync for PlatformStorage {}
        let mut capabilities = StorageCapabilities::default();
        capabilities.block_size = device.block_size();
        capabilities.max_file_size = device.capacity();
        
        Self {
            device,
            capabilities,
            block_cache: spin::Mutex::new(BTreeMap::new()),
            cache_size,
        }
    }

    fn read_block(&self, block_number: u64) -> Result<Vec<u8>> {
        // Check cache first
        {
            let cache = self.block_cache.lock();
            if let Some(data) = cache.get(&block_number) {
                return Ok(data.clone());
            }
        }

        // Read from device
        let block_size = self.device.block_size() as usize;
        let mut buffer = alloc::vec![0u8; block_size];
        let offset = block_number * self.device.block_size() as u64;
        
        match self.device.read(offset, &mut buffer) {
            Ok(_) => {
                // Cache the block
                self.cache_block(block_number, buffer.clone());
                Ok(buffer)
            }
            Err(e) => Err(GalleonError::IoError(e)),
        }
    }

    fn write_block(&self, block_number: u64, data: &[u8]) -> Result<()> {
        let offset = block_number * self.device.block_size() as u64;
        
        match self.device.write(offset, data) {
            Ok(_) => {
                // Update cache
                self.cache_block(block_number, data.to_vec());
                Ok(())
            }
            Err(e) => Err(GalleonError::IoError(e)),
        }
    }

    fn cache_block(&self, block_number: u64, data: Vec<u8>) {
        let mut cache = self.block_cache.lock();
        
        // Evict old blocks if cache is full
        if cache.len() >= self.cache_size {
            if let Some(oldest_key) = cache.keys().next().copied() {
                cache.remove(&oldest_key);
            }
        }
        
        cache.insert(block_number, data);
    }
}

impl StorageBackend for PlatformStorage {
    fn exists(&self, _id: ObjectId) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement existence check based on platform storage layout
            Err(GalleonError::NotSupported)
        })
    }

    fn read_inode(&self, _id: ObjectId) -> Pin<Box<dyn Future<Output = Result<Inode>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement inode reading from platform storage
            Err(GalleonError::NotSupported)
        })
    }

    fn write_inode(&self, _inode: &Inode, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement inode writing to platform storage
            Err(GalleonError::NotSupported)
        })
    }

    fn delete_inode(&self, _id: ObjectId, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement inode deletion from platform storage
            Err(GalleonError::NotSupported)
        })
    }

    fn read_data(&self, _id: ObjectId, _offset: u64, _length: u64) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement data reading from platform storage
            Err(GalleonError::NotSupported)
        })
    }

    fn write_data(&self, _id: ObjectId, _offset: u64, _data: &[u8], _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement data writing to platform storage
            Err(GalleonError::NotSupported)
        })
    }

    fn truncate(&self, _id: ObjectId, _size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement truncate for platform storage
            Err(GalleonError::NotSupported)
        })
    }

    fn stats(&self) -> Pin<Box<dyn Future<Output = Result<FilesystemStats>> + Send + '_>> {
        Box::pin(async move {
            Ok(FilesystemStats {
                total_space: self.device.capacity(),
                free_space: self.device.capacity(), // TODO: Calculate actual free space
                used_space: 0, // TODO: Calculate actual used space
                total_inodes: 65536, // Platform dependent
                free_inodes: 65536, // TODO: Calculate actual free inodes
                block_size: self.device.block_size(),
                fragment_size: self.device.block_size(),
                max_filename_length: 255,
            })
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            match self.device.flush() {
                Ok(()) => Ok(()),
                Err(e) => Err(GalleonError::IoError(e)),
            }
        })
    }

    fn check_integrity(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement platform storage integrity check
            Ok(Vec::new())
        })
    }

    fn allocate(&self, _size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement space allocation for platform storage
            Err(GalleonError::NotSupported)
        })
    }

    fn deallocate(&self, _offset: u64, _size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement space deallocation for platform storage
            Err(GalleonError::NotSupported)
        })
    }

    fn capabilities(&self) -> StorageCapabilities {
        self.capabilities.clone()
    }
}

/// Consistency level for network storage
#[derive(Debug, Clone)]
pub enum ConsistencyLevel {
    /// Eventually consistent (best performance)
    Eventual,
    /// Read from any, write to majority
    Quorum,
    /// Read/write from all replicas
    Strong,
}

/// Network storage backend for distributed filesystems (simplified for no_std)
pub struct NetworkStorage {
    primary_node: String,
    replica_nodes: Vec<String>,
    capabilities: StorageCapabilities,
    consistency_level: ConsistencyLevel,
}

impl NetworkStorage {
    pub fn new(primary: String, replicas: Vec<String>) -> Self {
        let mut capabilities = StorageCapabilities::default();
        capabilities.supports_async_io = true;
        capabilities.supports_deduplication = true;
        
        Self {
            primary_node: primary,
            replica_nodes: replicas,
            capabilities,
            consistency_level: ConsistencyLevel::Quorum,
        }
    }

    pub fn with_consistency(mut self, level: ConsistencyLevel) -> Self {
        self.consistency_level = level;
        self
    }
}

impl StorageBackend for NetworkStorage {
    fn exists(&self, _id: ObjectId) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network existence check
            Err(GalleonError::NotSupported)
        })
    }

    fn read_inode(&self, _id: ObjectId) -> Pin<Box<dyn Future<Output = Result<Inode>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network inode read
            Err(GalleonError::NotSupported)
        })
    }

    fn write_inode(&self, _inode: &Inode, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network inode write
            Err(GalleonError::NotSupported)
        })
    }

    fn delete_inode(&self, _id: ObjectId, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network inode delete
            Err(GalleonError::NotSupported)
        })
    }

    fn read_data(&self, _id: ObjectId, _offset: u64, _length: u64) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network data read
            Err(GalleonError::NotSupported)
        })
    }

    fn write_data(&self, _id: ObjectId, _offset: u64, _data: &[u8], _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network data write
            Err(GalleonError::NotSupported)
        })
    }

    fn truncate(&self, _id: ObjectId, _size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network truncate
            Err(GalleonError::NotSupported)
        })
    }

    fn stats(&self) -> Pin<Box<dyn Future<Output = Result<FilesystemStats>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network stats
            Err(GalleonError::NotSupported)
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network sync
            Err(GalleonError::NotSupported)
        })
    }

    fn check_integrity(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network integrity check
            Err(GalleonError::NotSupported)
        })
    }

    fn allocate(&self, _size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network allocation
            Err(GalleonError::NotSupported)
        })
    }

    fn deallocate(&self, _offset: u64, _size: u64, _transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // TODO: Implement network deallocation
            Err(GalleonError::NotSupported)
        })
    }

    fn capabilities(&self) -> StorageCapabilities {
        self.capabilities.clone()
    }
}