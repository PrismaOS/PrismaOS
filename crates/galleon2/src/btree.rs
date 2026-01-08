//! B+ Tree Directory Index System
//!
//! Implements B+ tree indexing for fast directory lookups.
//! Uses index nodes for internal navigation and leaf nodes for actual directory entries.

use alloc::{vec, vec::Vec, string::{String, ToString}};
use crate::{
    FilesystemResult, FilesystemError,
    mft::FileRecordNumber,
    file_record::FileName,
    ide_read_sectors, ide_write_sectors,
};
use lib_kernel::kprintln;

pub const INDEX_NODE_SIZE: usize = 4096; // 4KB index nodes
pub const INDEX_ENTRY_HEADER_SIZE: usize = 16;

/// Index entry flags
#[derive(Debug, Clone, Copy)]
pub struct IndexEntryFlags {
    pub has_sub_node: bool,
    pub is_last_entry: bool,
}

impl IndexEntryFlags {
    pub fn new() -> Self {
        Self {
            has_sub_node: false,
            is_last_entry: false,
        }
    }

    pub fn to_u8(&self) -> u8 {
        let mut flags = 0u8;
        if self.has_sub_node { flags |= 0x01; }
        if self.is_last_entry { flags |= 0x02; }
        flags
    }

    pub fn from_u8(value: u8) -> Self {
        Self {
            has_sub_node: (value & 0x01) != 0,
            is_last_entry: (value & 0x02) != 0,
        }
    }
}

/// Directory index entry
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub file_record_number: FileRecordNumber,
    pub entry_length: u16,
    pub key_length: u16,
    pub flags: IndexEntryFlags,
    pub file_name: Option<FileName>, // Present in leaf nodes
    pub sub_node_vcn: Option<u64>,   // Present in internal nodes
    pub key: String, // File name for sorting
}

impl IndexEntry {
    pub fn new_leaf(file_record_number: FileRecordNumber, file_name: FileName) -> Self {
        let key = file_name.name.clone();
        let key_length = key.len() as u16;
        let entry_length = INDEX_ENTRY_HEADER_SIZE as u16 + key_length + file_name.serialize().len() as u16;

        Self {
            file_record_number,
            entry_length,
            key_length,
            flags: IndexEntryFlags::new(),
            file_name: Some(file_name),
            sub_node_vcn: None,
            key,
        }
    }

    pub fn new_internal(key: String, sub_node_vcn: u64) -> Self {
        let key_length = key.len() as u16;
        let entry_length = INDEX_ENTRY_HEADER_SIZE as u16 + key_length + 8; // +8 for VCN

        let mut flags = IndexEntryFlags::new();
        flags.has_sub_node = true;

        Self {
            file_record_number: 0, // Not used in internal nodes
            entry_length,
            key_length,
            flags,
            file_name: None,
            sub_node_vcn: Some(sub_node_vcn),
            key,
        }
    }

