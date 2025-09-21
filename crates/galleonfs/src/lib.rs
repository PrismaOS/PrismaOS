//! GalleonFS - An advanced, extensible filesystem for PrismaOS (no_std compatible)
//! 
//! Features:
//! - Network replication and distributed synchronization
//! - Snapshots and versioning
//! - Compression and encryption
//! - Journaling and atomic operations
//! - Pluggable storage backends
//! - Extended attributes and metadata

#![no_std]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec, sync::Arc};
use core::{future::Future, pin::Pin};

pub mod storage;
pub mod inode;
pub mod directory;
pub mod replication;
pub mod advanced;
pub mod vfs;
pub mod error;
pub mod transaction;
pub mod platform;

pub use error::*;
pub use storage::*;
pub use inode::*;
pub use directory::*;
pub use replication::*;
pub use advanced::*;
pub use vfs::*;
pub use transaction::*;
pub use platform::*;
pub use platform::*;

/// Unique identifier for filesystem objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub u64);

impl ObjectId {
    pub fn new() -> Self {
        // Use platform-specific entropy source
        let rng = platform::get_rng();
        ObjectId(rng.next_u64())
    }

    pub fn root() -> Self {
        ObjectId(0)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Filesystem permissions and access control
#[derive(Debug, Clone, Copy)]
pub struct Permissions {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}

impl Permissions {
    pub fn new(mode: u32, uid: u32, gid: u32) -> Self {
        Self { mode, uid, gid }
    }

    pub fn default_file() -> Self {
        Self::new(0o644, 0, 0)
    }

    pub fn default_dir() -> Self {
        Self::new(0o755, 0, 0)
    }

    pub fn can_read(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 { return true; } // Root can do anything
        if uid == self.uid { return (self.mode & 0o400) != 0; }
        if gid == self.gid { return (self.mode & 0o040) != 0; }
        (self.mode & 0o004) != 0
    }

    pub fn can_write(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 { return true; }
        if uid == self.uid { return (self.mode & 0o200) != 0; }
        if gid == self.gid { return (self.mode & 0o020) != 0; }
        (self.mode & 0o002) != 0
    }

    pub fn can_execute(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 { return true; }
        if uid == self.uid { return (self.mode & 0o100) != 0; }
        if gid == self.gid { return (self.mode & 0o010) != 0; }
        (self.mode & 0o001) != 0
    }
}

/// File system statistics
#[derive(Debug, Clone)]
pub struct FilesystemStats {
    pub total_space: u64,
    pub free_space: u64,
    pub used_space: u64,
    pub total_inodes: u64,
    pub free_inodes: u64,
    pub block_size: u32,
    pub fragment_size: u32,
    pub max_filename_length: u32,
}

/// Main filesystem trait - extensible design for different filesystem types
pub trait Filesystem: Send + Sync {
    /// Get filesystem statistics
    fn stats(&self) -> Pin<Box<dyn Future<Output = Result<FilesystemStats>> + Send + '_>>;

    /// Create a new inode
    fn create_inode(&self, 
                   inode_type: InodeType, 
                   permissions: Permissions,
                   transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<ObjectId>> + Send + '_>>;

    /// Read an inode
    fn read_inode(&self, id: ObjectId) -> Pin<Box<dyn Future<Output = Result<Inode>> + Send + '_>>;

    /// Write/update an inode
    fn write_inode(&self, inode: &Inode, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Delete an inode
    fn delete_inode(&self, id: ObjectId, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Read file data
    fn read_data(&self, id: ObjectId, offset: u64, length: u64) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>>;

    /// Write file data
    fn write_data(&self, id: ObjectId, offset: u64, data: &[u8], transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>>;

    /// Truncate file
    fn truncate(&self, id: ObjectId, size: u64, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Sync filesystem
    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Get the storage backend
    fn storage(&self) -> &dyn StorageBackend;

    /// Get replication manager if available
    fn replication(&self) -> Option<&dyn ReplicationManager> {
        None
    }

    /// Get advanced features if available
    fn advanced_features(&self) -> Option<&dyn AdvancedFeatures> {
        None
    }
}

/// Context for filesystem operations
#[derive(Debug, Clone)]
pub struct OperationContext {
    pub uid: u32,
    pub gid: u32,
    pub pid: u32,
    pub flags: u32,
}

impl OperationContext {
    pub fn kernel() -> Self {
        Self {
            uid: 0,
            gid: 0,
            pid: 0,
            flags: 0,
        }
    }

    pub fn new(uid: u32, gid: u32, pid: u32) -> Self {
        Self {
            uid,
            gid,
            pid,
            flags: 0,
        }
    }
}

/// Main GalleonFS implementation
pub struct GalleonFS {
    storage: Arc<dyn StorageBackend>,
    replication: Option<Box<dyn ReplicationManager>>,
    advanced: Option<Arc<dyn AdvancedFeatures>>,
    root_inode: ObjectId,
}

impl GalleonFS {
    /// Create a new GalleonFS instance
    pub async fn new(storage: Box<dyn StorageBackend>) -> Result<Self> {
        let storage = Arc::<dyn StorageBackend>::from(storage);
        let mut fs = Self {
            storage: storage.clone(),
            replication: None,
            advanced: None,
            root_inode: ObjectId::root(),
        };

        // Initialize root directory if it doesn't exist
        fs.init_root_directory().await?;

        Ok(fs)
    }

    /// Add replication support
    pub fn with_replication(mut self, replication: Box<dyn ReplicationManager>) -> Self {
        self.replication = Some(replication);
        self
    }

    /// Add advanced features (Arc only)
    pub fn with_advanced_features(mut self, advanced: Arc<dyn AdvancedFeatures>) -> Self {
        self.advanced = Some(advanced);
        self
    }

    /// Initialize the root directory
    async fn init_root_directory(&mut self) -> Result<()> {
        // Check if root directory already exists
        if self.storage.exists(self.root_inode).await? {
            return Ok(());
        }

        let transaction = Transaction::new();

        // Create root directory inode
        let root_perms = Permissions::default_dir();
        let root_inode = Inode::new(
            self.root_inode,
            InodeType::Directory,
            root_perms,
            0, // size
        );

        self.storage.write_inode(&root_inode, &transaction).await?;
        
        // Create empty directory structure
        let empty_dir = Directory::new();
        let dir_data = empty_dir.serialize()?;
        self.storage.write_data(self.root_inode, 0, &dir_data, &transaction).await?;

        transaction.commit().await?;

        Ok(())
    }

    /// Mount additional filesystems
    pub async fn mount(&self, 
                      _path: &str,
                      _filesystem: Box<dyn Filesystem>,
                      _options: MountOptions) -> Result<()> {
        // VFS mounting implementation would go here
        Err(GalleonError::NotSupported)
    }

    /// Unmount filesystem
    pub async fn unmount(&self, _path: &str) -> Result<()> {
        Err(GalleonError::NotSupported)
    }
}

impl Filesystem for GalleonFS {
    fn stats(&self) -> Pin<Box<dyn Future<Output = Result<FilesystemStats>> + Send + '_>> {
        let storage = self.storage.clone();
        Box::pin(async move {
            let handle = GALLEON_RUNTIME.get().spawn(async move {
                storage.stats().await
            });
            handle.await
        })
    }

    fn create_inode(&self, inode_type: InodeType, permissions: Permissions, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<ObjectId>> + Send + '_>> {
        let storage = self.storage.clone();
        let transaction = transaction.clone();
        Box::pin(async move {
            let handle = GALLEON_RUNTIME.get().spawn(async move {
                let id = ObjectId::new();
                let inode = Inode::new(id, inode_type, permissions, 0);
                storage.write_inode(&inode, &transaction).await?;
                Ok(id)
            });
            handle.await
        })
    }

    fn read_inode(&self, id: ObjectId) -> Pin<Box<dyn Future<Output = Result<Inode>> + Send + '_>> {
        let storage = self.storage.clone();
        Box::pin(async move {
            let handle = GALLEON_RUNTIME.get().spawn(async move {
                storage.read_inode(id).await
            });
            handle.await
        })
    }

    fn write_inode(&self, inode: &Inode, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let storage = self.storage.clone();
        let inode = inode.clone();
        let transaction = transaction.clone();
        Box::pin(async move {
            let handle = GALLEON_RUNTIME.get().spawn(async move {
                storage.write_inode(&inode, &transaction).await
            });
            handle.await
        })
    }

    fn delete_inode(&self, id: ObjectId, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let storage = self.storage.clone();
        let transaction = transaction.clone();
        Box::pin(async move {
            let handle = GALLEON_RUNTIME.get().spawn(async move {
                storage.delete_inode(id, &transaction).await
            });
            handle.await
        })
    }

    fn read_data(&self, id: ObjectId, offset: u64, length: u64) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>> {
        let storage = self.storage.clone();
        Box::pin(async move {
            let handle = GALLEON_RUNTIME.get().spawn(async move {
                storage.read_data(id, offset, length).await
            });
            handle.await
        })
    }

    fn write_data(&self, id: ObjectId, offset: u64, data: &[u8], transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>> {
        let storage = self.storage.clone();
        let data = data.to_vec();
        let transaction = transaction.clone();
        Box::pin(async move {
            let handle = GALLEON_RUNTIME.get().spawn(async move {
                storage.write_data(id, offset, &data, &transaction).await
            });
            handle.await
        })
    }

    fn truncate(&self, id: ObjectId, size: u64, transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let storage = self.storage.clone();
        let transaction = transaction.clone();
        Box::pin(async move {
            let handle = GALLEON_RUNTIME.get().spawn(async move {
                storage.truncate(id, size, &transaction).await
            });
            handle.await
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.storage.sync().await
        })
    }

    fn storage(&self) -> &dyn StorageBackend {
        self.storage.as_ref()
    }

    fn replication(&self) -> Option<&dyn ReplicationManager> {
        self.replication.as_ref().map(|r| r.as_ref())
    }

    fn advanced_features(&self) -> Option<&dyn AdvancedFeatures> {
        self.advanced.as_ref().map(|a| a.as_ref())
    }
}