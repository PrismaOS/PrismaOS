//! Directory operations for GalleonFS

use alloc::{boxed::Box, collections::BTreeMap, string::{String, ToString}, vec::Vec};
use core::{future::Future, pin::Pin};
use super::{Result, ObjectId, Inode, InodeType, Permissions, Timestamp, OperationContext, Transaction, GalleonError};

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    pub object_id: ObjectId,
    pub inode_type: InodeType,
}

impl DirectoryEntry {
    pub fn new(name: String, object_id: ObjectId, inode_type: InodeType) -> Self {
        Self {
            name,
            object_id,
            inode_type,
        }
    }
}

/// Directory structure
#[derive(Debug, Clone)]
pub struct Directory {
    entries: BTreeMap<String, DirectoryEntry>,
}

impl Directory {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn add_entry(&mut self, entry: DirectoryEntry) -> Result<()> {
        if self.entries.contains_key(&entry.name) {
            return Err(GalleonError::AlreadyExists);
        }
        
        self.entries.insert(entry.name.clone(), entry);
        Ok(())
    }

    pub fn remove_entry(&mut self, name: &str) -> Option<DirectoryEntry> {
        self.entries.remove(name)
    }

    pub fn get_entry(&self, name: &str) -> Option<&DirectoryEntry> {
        self.entries.get(name)
    }

    pub fn entries(&self) -> impl Iterator<Item = &DirectoryEntry> {
        self.entries.values()
    }

    pub fn entry_names(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        // Simple serialization format:
        // [entry_count: 4 bytes][entries...]
        // Each entry: [name_len: 2 bytes][name][object_id: 8 bytes][inode_type: 1 byte]
        
        let mut data = Vec::new();
        
        // Entry count
        data.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        
        // Entries
        for entry in self.entries.values() {
            // Name length and name
            let name_bytes = entry.name.as_bytes();
            if name_bytes.len() > u16::MAX as usize {
                return Err(GalleonError::NameTooLong);
            }
            data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            data.extend_from_slice(name_bytes);
            
            // Object ID
            data.extend_from_slice(&entry.object_id.as_u64().to_le_bytes());
            
            // Inode type
            let inode_type_byte = match entry.inode_type {
                InodeType::RegularFile => 0,
                InodeType::Directory => 1,
                InodeType::SymbolicLink => 2,
                InodeType::BlockDevice => 3,
                InodeType::CharacterDevice => 4,
                InodeType::Fifo => 5,
                InodeType::Socket => 6,
                InodeType::Snapshot => 7,
                InodeType::HardLink => 8,
            };
            data.push(inode_type_byte);
        }
        
        Ok(data)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(GalleonError::Corruption("Invalid directory data".into()));
        }

        let mut offset = 0;
        
        // Read entry count
        let entry_count = u32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]) as usize;
        offset += 4;

        let mut directory = Directory::new();

        // Read entries
        for _ in 0..entry_count {
            if offset + 2 > data.len() {
                return Err(GalleonError::Corruption("Truncated directory data".into()));
            }

            // Read name length
            let name_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;

            if offset + name_len + 8 + 1 > data.len() {
                return Err(GalleonError::Corruption("Truncated directory entry".into()));
            }

            // Read name
            let name = String::from_utf8(data[offset..offset + name_len].to_vec())
                .map_err(|_| GalleonError::Corruption("Invalid UTF-8 in filename".into()))?;
            offset += name_len;

            // Read object ID
            let object_id = ObjectId(u64::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]
            ]));
            offset += 8;

            // Read inode type
            let inode_type = match data[offset] {
                0 => InodeType::RegularFile,
                1 => InodeType::Directory,
                2 => InodeType::SymbolicLink,
                3 => InodeType::BlockDevice,
                4 => InodeType::CharacterDevice,
                5 => InodeType::Fifo,
                6 => InodeType::Socket,
                7 => InodeType::Snapshot,
                8 => InodeType::HardLink,
                _ => return Err(GalleonError::Corruption("Invalid inode type".into())),
            };
            offset += 1;

            let entry = DirectoryEntry::new(name, object_id, inode_type);
            directory.add_entry(entry)?;
        }

        Ok(directory)
    }
}