    pub fn new_end_marker() -> Self {
        let mut flags = IndexEntryFlags::new();
        flags.is_last_entry = true;

        Self {
            file_record_number: 0,
            entry_length: INDEX_ENTRY_HEADER_SIZE as u16,
            key_length: 0,
            flags,
            file_name: None,
            sub_node_vcn: None,
            key: String::new(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Entry header
        data.extend_from_slice(&self.file_record_number.to_le_bytes());
        data.extend_from_slice(&self.entry_length.to_le_bytes());
        data.extend_from_slice(&self.key_length.to_le_bytes());
        data.push(self.flags.to_u8());
        data.push(0); // Padding

        // Key (file name)
        data.extend_from_slice(self.key.as_bytes());

        // File name attribute (for leaf nodes)
        if let Some(ref file_name) = self.file_name {
            data.extend_from_slice(&file_name.serialize());
        }

        // Sub-node VCN (for internal nodes)
        if let Some(vcn) = self.sub_node_vcn {
            data.extend_from_slice(&vcn.to_le_bytes());
        }

        // Pad to 8-byte boundary
        while data.len() % 8 != 0 {
            data.push(0);
        }

        data
    }

    pub fn deserialize(data: &[u8]) -> FilesystemResult<(Self, usize)> {
        if data.len() < INDEX_ENTRY_HEADER_SIZE {
            return Err(FilesystemError::InvalidParameter);
        }

        let file_record_number = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let entry_length = u16::from_le_bytes(data[8..10].try_into().unwrap());
        let key_length = u16::from_le_bytes(data[10..12].try_into().unwrap());
        let flags = IndexEntryFlags::from_u8(data[12]);

        if data.len() < entry_length as usize {
            return Err(FilesystemError::InvalidParameter);
        }

        let key = String::from_utf8_lossy(&data[INDEX_ENTRY_HEADER_SIZE..INDEX_ENTRY_HEADER_SIZE + key_length as usize]).to_string();

        let mut offset = INDEX_ENTRY_HEADER_SIZE + key_length as usize;

        let (file_name, sub_node_vcn) = if flags.has_sub_node {
            // Internal node - read VCN
            if offset + 8 <= entry_length as usize {
                let vcn = u64::from_le_bytes(data[offset..offset+8].try_into().unwrap());
                (None, Some(vcn))
            } else {
                (None, None)
            }
        } else if !flags.is_last_entry {
            // Leaf node - read file name attribute
            if offset < entry_length as usize {
                match FileName::deserialize(&data[offset..entry_length as usize]) {
                    Ok(fname) => (Some(fname), None),
                    Err(_) => (None, None),
                }
            } else {
                (None, None)
            }
        } else {
            // End marker
            (None, None)
        };

        let entry = Self {
            file_record_number,
            entry_length,
            key_length,
            flags,
            file_name,
            sub_node_vcn,
            key,
        };

        Ok((entry, entry_length as usize))
    }
}

/// Index node header
#[derive(Debug, Clone)]
pub struct IndexHeader {
    pub signature: [u8; 4], // "INDX"
    pub entries_offset: u32,
    pub index_length: u32,
    pub allocated_size: u32,
    pub flags: u8, // 0=leaf, 1=internal
    pub sequence_number: u16,
}

impl IndexHeader {
    pub fn new_leaf() -> Self {
        Self {
            signature: *b"INDX",
            entries_offset: 32, // Header size
            index_length: 32,
            allocated_size: INDEX_NODE_SIZE as u32,
            flags: 0, // Leaf
            sequence_number: 1,
        }
    }

    pub fn new_internal() -> Self {
        Self {
            signature: *b"INDX",
            entries_offset: 32,
            index_length: 32,
            allocated_size: INDEX_NODE_SIZE as u32,
            flags: 1, // Internal
            sequence_number: 1,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(&self.signature);
        data[4..8].copy_from_slice(&self.entries_offset.to_le_bytes());
        data[8..12].copy_from_slice(&self.index_length.to_le_bytes());
        data[12..16].copy_from_slice(&self.allocated_size.to_le_bytes());
        data[16] = self.flags;
        data[17..19].copy_from_slice(&self.sequence_number.to_le_bytes());
        data
    }

    pub fn deserialize(data: &[u8]) -> FilesystemResult<Self> {
        if data.len() < 32 {
            return Err(FilesystemError::InvalidParameter);
        }

        let mut signature = [0u8; 4];
        signature.copy_from_slice(&data[0..4]);

        if &signature != b"INDX" {
            return Err(FilesystemError::InvalidParameter);
        }

        Ok(Self {
            signature,
            entries_offset: u32::from_le_bytes(data[4..8].try_into().unwrap()),
            index_length: u32::from_le_bytes(data[8..12].try_into().unwrap()),
            allocated_size: u32::from_le_bytes(data[12..16].try_into().unwrap()),
            flags: data[16],
            sequence_number: u16::from_le_bytes(data[17..19].try_into().unwrap()),
        })
    }

    pub fn is_leaf(&self) -> bool {
        self.flags == 0
    }
}

/// B+ Tree index node
#[derive(Debug, Clone)]
pub struct IndexNode {
    pub header: IndexHeader,
    pub entries: Vec<IndexEntry>,
    pub vcn: u64, // Virtual Cluster Number
}

impl IndexNode {
    pub fn new_leaf(vcn: u64) -> Self {
        Self {
            header: IndexHeader::new_leaf(),
            entries: vec![IndexEntry::new_end_marker()], // Always has end marker
            vcn,
        }
    }

    pub fn new_internal(vcn: u64) -> Self {
        Self {
            header: IndexHeader::new_internal(),
            entries: vec![IndexEntry::new_end_marker()],
            vcn,
        }
    }

    pub fn insert_entry(&mut self, entry: IndexEntry) {
        // Find insertion point (entries are sorted by key)
        let insert_pos = self.entries.iter()
            .position(|e| e.flags.is_last_entry || e.key > entry.key)
            .unwrap_or(self.entries.len());

        // Insert before the end marker or at the found position
        if insert_pos < self.entries.len() && self.entries[insert_pos].flags.is_last_entry {
            self.entries.insert(insert_pos, entry);
        } else {
            self.entries.insert(insert_pos, entry);
        }

        self.update_header();
    }

    pub fn remove_entry(&mut self, key: &str) -> Option<IndexEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.key == key) {
            let removed = self.entries.remove(pos);
            self.update_header();
            Some(removed)
        } else {
            None
        }
    }

    pub fn find_entry(&self, key: &str) -> Option<&IndexEntry> {
        self.entries.iter().find(|e| e.key == key && !e.flags.is_last_entry)
    }

    pub fn is_full(&self) -> bool {
        let size = self.calculate_size();
        size > INDEX_NODE_SIZE - 512 // Leave some margin
    }

    pub fn is_underflow(&self) -> bool {
        let size = self.calculate_size();
        size < INDEX_NODE_SIZE / 4 // Less than 25% full
    }

    fn calculate_size(&self) -> usize {
        let mut size = self.header.entries_offset as usize;
        for entry in &self.entries {
            size += entry.entry_length as usize;
        }
        size
    }

    pub fn update_header(&mut self) {
        self.header.index_length = self.calculate_size() as u32;
    }

    pub fn split(&mut self) -> IndexNode {
        let mid_point = self.entries.len() / 2;

        // Create new node with second half of entries
        let mut new_node = if self.header.is_leaf() {
            IndexNode::new_leaf(self.vcn + 1)
        } else {
            IndexNode::new_internal(self.vcn + 1)
        };

        // Move entries from mid_point onwards to new node (except end marker)
        let end_marker_pos = self.entries.len() - 1;
        if mid_point < end_marker_pos {
            let moved_entries: Vec<_> = self.entries.drain(mid_point..end_marker_pos).collect();
            for entry in moved_entries {
                new_node.entries.insert(new_node.entries.len() - 1, entry);
            }
        }

        self.update_header();
        new_node.update_header();

        new_node
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = vec![0u8; INDEX_NODE_SIZE];

        // Serialize header
        let header_data = self.header.serialize();
        data[..header_data.len()].copy_from_slice(&header_data);

        // Serialize entries
        let mut offset = self.header.entries_offset as usize;
        for entry in &self.entries {
            let entry_data = entry.serialize();
            if offset + entry_data.len() <= INDEX_NODE_SIZE {
                data[offset..offset + entry_data.len()].copy_from_slice(&entry_data);
                offset += entry_data.len();
            }
        }

        data
    }

    pub fn deserialize(data: &[u8], vcn: u64) -> FilesystemResult<Self> {
        if data.len() < INDEX_NODE_SIZE {
            return Err(FilesystemError::InvalidParameter);
        }

        let header = IndexHeader::deserialize(data)?;
        let mut entries = Vec::new();

        let mut offset = header.entries_offset as usize;
        while offset < header.index_length as usize {
            let (entry, entry_size) = IndexEntry::deserialize(&data[offset..])?;
            entries.push(entry.clone());
            offset += entry_size;

            if entry.flags.is_last_entry {
                break;
            }
        }

        Ok(Self {
            header,
            entries,
            vcn,
        })
    }
}

/// B+ Tree manager for directory indexing
pub struct BTreeManager {
    drive: u8,
    root_vcn: u64,
    index_allocation_start: u64, // Starting cluster for index allocation
    cluster_size: u32,
}

impl BTreeManager {
    pub fn new(drive: u8, root_vcn: u64, index_allocation_start: u64, cluster_size: u32) -> Self {
        Self {
            drive,
            root_vcn,
            index_allocation_start,
            cluster_size,
        }
    }

