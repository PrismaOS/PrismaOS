//! Inode and metadata system for GalleonFS (no_std compatible)
//! 
//! Features:
//! - Extensible inode structure
//! - Extended attributes
//! - Versioning support
//! - Access control lists
//! - Metadata caching

// #![no_std] // Only at crate root

extern crate alloc;

use alloc::{vec::Vec, collections::BTreeMap, string::String};
use core::{fmt, time::Duration};
use super::{ObjectId, Permissions, Result, platform::Timestamp};

/// Inode type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    RegularFile,
    Directory,
    SymbolicLink,
    BlockDevice,
    CharacterDevice,
    Fifo,
    Socket,
    Snapshot,
    HardLink,
}

impl fmt::Display for InodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            InodeType::RegularFile => "regular file",
            InodeType::Directory => "directory",
            InodeType::SymbolicLink => "symbolic link",
            InodeType::BlockDevice => "block device",
            InodeType::CharacterDevice => "character device",
            InodeType::Fifo => "FIFO",
            InodeType::Socket => "socket",
            InodeType::Snapshot => "snapshot",
            InodeType::HardLink => "hard link",
        };
        write!(f, "{}", s)
    }
}

/// Extended attribute value
#[derive(Debug, Clone)]
pub enum ExtendedAttributeValue {
    String(String),
    Binary(Vec<u8>),
    Integer(i64),
    Boolean(bool),
}

/// Extended attributes for inodes
pub type ExtendedAttributes = BTreeMap<String, ExtendedAttributeValue>;

/// Access Control List entry
#[derive(Debug, Clone)]
pub struct AclEntry {
    pub entry_type: AclEntryType,
    pub principal: u32, // uid or gid
    pub permissions: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum AclEntryType {
    User,
    Group,
    Other,
    Mask,
}

/// Access Control List
pub type AccessControlList = Vec<AclEntry>;

/// Version information for versioned files
#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub version_number: u64,
    pub parent_version: Option<u64>,
    pub created_at: Timestamp,
    pub created_by: u32,
    pub description: String,
    pub checksum: Option<[u8; 32]>, // SHA-256
}

/// Inode structure with extensible metadata
#[derive(Debug, Clone)]
pub struct Inode {
    /// Unique identifier
    id: ObjectId,
    
    /// Inode type
    inode_type: InodeType,
    
    /// Standard permissions
    permissions: Permissions,
    
    /// File size in bytes
    size: u64,
    
    /// Number of hard links
    link_count: u32,
    
    /// Timestamps
    created_at: Timestamp,
    modified_at: Timestamp,
    accessed_at: Timestamp,
    changed_at: Timestamp, // metadata change time
    
    /// Block allocation information
    blocks: Vec<u64>,
    indirect_blocks: Vec<u64>,
    
    /// Extended attributes
    extended_attributes: ExtendedAttributes,
    
    /// Access Control List
    acl: Option<AccessControlList>,
    
    /// Version information (for versioned files)
    version_info: Option<VersionInfo>,
    
    /// Compression information
    compression: Option<CompressionInfo>,
    
    /// Encryption information  
    encryption: Option<EncryptionInfo>,
    
    /// Deduplication information
    dedup_hash: Option<[u8; 32]>,
    
    /// Replication metadata
    replication_meta: Option<ReplicationMetadata>,
    