/// Path component for filesystem navigation
#[derive(Debug, Clone)]
pub enum PathComponent {
    Root,
    Current,
    Parent,
    Name(String),
}

/// Parsed filesystem path
#[derive(Debug, Clone)]
pub struct Path {
    pub is_absolute: bool,
    pub components: Vec<PathComponent>,
}

impl Path {
    pub fn parse(path: &str) -> Result<Self> {
        if path.is_empty() {
            return Err(GalleonError::InvalidPath("Empty path".into()));
        }

        let is_absolute = path.starts_with('/');
        let mut components = Vec::new();

        if is_absolute {
            components.push(PathComponent::Root);
        }

        for component in path.split('/').filter(|c| !c.is_empty()) {
            match component {
                "." => components.push(PathComponent::Current),
                ".." => components.push(PathComponent::Parent),
                name => {
                    if name.len() > 255 {
                        return Err(GalleonError::NameTooLong);
                    }
                    if name.contains('\0') {
                        return Err(GalleonError::InvalidPath("Null byte in path".into()));
                    }
                    components.push(PathComponent::Name(String::from(name)));
                }
            }
        }

        Ok(Self {
            is_absolute,
            components,
        })
    }

    pub fn normalize(&mut self) {
        let mut normalized = Vec::new();

        for component in self.components.drain(..) {
            match component {
                PathComponent::Current => {
                    // Skip "." components
                }
                PathComponent::Parent => {
                    // Remove last component if possible
                    if let Some(last) = normalized.last() {
                        match last {
                            PathComponent::Root => {
                                // Can't go above root
                            }
                            PathComponent::Parent => {
                                // Add another ".."
                                normalized.push(component);
                            }
                            _ => {
                                // Remove the last component
                                normalized.pop();
                            }
                        }
                    } else if !self.is_absolute {
                        // Add ".." for relative paths
                        normalized.push(component);
                    }
                }
                other => {
                    normalized.push(other);
                }
            }
        }

        self.components = normalized;
    }

    pub fn parent(&self) -> Option<Path> {
        if self.components.len() <= 1 {
            return None;
        }

        let mut parent_components = self.components.clone();
        parent_components.pop();

        Some(Path {
            is_absolute: self.is_absolute,
            components: parent_components,
        })
    }

    pub fn file_name(&self) -> Option<&str> {
        if let Some(PathComponent::Name(name)) = self.components.last() {
            Some(name)
        } else {
            None
        }
    }

    pub fn join(&self, other: &Path) -> Path {
        if other.is_absolute {
            return other.clone();
        }

        let mut joined_components = self.components.clone();
        joined_components.extend(other.components.clone());

        let mut result = Path {
            is_absolute: self.is_absolute,
            components: joined_components,
        };
        result.normalize();
        result
    }

    pub fn to_string(&self) -> String {
        if self.components.is_empty() {
            return if self.is_absolute { "/".to_string() } else { ".".to_string() };
        }

        let mut result = String::new();

        for (i, component) in self.components.iter().enumerate() {
            match component {
                PathComponent::Root => {
                    if i == 0 {
                        result.push('/');
                    }
                }
                PathComponent::Current => {
                    if i > 0 {
                        result.push('/');
                    }
                    result.push('.');
                }
                PathComponent::Parent => {
                    if i > 0 {
                        result.push('/');
                    }
                    result.push_str("..");
                }
                PathComponent::Name(name) => {
                    if i > 0 && !result.ends_with('/') {
                        result.push('/');
                    }
                    result.push_str(name);
                }
            }
        }

        result
    }
}