    pub fn read_node(&self, vcn: u64) -> FilesystemResult<IndexNode> {
        let cluster_num = self.index_allocation_start + vcn;
        let sector_start = cluster_num * (self.cluster_size / 512) as u64;
        let sectors_per_node = INDEX_NODE_SIZE / 512;  // INDEX_NODE_SIZE is always a multiple of 512

        kprintln!("Reading B-tree node: vcn={}, cluster_num={}, sector_start={}, sectors={}", 
                  vcn, cluster_num, sector_start, sectors_per_node);

        let mut node_data = vec![0u8; INDEX_NODE_SIZE];
        ide_read_sectors(self.drive, sectors_per_node as u8, sector_start as u32, &mut node_data)?;

        kprintln!("First 16 bytes of node data: {:02x?}", &node_data[0..16]);
        
        IndexNode::deserialize(&node_data, vcn)
    }

    pub fn write_node(&self, node: &IndexNode) -> FilesystemResult<()> {
        let cluster_num = self.index_allocation_start + node.vcn;
        let sector_start = cluster_num * (self.cluster_size / 512) as u64;
        let sectors_per_node = INDEX_NODE_SIZE / 512;  // INDEX_NODE_SIZE is always a multiple of 512

        kprintln!("Writing B-tree node: vcn={}, cluster_num={}, sector_start={}, sectors={}", 
                  node.vcn, cluster_num, sector_start, sectors_per_node);

        let node_data = node.serialize();
        kprintln!("First 16 bytes being written: {:02x?}", &node_data[0..16]);
        
        ide_write_sectors(self.drive, sectors_per_node as u8, sector_start as u32, &node_data)?;
        kprintln!("B-tree node written successfully");
        Ok(())
    }