    /// Custom metadata for filesystem extensions
    custom_metadata: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct CompressionInfo {
    pub algorithm: CompressionAlgorithm,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub compression_ratio: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum CompressionAlgorithm {
    None,
    Lz4,
    Zstd,
    Gzip,
    Brotli,
}

#[derive(Debug, Clone)]
pub struct EncryptionInfo {
    pub algorithm: EncryptionAlgorithm,
    pub key_id: u64,
    pub iv: Vec<u8>,
    pub authenticated: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum EncryptionAlgorithm {
    None,
    Aes256Gcm,
    ChaCha20Poly1305,
    Aes256Ctr,
}

#[derive(Debug, Clone)]
pub struct ReplicationMetadata {
    pub replica_count: u32,
    pub replicas: Vec<String>, // Node identifiers
    pub consistency_level: String,
    pub last_synchronized: Timestamp,
    pub conflict_version: Option<u64>,
}

impl Inode {
    /// Create a new inode
    pub fn new(id: ObjectId, inode_type: InodeType, permissions: Permissions, size: u64) -> Self {
        let now = Timestamp::now();
        
        Self {
            id,
            inode_type,
            permissions,
            size,
            link_count: 1,
            created_at: now,
            modified_at: now,
            accessed_at: now,
            changed_at: now,
            blocks: Vec::new(),
            indirect_blocks: Vec::new(),
            extended_attributes: BTreeMap::new(),
            acl: None,
            version_info: None,
            compression: None,
            encryption: None,
            dedup_hash: None,
            replication_meta: None,
            custom_metadata: BTreeMap::new(),
        }
    }

    // Getters
    pub fn id(&self) -> ObjectId { self.id }
    pub fn inode_type(&self) -> InodeType { self.inode_type }
    pub fn permissions(&self) -> &Permissions { &self.permissions }
    pub fn size(&self) -> u64 { self.size }
    pub fn link_count(&self) -> u32 { self.link_count }
    pub fn created_at(&self) -> Timestamp { self.created_at }
    pub fn modified_at(&self) -> Timestamp { self.modified_at }
    pub fn accessed_at(&self) -> Timestamp { self.accessed_at }
    pub fn changed_at(&self) -> Timestamp { self.changed_at }
    pub fn blocks(&self) -> &Vec<u64> { &self.blocks }
    pub fn extended_attributes(&self) -> &ExtendedAttributes { &self.extended_attributes }
    pub fn acl(&self) -> Option<&AccessControlList> { self.acl.as_ref() }
    pub fn version_info(&self) -> Option<&VersionInfo> { self.version_info.as_ref() }
    pub fn compression(&self) -> Option<&CompressionInfo> { self.compression.as_ref() }
    pub fn encryption(&self) -> Option<&EncryptionInfo> { self.encryption.as_ref() }
    pub fn dedup_hash(&self) -> Option<&[u8; 32]> { self.dedup_hash.as_ref() }
    pub fn replication_meta(&self) -> Option<&ReplicationMetadata> { self.replication_meta.as_ref() }

    // Setters
    pub fn set_size(&mut self, size: u64) {
        self.size = size;
        self.modified_at = Timestamp::now();
        self.changed_at = Timestamp::now();
    }

    pub fn set_permissions(&mut self, permissions: Permissions) {
        self.permissions = permissions;
        self.changed_at = Timestamp::now();
    }

    pub fn increment_link_count(&mut self) {
        self.link_count += 1;
        self.changed_at = Timestamp::now();
    }

    pub fn decrement_link_count(&mut self) {
        if self.link_count > 0 {
            self.link_count -= 1;
        }
        self.changed_at = Timestamp::now();
    }

    pub fn touch_accessed(&mut self) {
        self.accessed_at = Timestamp::now();
    }

    pub fn touch_modified(&mut self) {
        self.modified_at = Timestamp::now();
        self.changed_at = Timestamp::now();
    }

    pub fn add_block(&mut self, block: u64) {
        self.blocks.push(block);
        self.changed_at = Timestamp::now();
    }

    pub fn remove_block(&mut self, block: u64) {
        self.blocks.retain(|&b| b != block);
        self.changed_at = Timestamp::now();
    }

    // Extended attributes
    pub fn set_extended_attribute(&mut self, name: String, value: ExtendedAttributeValue) {
        self.extended_attributes.insert(name, value);
        self.changed_at = Timestamp::now();
    }

    pub fn get_extended_attribute(&self, name: &str) -> Option<&ExtendedAttributeValue> {
        self.extended_attributes.get(name)
    }

    pub fn remove_extended_attribute(&mut self, name: &str) -> Option<ExtendedAttributeValue> {
        let result = self.extended_attributes.remove(name);
        if result.is_some() {
            self.changed_at = Timestamp::now();
        }
        result
    }

    // Access Control List
    pub fn set_acl(&mut self, acl: AccessControlList) {
        self.acl = Some(acl);
        self.changed_at = Timestamp::now();
    }

    pub fn clear_acl(&mut self) {
        self.acl = None;
        self.changed_at = Timestamp::now();
    }

    // Versioning
    pub fn set_version_info(&mut self, version_info: VersionInfo) {
        self.version_info = Some(version_info);
        self.changed_at = Timestamp::now();
    }

    pub fn create_new_version(&mut self, description: String, created_by: u32) -> u64 {
        let new_version = self.version_info
            .as_ref()
            .map(|v| v.version_number + 1)
            .unwrap_or(1);

        let parent_version = self.version_info
            .as_ref()
            .map(|v| v.version_number);

        self.version_info = Some(VersionInfo {
            version_number: new_version,
            parent_version,
            created_at: Timestamp::now(),
            created_by,
            description,
            checksum: None,
        });

        self.changed_at = Timestamp::now();
        new_version
    }

    // Compression
    pub fn set_compression(&mut self, compression: CompressionInfo) {
        self.compression = Some(compression);
        self.changed_at = Timestamp::now();
    }

    pub fn clear_compression(&mut self) {
        self.compression = None;
        self.changed_at = Timestamp::now();
    }

    // Encryption
    pub fn set_encryption(&mut self, encryption: EncryptionInfo) {
        self.encryption = Some(encryption);
        self.changed_at = Timestamp::now();
    }

    pub fn clear_encryption(&mut self) {
        self.encryption = None;
        self.changed_at = Timestamp::now();
    }

    // Deduplication
    pub fn set_dedup_hash(&mut self, hash: [u8; 32]) {
        self.dedup_hash = Some(hash);
        self.changed_at = Timestamp::now();
    }

    pub fn clear_dedup_hash(&mut self) {
        self.dedup_hash = None;
        self.changed_at = Timestamp::now();
    }

    // Replication
    pub fn set_replication_meta(&mut self, meta: ReplicationMetadata) {
        self.replication_meta = Some(meta);
        self.changed_at = Timestamp::now();
    }

    pub fn clear_replication_meta(&mut self) {
        self.replication_meta = None;
        self.changed_at = Timestamp::now();
    }

    // Custom metadata
    pub fn set_custom_metadata(&mut self, key: String, value: Vec<u8>) {
        self.custom_metadata.insert(key, value);
        self.changed_at = Timestamp::now();
    }

    pub fn get_custom_metadata(&self, key: &str) -> Option<&Vec<u8>> {
        self.custom_metadata.get(key)
    }

    pub fn remove_custom_metadata(&mut self, key: &str) -> Option<Vec<u8>> {
        let result = self.custom_metadata.remove(key);
        if result.is_some() {
            self.changed_at = Timestamp::now();
        }
        result
    }

    // Serialization (for storage)
    pub fn serialize(&self) -> Result<Vec<u8>> {
        // TODO: Implement proper serialization (could use bincode, protobuf, etc.)
        // For now, return a placeholder
        Err(super::GalleonError::NotSupported)
    }

    pub fn deserialize(_data: &[u8]) -> Result<Self> {
        // TODO: Implement proper deserialization
        // For now, return an error
        Err(super::GalleonError::NotSupported)
    }

    // Check if inode is a specific type
    pub fn is_file(&self) -> bool {
        matches!(self.inode_type, InodeType::RegularFile)
    }

    pub fn is_directory(&self) -> bool {
        matches!(self.inode_type, InodeType::Directory)
    }

    pub fn is_symlink(&self) -> bool {
        matches!(self.inode_type, InodeType::SymbolicLink)
    }

    pub fn is_device(&self) -> bool {
        matches!(self.inode_type, InodeType::BlockDevice | InodeType::CharacterDevice)
    }

    // Check permissions
    pub fn can_read(&self, uid: u32, gid: u32) -> bool {
        // Check ACL first if present
        if let Some(acl) = &self.acl {
            return self.check_acl_permission(acl, uid, gid, 0o4);
        }
        
        // Fall back to standard permissions
        self.permissions.can_read(uid, gid)
    }

    pub fn can_write(&self, uid: u32, gid: u32) -> bool {
        if let Some(acl) = &self.acl {
            return self.check_acl_permission(acl, uid, gid, 0o2);
        }
        
        self.permissions.can_write(uid, gid)
    }

    pub fn can_execute(&self, uid: u32, gid: u32) -> bool {
        if let Some(acl) = &self.acl {
            return self.check_acl_permission(acl, uid, gid, 0o1);
        }
        
        self.permissions.can_execute(uid, gid)
    }

    fn check_acl_permission(&self, acl: &AccessControlList, uid: u32, gid: u32, permission: u32) -> bool {
        // Root can do anything
        if uid == 0 {
            return true;
        }

        // Check user-specific entries
        for entry in acl {
            match entry.entry_type {
                AclEntryType::User if entry.principal == uid => {
                    return (entry.permissions & permission) != 0;
                }
                AclEntryType::Group if entry.principal == gid => {
                    return (entry.permissions & permission) != 0;
                }
                _ => {}
            }
        }

        // Fall back to standard permissions
        self.permissions.can_read(uid, gid)
    }

    // Calculate storage requirements
    pub fn storage_size(&self) -> u64 {
        // Base inode size
        let mut size = 512; // Approximate base size
        
        // Add extended attributes
        for (key, value) in &self.extended_attributes {
            size += key.len() as u64;
            size += match value {
                ExtendedAttributeValue::String(s) => s.len() as u64,
                ExtendedAttributeValue::Binary(b) => b.len() as u64,
                ExtendedAttributeValue::Integer(_) => 8,
                ExtendedAttributeValue::Boolean(_) => 1,
            };
        }
        
        // Add ACL size
        if let Some(acl) = &self.acl {
            size += acl.len() as u64 * 16; // Approximate ACL entry size
        }
        
        // Add custom metadata
        for (key, value) in &self.custom_metadata {
            size += key.len() as u64 + value.len() as u64;
        }
        
        size
    }
}

/// Inode cache for performance optimization (no_std compatible)
pub struct InodeCache {
    cache: spin::Mutex<BTreeMap<ObjectId, (Inode, Timestamp)>>,
    max_entries: usize,
    ttl: Duration,
}

impl InodeCache {
    pub const fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            cache: spin::Mutex::new(BTreeMap::new()),
            max_entries,
            ttl,
        }
    }

    pub fn get(&self, id: ObjectId) -> Option<Inode> {
        let mut cache = self.cache.lock();
        
        if let Some((inode, timestamp)) = cache.get(&id) {
            // Check if entry is still valid
            let now = Timestamp::now();
            if self.is_valid_timestamp(*timestamp, now) {
                return Some(inode.clone());
            } else {
                // Remove expired entry
                cache.remove(&id);
            }
        }
        
        None
    }

    pub fn put(&self, inode: Inode) {
        let mut cache = self.cache.lock();
        
        // Evict old entries if cache is full
        if cache.len() >= self.max_entries {
            self.evict_oldest(&mut cache);
        }
        
        cache.insert(inode.id(), (inode, Timestamp::now()));
    }

    pub fn remove(&self, id: ObjectId) {
        let mut cache = self.cache.lock();
        cache.remove(&id);
    }

    pub fn clear(&self) {
        let mut cache = self.cache.lock();
        cache.clear();
    }

    fn is_valid_timestamp(&self, cached: Timestamp, now: Timestamp) -> bool {
        let elapsed = now.elapsed_since(cached);
        elapsed < self.ttl
    }

    fn evict_oldest(&self, cache: &mut BTreeMap<ObjectId, (Inode, Timestamp)>) {
        if let Some(oldest_key) = cache.iter()
            .min_by_key(|(_, (_, timestamp))| *timestamp)
            .map(|(id, _)| *id) {
            cache.remove(&oldest_key);
        }
    }
}