/// Directory operations trait
pub trait DirectoryOperations {
    /// Create a new directory
    fn mkdir(&self, 
            parent_id: ObjectId, 
            name: &str, 
            permissions: Permissions,
            context: &OperationContext,
            transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<ObjectId>> + Send + '_>>;

    /// Remove a directory (must be empty)
    fn rmdir(&self, 
            parent_id: ObjectId, 
            name: &str,
            context: &OperationContext,
            transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// List directory contents
    fn readdir(&self, 
              dir_id: ObjectId,
              context: &OperationContext) -> Pin<Box<dyn Future<Output = Result<Vec<DirectoryEntry>>> + Send + '_>>;

    /// Look up an entry in a directory
    fn lookup(&self, 
             dir_id: ObjectId, 
             name: &str,
             context: &OperationContext) -> Pin<Box<dyn Future<Output = Result<ObjectId>> + Send + '_>>;

    /// Create a hard link
    fn link(&self, 
           target_id: ObjectId,
           parent_id: ObjectId,
           name: &str,
           context: &OperationContext,
           transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Remove a directory entry
    fn unlink(&self, 
             parent_id: ObjectId, 
             name: &str,
             context: &OperationContext,
             transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Rename/move a directory entry
    fn rename(&self, 
             old_parent_id: ObjectId,
             old_name: &str,
             new_parent_id: ObjectId,
             new_name: &str,
             context: &OperationContext,
             transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Create a symbolic link
    fn symlink(&self, 
              target: &str,
              parent_id: ObjectId,
              name: &str,
              context: &OperationContext,
              transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<ObjectId>> + Send + '_>>;

    /// Read a symbolic link
    fn readlink(&self, 
               symlink_id: ObjectId,
               context: &OperationContext) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>>;

    /// Resolve a path to an object ID
    fn resolve_path(&self, 
                   start_id: ObjectId,
                   path: &Path,
                   context: &OperationContext,
                   follow_symlinks: bool) -> Pin<Box<dyn Future<Output = Result<ObjectId>> + Send + '_>>;
}

/// File operations trait
pub trait FileOperations {
    /// Create a new file
    fn create_file(&self, 
                  parent_id: ObjectId, 
                  name: &str, 
                  permissions: Permissions,
                  context: &OperationContext,
                  transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<ObjectId>> + Send + '_>>;

    /// Open a file for reading/writing
    fn open_file(&self, 
                file_id: ObjectId,
                flags: u32,
                context: &OperationContext) -> Pin<Box<dyn Future<Output = Result<FileHandle>> + Send + '_>>;

    /// Read data from a file
    fn read_file(&self, 
                handle: &FileHandle,
                offset: u64,
                length: u64,
                context: &OperationContext) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>>;

    /// Write data to a file
    fn write_file(&self, 
                 handle: &FileHandle,
                 offset: u64,
                 data: &[u8],
                 context: &OperationContext,
                 transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + '_>>;

    /// Truncate a file
    fn truncate_file(&self, 
                    handle: &FileHandle,
                    size: u64,
                    context: &OperationContext,
                    transaction: &Transaction) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Sync file data to storage
    fn sync_file(&self, 
                handle: &FileHandle,
                context: &OperationContext) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Close a file handle
    fn close_file(&self, 
                 handle: FileHandle,
                 context: &OperationContext) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

/// File handle for open files
#[derive(Debug, Clone)]
pub struct FileHandle {
    pub object_id: ObjectId,
    pub flags: u32,
    pub position: u64,
    pub handle_id: u64,
}

impl FileHandle {
    pub fn new(object_id: ObjectId, flags: u32) -> Self {
        use core::sync::atomic::{AtomicU64, Ordering};
        static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);
        
        Self {
            object_id,
            flags,
            position: 0,
            handle_id: NEXT_HANDLE_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub fn seek(&mut self, position: u64) {
        self.position = position;
    }

    pub fn advance(&mut self, bytes: u64) {
        self.position += bytes;
    }
}

/// File open flags
pub mod file_flags {
    pub const O_RDONLY: u32 = 0x0000;
    pub const O_WRONLY: u32 = 0x0001;
    pub const O_RDWR: u32 = 0x0002;
    pub const O_CREAT: u32 = 0x0040;
    pub const O_EXCL: u32 = 0x0080;
    pub const O_TRUNC: u32 = 0x0200;
    pub const O_APPEND: u32 = 0x0400;
    pub const O_SYNC: u32 = 0x1000;
    pub const O_DIRECT: u32 = 0x4000;
    pub const O_TMPFILE: u32 = 0x410000;
}

/// File type checking utilities
impl DirectoryEntry {
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

    pub fn is_special(&self) -> bool {
        matches!(self.inode_type, InodeType::Fifo | InodeType::Socket)
    }
}