    pub fn search(&self, key: &str) -> FilesystemResult<Option<FileRecordNumber>> {
        let mut current_vcn = self.root_vcn;

        loop {
            let node = self.read_node(current_vcn)?;

            if node.header.is_leaf() {
                // Leaf node - search for exact match
                if let Some(entry) = node.find_entry(key) {
                    return Ok(Some(entry.file_record_number));
                } else {
                    return Ok(None);
                }
            } else {
                // Internal node - find child to descend to
                let mut found_child = None;
                for entry in &node.entries {
                    if entry.flags.is_last_entry {
                        // Last entry points to rightmost child
                        if let Some(vcn) = entry.sub_node_vcn {
                            found_child = Some(vcn);
                        }
                        break;
                    } else if key <= &entry.key {
                        if let Some(vcn) = entry.sub_node_vcn {
                            found_child = Some(vcn);
                            break;
                        }
                    }
                }

                if let Some(vcn) = found_child {
                    current_vcn = vcn;
                } else {
                    return Ok(None);
                }
            }
        }
    }

    pub fn insert(&mut self, key: String, file_record_number: FileRecordNumber, file_name: FileName) -> FilesystemResult<()> {
        let root = self.read_node(self.root_vcn)?;

        if root.is_full() {
            // Split root and create new root
            self.split_root()?;
        }

        self.insert_recursive(self.root_vcn, key, file_record_number, file_name)
    }

    fn insert_recursive(&self, vcn: u64, key: String, file_record_number: FileRecordNumber, file_name: FileName) -> FilesystemResult<()> {
        let mut node = self.read_node(vcn)?;

        if node.header.is_leaf() {
            // Insert into leaf node
            let entry = IndexEntry::new_leaf(file_record_number, file_name);
            node.insert_entry(entry);
            self.write_node(&node)?;
        } else {
            // Find child to insert into
            let mut child_vcn = None;
            for entry in &node.entries {
                if entry.flags.is_last_entry {
                    if let Some(vcn) = entry.sub_node_vcn {
                        child_vcn = Some(vcn);
                    }
                    break;
                } else if key <= entry.key {
                    if let Some(vcn) = entry.sub_node_vcn {
                        child_vcn = Some(vcn);
                        break;
                    }
                }
            }

            if let Some(child_vcn) = child_vcn {
                self.insert_recursive(child_vcn, key, file_record_number, file_name)?;

                // Check if child split and handle promotion
                let child = self.read_node(child_vcn)?;
                if child.is_full() {
                    // Child needs to be split
                    // This would involve more complex split handling
                }
            }
        }

        Ok(())
    }

    fn split_root(&mut self) -> FilesystemResult<()> {
        let old_root = self.read_node(self.root_vcn)?;

        // Create new root
        let new_root_vcn = self.allocate_vcn()?;
        let mut new_root = IndexNode::new_internal(new_root_vcn);

        // Split old root
        let mut old_root_copy = old_root.clone();
        let new_node = old_root_copy.split();
        let new_node_vcn = self.allocate_vcn()?;

        // Add entries to new root pointing to split nodes
        let left_entry = IndexEntry::new_internal(
            old_root_copy.entries[0].key.clone(),
            old_root_copy.vcn,
        );
        let right_entry = IndexEntry::new_internal(
            new_node.entries[0].key.clone(),
            new_node_vcn,
        );

        new_root.insert_entry(left_entry);
        new_root.insert_entry(right_entry);

        // Write all nodes
        self.write_node(&old_root_copy)?;
        self.write_node(&new_node)?;
        self.write_node(&new_root)?;

        // Update root VCN
        self.root_vcn = new_root_vcn;

        Ok(())
    }

    fn allocate_vcn(&self) -> FilesystemResult<u64> {
        // Simplified VCN allocation
        static mut NEXT_VCN: u64 = 1;
        unsafe {
            let vcn = NEXT_VCN;
            NEXT_VCN += 1;
            Ok(vcn)
        }
    }

    pub fn list_directory(&self) -> FilesystemResult<Vec<(String, FileRecordNumber)>> {
        let mut results = Vec::new();
        self.list_recursive(self.root_vcn, &mut results)?;
        Ok(results)
    }

    fn list_recursive(&self, vcn: u64, results: &mut Vec<(String, FileRecordNumber)>) -> FilesystemResult<()> {
        let node = self.read_node(vcn)?;

        if node.header.is_leaf() {
            // Collect all entries from leaf
            for entry in &node.entries {
                if !entry.flags.is_last_entry {
                    results.push((entry.key.clone(), entry.file_record_number));
                }
            }
        } else {
            // Traverse all children
            for entry in &node.entries {
                if let Some(child_vcn) = entry.sub_node_vcn {
                    self.list_recursive(child_vcn, results)?;
                }
            }
        }

        Ok(())
    }

    pub fn delete(&mut self, key: &str) -> FilesystemResult<bool> {
        self.delete_recursive(self.root_vcn, key)
    }

    fn delete_recursive(&self, vcn: u64, key: &str) -> FilesystemResult<bool> {
        let mut node = self.read_node(vcn)?;

        if node.header.is_leaf() {
            // Remove from leaf if found
            if node.remove_entry(key).is_some() {
                self.write_node(&node)?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            // Find child containing the key
            for entry in &node.entries {
                if entry.flags.is_last_entry {
                    if let Some(child_vcn) = entry.sub_node_vcn {
                        return self.delete_recursive(child_vcn, key);
                    }
                } else if key <= &entry.key {
                    if let Some(child_vcn) = entry.sub_node_vcn {
                        return self.delete_recursive(child_vcn, key);
                    }
                }
            }
            Ok(false)
        }
    }
}
