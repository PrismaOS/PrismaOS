//! Complete Advanced Filesystem Implementation
//!
//! Combines all components (MFT, journaling, B+trees, allocation) into a cohesive filesystem.

use alloc::{vec, string::String, vec::Vec};
use lib_kernel::{kprint, kprintln};
use crate::{
    FilesystemResult, FilesystemError,
    super_block::SuperBlock,
    mft::{MftManager, FileRecordNumber, MFT_RECORD_ROOT},
    journal::JournalManager,
    file_record::{FileRecordManager, FileName},
    btree::BTreeManager,
    allocation::{ClusterAllocator, ClusterBitmap, AllocationStrategy},
    ide_write_sectors, ide_read_sectors, return_drive_size_bytes,
};

/// Enhanced SuperBlock with advanced layout
#[derive(Debug, Clone)]
pub struct GalleonSuperBlock {
    pub legacy_super_block: SuperBlock,
    pub mft_start_cluster: u64,
    pub mft_mirror_cluster: u64,
    pub journal_start_cluster: u64,
    pub journal_size_clusters: u64,
    pub bitmap_start_cluster: u64,
    pub bitmap_size_clusters: u64,
    pub cluster_size: u32,
    pub index_allocation_start: u64,
}

impl GalleonSuperBlock {
    pub fn new(total_clusters: u64, cluster_size: u32) -> Self {
        kprintln!("Creating new GalleonSuperBlock...");
        kprintln!("Total clusters: {}, Cluster size: {}", total_clusters, cluster_size);
        
        // Advanced filesystem layout:
        // Clusters 0-15: Boot sector and reserved
        // Clusters 16-?: MFT (starts at cluster 16)
        // Clusters ?-?: MFT Mirror (backup)
        // Clusters ?-?: Journal/Log file
        // Clusters ?-?: Cluster bitmap
        // Clusters ?-?: Index allocation
        // Clusters ?-?: User data

        let mft_size_clusters = (total_clusters / 8).max(16); // 12.5% for MFT, minimum 16 clusters
        let mft_start_cluster = 16;
        let mft_mirror_cluster = mft_start_cluster + mft_size_clusters;
        let journal_start_cluster = mft_mirror_cluster + mft_size_clusters / 4; // 25% of MFT size for mirror
        let journal_size_clusters = (total_clusters / 32).max(8); // 3.125% for journal, minimum 8 clusters
        let bitmap_start_cluster = journal_start_cluster + journal_size_clusters;
        let bitmap_size_clusters = ((total_clusters + 8 * cluster_size as u64 - 1) / (8 * cluster_size as u64)).max(1);
        let index_allocation_start = bitmap_start_cluster + bitmap_size_clusters;

        kprintln!("Layout - MFT: {}, Mirror: {}, Journal: {}+{}, Bitmap: {}+{}, Index: {}", 
                  mft_start_cluster, mft_mirror_cluster, journal_start_cluster, journal_size_clusters,
                  bitmap_start_cluster, bitmap_size_clusters, index_allocation_start);

        let legacy_super_block = SuperBlock::new(
            cluster_size,
            total_clusters,
            MFT_RECORD_ROOT,
        );

        kprintln!("GalleonSuperBlock created successfully");
        Self {
            legacy_super_block,
            mft_start_cluster,
            mft_mirror_cluster,
            journal_start_cluster,
            journal_size_clusters,
            bitmap_start_cluster,
            bitmap_size_clusters,
            cluster_size,
            index_allocation_start,
        }
    }

    pub fn serialize(&self) -> [u8; 512] {
        kprintln!("Serializing GalleonSuperBlock...");
        let mut sector = self.legacy_super_block.as_sector();

        // Add extended fields at the end of the sector
        let mut offset = 256; // Start after legacy fields
        kprintln!("Writing extended fields starting at offset {}", offset);

        sector[offset..offset+8].copy_from_slice(&self.mft_start_cluster.to_le_bytes());
        kprintln!("Wrote MFT start cluster: {} at offset {}", self.mft_start_cluster, offset);
        offset += 8;
        
        sector[offset..offset+8].copy_from_slice(&self.mft_mirror_cluster.to_le_bytes());
        kprintln!("Wrote MFT mirror cluster: {} at offset {}", self.mft_mirror_cluster, offset);
        offset += 8;
        
        sector[offset..offset+8].copy_from_slice(&self.journal_start_cluster.to_le_bytes());
        kprintln!("Wrote journal start cluster: {} at offset {}", self.journal_start_cluster, offset);
        offset += 8;
        
        sector[offset..offset+8].copy_from_slice(&self.journal_size_clusters.to_le_bytes());
        kprintln!("Wrote journal size clusters: {} at offset {}", self.journal_size_clusters, offset);
        offset += 8;
        
        sector[offset..offset+8].copy_from_slice(&self.bitmap_start_cluster.to_le_bytes());
        kprintln!("Wrote bitmap start cluster: {} at offset {}", self.bitmap_start_cluster, offset);
        offset += 8;
        
        sector[offset..offset+8].copy_from_slice(&self.bitmap_size_clusters.to_le_bytes());
        kprintln!("Wrote bitmap size clusters: {} at offset {}", self.bitmap_size_clusters, offset);
        offset += 8;
        
        sector[offset..offset+4].copy_from_slice(&self.cluster_size.to_le_bytes());
        kprintln!("Wrote cluster size: {} at offset {}", self.cluster_size, offset);
        offset += 4;
        
        sector[offset..offset+8].copy_from_slice(&self.index_allocation_start.to_le_bytes());
        kprintln!("Wrote index allocation start: {} at offset {}", self.index_allocation_start, offset);

        kprintln!("GalleonSuperBlock serialization completed");
        sector
    }

    pub fn deserialize(sector: &[u8; 512]) -> FilesystemResult<Self> {
        kprintln!("Starting GalleonSuperBlock deserialization...");
        
        kprintln!("Deserializing legacy super block...");
        let legacy_super_block = SuperBlock::from_sector(sector);
        kprintln!("Legacy super block deserialized");

        kprintln!("Reading extended fields starting at offset 256...");
        kprintln!("Sector length: {}, checking bounds...", sector.len());
        
        // Check if we have enough data
        if sector.len() < 256 + 60 { // Need at least 60 bytes for all fields
            kprintln!("ERROR: Sector too small for extended fields");
            return Err(FilesystemError::InvalidBootBlock);
        }
        
        let mut offset = 256;
        
        // Read with bounds checking and error handling
        kprintln!("Reading MFT start cluster at offset {}...", offset);
        if offset + 8 > sector.len() {
            kprintln!("ERROR: MFT start cluster read would exceed bounds");
            return Err(FilesystemError::InvalidBootBlock);
        }
        let mft_start_slice = &sector[offset..offset+8];
        kprintln!("MFT start slice: {:02x?}", mft_start_slice);
        let mft_start_cluster = match mft_start_slice.try_into() {
            Ok(bytes) => u64::from_le_bytes(bytes),
            Err(_) => {
                kprintln!("ERROR: Failed to convert MFT start cluster bytes");
                return Err(FilesystemError::InvalidBootBlock);
            }
        };
        kprintln!("MFT start cluster: {}", mft_start_cluster);
        offset += 8;
        
        kprintln!("Reading MFT mirror cluster at offset {}...", offset);
        if offset + 8 > sector.len() {
            kprintln!("ERROR: MFT mirror cluster read would exceed bounds");
            return Err(FilesystemError::InvalidBootBlock);
        }
        let mft_mirror_slice = &sector[offset..offset+8];
        kprintln!("MFT mirror slice: {:02x?}", mft_mirror_slice);
        let mft_mirror_cluster = match mft_mirror_slice.try_into() {
            Ok(bytes) => u64::from_le_bytes(bytes),
            Err(_) => {
                kprintln!("ERROR: Failed to convert MFT mirror cluster bytes");
                return Err(FilesystemError::InvalidBootBlock);
            }
        };
        kprintln!("MFT mirror cluster: {}", mft_mirror_cluster);
        offset += 8;
        
        kprintln!("Reading journal start cluster at offset {}...", offset);
        if offset + 8 > sector.len() {
            kprintln!("ERROR: Journal start cluster read would exceed bounds");
            return Err(FilesystemError::InvalidBootBlock);
        }
        let journal_start_slice = &sector[offset..offset+8];
        kprintln!("Journal start slice: {:02x?}", journal_start_slice);
        let journal_start_cluster = match journal_start_slice.try_into() {
            Ok(bytes) => u64::from_le_bytes(bytes),
            Err(_) => {
                kprintln!("ERROR: Failed to convert journal start cluster bytes");
                return Err(FilesystemError::InvalidBootBlock);
            }
        };
        kprintln!("Journal start cluster: {}", journal_start_cluster);
        offset += 8;
        
        kprintln!("Reading journal size clusters at offset {}...", offset);
        if offset + 8 > sector.len() {
            kprintln!("ERROR: Journal size clusters read would exceed bounds");
            return Err(FilesystemError::InvalidBootBlock);
        }
        let journal_size_slice = &sector[offset..offset+8];
        kprintln!("Journal size slice: {:02x?}", journal_size_slice);
        let journal_size_clusters = match journal_size_slice.try_into() {
            Ok(bytes) => u64::from_le_bytes(bytes),
            Err(_) => {
                kprintln!("ERROR: Failed to convert journal size clusters bytes");
                return Err(FilesystemError::InvalidBootBlock);
            }
        };
        kprintln!("Journal size clusters: {}", journal_size_clusters);
        offset += 8;
        
        kprintln!("Reading bitmap start cluster at offset {}...", offset);
        if offset + 8 > sector.len() {
            kprintln!("ERROR: Bitmap start cluster read would exceed bounds");
            return Err(FilesystemError::InvalidBootBlock);
        }
        let bitmap_start_slice = &sector[offset..offset+8];
        kprintln!("Bitmap start slice: {:02x?}", bitmap_start_slice);
        let bitmap_start_cluster = match bitmap_start_slice.try_into() {
            Ok(bytes) => u64::from_le_bytes(bytes),
            Err(_) => {
                kprintln!("ERROR: Failed to convert bitmap start cluster bytes");
                return Err(FilesystemError::InvalidBootBlock);
            }
        };
        kprintln!("Bitmap start cluster: {}", bitmap_start_cluster);
        offset += 8;
        
        kprintln!("Reading bitmap size clusters at offset {}...", offset);
        if offset + 8 > sector.len() {
            kprintln!("ERROR: Bitmap size clusters read would exceed bounds");
            return Err(FilesystemError::InvalidBootBlock);
        }
        let bitmap_size_slice = &sector[offset..offset+8];
        kprintln!("Bitmap size slice: {:02x?}", bitmap_size_slice);
        let bitmap_size_clusters = match bitmap_size_slice.try_into() {
            Ok(bytes) => u64::from_le_bytes(bytes),
            Err(_) => {
                kprintln!("ERROR: Failed to convert bitmap size clusters bytes");
                return Err(FilesystemError::InvalidBootBlock);
            }
        };
        kprintln!("Bitmap size clusters: {}", bitmap_size_clusters);
        offset += 8;
        
        kprintln!("Reading cluster size at offset {}...", offset);
        if offset + 4 > sector.len() {
            kprintln!("ERROR: Cluster size read would exceed bounds");
            return Err(FilesystemError::InvalidBootBlock);
        }
        let cluster_size_slice = &sector[offset..offset+4];
        kprintln!("Cluster size slice: {:02x?}", cluster_size_slice);
        let cluster_size = match cluster_size_slice.try_into() {
            Ok(bytes) => u32::from_le_bytes(bytes),
            Err(_) => {
                kprintln!("ERROR: Failed to convert cluster size bytes");
                return Err(FilesystemError::InvalidBootBlock);
            }
        };
        kprintln!("Cluster size: {} bytes", cluster_size);
        offset += 4;
        
        kprintln!("Reading index allocation start at offset {}...", offset);
        if offset + 8 > sector.len() {
            kprintln!("ERROR: Index allocation start read would exceed bounds");
            return Err(FilesystemError::InvalidBootBlock);
        }
        let index_alloc_slice = &sector[offset..offset+8];
        kprintln!("Index allocation slice: {:02x?}", index_alloc_slice);
        let index_allocation_start = match index_alloc_slice.try_into() {
            Ok(bytes) => u64::from_le_bytes(bytes),
            Err(_) => {
                kprintln!("ERROR: Failed to convert index allocation start bytes");
                return Err(FilesystemError::InvalidBootBlock);
            }
        };
        kprintln!("Index allocation start: {}", index_allocation_start);

        kprintln!("All fields read successfully, validating values...");
        
        // Validate the values make sense
        if cluster_size == 0 || cluster_size > 65536 {
            kprintln!("ERROR: Invalid cluster size: {}", cluster_size);
            return Err(FilesystemError::InvalidBootBlock);
        }
        
        if mft_start_cluster == 0 {
            kprintln!("ERROR: Invalid MFT start cluster: {}", mft_start_cluster);
            return Err(FilesystemError::InvalidBootBlock);
        }

        kprintln!("Values validated, constructing GalleonSuperBlock...");
        kprintln!("Summary - MFT: {}, Journal: {}+{}, Bitmap: {}+{}, Cluster: {} bytes, Index: {}", 
                  mft_start_cluster, journal_start_cluster, journal_size_clusters,
                  bitmap_start_cluster, bitmap_size_clusters, cluster_size, index_allocation_start);

        let result = Self {
            legacy_super_block,
            mft_start_cluster,
            mft_mirror_cluster,
            journal_start_cluster,
            journal_size_clusters,
            bitmap_start_cluster,
            bitmap_size_clusters,
            cluster_size,
            index_allocation_start,
        };
        
        kprintln!("GalleonSuperBlock construction completed successfully!");
        Ok(result)
    }
}

/// Complete Galleon Filesystem
pub struct GalleonFilesystem {
    drive: u8,
    super_block: GalleonSuperBlock,
    mft_manager: MftManager,
    journal_manager: JournalManager,
    file_manager: FileRecordManager,
    btree_manager: BTreeManager,
    allocator: ClusterAllocator,
}

impl GalleonFilesystem {
    /// Create a new filesystem on the specified drive
    pub fn format(drive: u8) -> FilesystemResult<Self> {
        kprintln!("Format was called!");
        kprintln!("Getting drive size for drive {}...", drive);
        let disk_size_bytes = return_drive_size_bytes(drive)?;
        kprintln!("Drive size: {} bytes", disk_size_bytes);
        
        let cluster_size = 4096u32;
        let total_clusters = disk_size_bytes / cluster_size as u64;
        kprintln!("Calculated {} clusters of {} bytes each", total_clusters, cluster_size);

        if total_clusters < 100 {
            kprintln!("ERROR: Insufficient space - only {} clusters", total_clusters);
            return Err(FilesystemError::InsufficientSpace);
        }

        // Create super block
        kprintln!("Creating super block...");
        let super_block = GalleonSuperBlock::new(total_clusters, cluster_size);

        // Write super block
        kprintln!("Writing super block to sector 1...");
        let sector = super_block.serialize();
        ide_write_sectors(drive, 1, 0, &sector)?;
        kprintln!("Super block written successfully");

        // Initialize bitmap
        kprintln!("Initializing bitmap...");
        let bitmap_start_sector = super_block.bitmap_start_cluster * (cluster_size / 512) as u64;
        kprintln!("Bitmap starts at sector {}", bitmap_start_sector);
        let bitmap = ClusterBitmap::new(
            drive,
            bitmap_start_sector,
            total_clusters,
        );
        let mut allocator = ClusterAllocator::new(bitmap, AllocationStrategy::FirstFit);
        kprintln!("Bitmap and allocator initialized");

        // Mark system areas as used
        let system_clusters = super_block.index_allocation_start + 100; // Reserve some for indexes
        kprintln!("Marking {} system clusters as used", system_clusters);
        for cluster in 0..system_clusters {
            allocator.bitmap.set_cluster_used(cluster)?;
        }
        allocator.bitmap.flush_bitmap()?;
        kprintln!("System clusters marked as used");

        // Initialize managers
        kprintln!("Initializing MFT manager...");
        let mft_manager = MftManager::new(
            drive,
            super_block.mft_start_cluster,
            cluster_size,
        );
        kprintln!("MFT manager initialized");

        kprintln!("Initializing journal manager...");
        let journal_start_sector = super_block.journal_start_cluster * (cluster_size / 512) as u64;
        let journal_size_sectors = super_block.journal_size_clusters * (cluster_size / 512) as u64;
        let journal_manager = JournalManager::new(
            drive,
            journal_start_sector,
            journal_size_sectors,
        );
        kprintln!("Journal manager initialized");

        kprintln!("Initializing file record manager...");
        let file_manager = FileRecordManager::new(mft_manager.clone(), journal_manager.clone());
        kprintln!("File record manager initialized");

        kprintln!("Initializing B-tree manager...");
        let btree_manager = BTreeManager::new(
            drive,
            0, // Root directory index
            super_block.index_allocation_start,
            cluster_size,
        );
        kprintln!("B-tree manager initialized");

        // Create system files in MFT
        kprintln!("Creating system files...");
        Self::create_system_files(&mft_manager, &super_block)?;
        kprintln!("System files created");

        kprintln!("Filesystem format completed successfully!");
        Ok(Self {
            drive,
            super_block,
            mft_manager,
            journal_manager,
            file_manager,
            btree_manager,
            allocator,
        })
    }

    /// Mount an existing filesystem
    pub fn mount(drive: u8) -> FilesystemResult<Self> {
        kprintln!("Mount!");
        kprintln!("Starting filesystem mount for drive {}", drive);

        // Read super block
        kprintln!("Reading super block from sector 1...");
        let mut sector = [0u8; 512];
        ide_read_sectors(drive, 1, 0, &mut sector)?;
        kprintln!("Super block sector read successfully");

        kprintln!("Validating super block...");
        if !SuperBlock::is_valid(&sector) {
            kprintln!("Super block validation failed - invalid boot block");
            return Err(FilesystemError::InvalidBootBlock);
        }
        kprintln!("Super block validation passed");

        kprintln!("Deserializing Galleon super block...");
        let super_block = GalleonSuperBlock::deserialize(&sector)?;
        kprintln!("Super block deserialized successfully");
        kprintln!("Cluster size: {}", super_block.cluster_size);
        kprintln!("Total blocks: {}", super_block.legacy_super_block.total_blocks);

        // Validate cluster size before using it for calculations
        if super_block.cluster_size == 0 {
            kprintln!("ERROR: Cluster size is zero - filesystem corrupted");
            return Err(FilesystemError::InvalidBootBlock);
        }

        // Initialize bitmap
        kprintln!("Initializing cluster bitmap...");
        let bitmap_start = super_block.bitmap_start_cluster * (super_block.cluster_size / 512) as u64;
        kprintln!("Bitmap start sector: {}", bitmap_start);
        let bitmap = ClusterBitmap::new(
            drive,
            bitmap_start,
            super_block.legacy_super_block.total_blocks,
        );
        kprintln!("Cluster bitmap initialized");

        kprintln!("Creating cluster allocator with FirstFit strategy...");
        let allocator = ClusterAllocator::new(bitmap, AllocationStrategy::FirstFit);
        kprintln!("Cluster allocator created");

        // Initialize managers
        kprintln!("Initializing MFT manager...");
        kprintln!("MFT start cluster: {}", super_block.mft_start_cluster);
        let mft_manager = MftManager::new(
            drive,
            super_block.mft_start_cluster,
            super_block.cluster_size,
        );
        kprintln!("MFT manager initialized");

        kprintln!("Initializing journal manager...");
        let journal_start = super_block.journal_start_cluster * (super_block.cluster_size / 512) as u64;
        let journal_size = super_block.journal_size_clusters * (super_block.cluster_size / 512) as u64;
        kprintln!("Journal start sector: {}", journal_start);
        kprintln!("Journal size in sectors: {}", journal_size);
        let mut journal_manager = JournalManager::new(
            drive,
            journal_start,
            journal_size,
        );
        kprintln!("Journal manager initialized");

        // Perform recovery if needed
        kprintln!("Starting journal recovery...");
        journal_manager.recover()?;
        kprintln!("Journal recovery completed");

        kprintln!("Initializing file record manager...");
        let file_manager = FileRecordManager::new(mft_manager.clone(), journal_manager.clone());
        kprintln!("File record manager initialized");

        kprintln!("Initializing B-tree manager...");
        kprintln!("Index allocation start: {}", super_block.index_allocation_start);
        let btree_manager = BTreeManager::new(
            drive,
            0, // Root directory index
            super_block.index_allocation_start,
            super_block.cluster_size,
        );
        kprintln!("B-tree manager initialized");

        kprintln!("Filesystem mount completed successfully!");
        Ok(Self {
            drive,
            super_block,
            mft_manager,
            journal_manager,
            file_manager,
            btree_manager,
            allocator,
        })
    }

    fn create_system_files(mft_manager: &MftManager, _super_block: &GalleonSuperBlock) -> FilesystemResult<()> {
        kprintln!("Creating system files in MFT...");
        // Create basic system file records in MFT
        // In a full implementation, these would have proper attributes

        // MFT record for MFT itself (record 0)
        kprintln!("Creating MFT record 0 (MFT itself)...");
        let mft_record = crate::mft::MftRecord::new(0);
        mft_manager.write_record(&mft_record)?;

        // Root directory record (record 5)
        kprintln!("Creating MFT record {} (root directory)...", MFT_RECORD_ROOT);
        let root_record = crate::mft::MftRecord::new(MFT_RECORD_ROOT);
        mft_manager.write_record(&root_record)?;

        kprintln!("System files created successfully");
        Ok(())
    }

    /// Create a new file
    pub fn create_file(&mut self, parent_dir: FileRecordNumber, name: String, data: Option<Vec<u8>>) -> FilesystemResult<FileRecordNumber> {
        kprintln!("Creating file '{}' in directory {}", name, parent_dir);
        let file_record = self.file_manager.create_file(parent_dir, name.clone(), data)?;
        kprintln!("File record {} created", file_record);

        // Add to directory index
        let file_name = FileName::new(parent_dir, name, false);
        self.btree_manager.insert(file_name.name.clone(), file_record, file_name)?;
        kprintln!("File added to directory index");

        Ok(file_record)
    }

    /// Create a new directory
    pub fn create_directory(&mut self, parent_dir: FileRecordNumber, name: String) -> FilesystemResult<FileRecordNumber> {
        kprintln!("Creating directory '{}' in directory {}", name, parent_dir);
        let dir_record = self.file_manager.create_directory(parent_dir, name.clone())?;
        kprintln!("Directory record {} created", dir_record);

        // Add to parent directory index
        let file_name = FileName::new(parent_dir, name, true);
        self.btree_manager.insert(file_name.name.clone(), dir_record, file_name)?;
        kprintln!("Directory added to parent directory index");

        Ok(dir_record)
    }

    /// Read file data
    pub fn read_file(&mut self, file_record: FileRecordNumber) -> FilesystemResult<Vec<u8>> {
        kprintln!("Reading file record {}", file_record);
        let data = self.file_manager.read_file(file_record)?;
        kprintln!("Read {} bytes from file", data.len());
        Ok(data)
    }

    /// Write file data
    pub fn write_file(&mut self, file_record: FileRecordNumber, data: Vec<u8>) -> FilesystemResult<()> {
        kprintln!("Writing {} bytes to file record {}", data.len(), file_record);
        self.file_manager.write_file(file_record, data)?;
        kprintln!("File write completed");
        Ok(())
    }

    /// Delete a file
    pub fn delete_file(&mut self, file_record: FileRecordNumber, name: &str) -> FilesystemResult<()> {
        kprintln!("Deleting file '{}' (record {})", name, file_record);

        // Remove from directory index
        self.btree_manager.delete(name)?;
        kprintln!("File removed from directory index");

        // Deallocate clusters
        self.allocator.deallocate_all_clusters(file_record)?;
        kprintln!("File clusters deallocated");

        // Delete file record
        self.file_manager.delete_file(file_record)?;
        kprintln!("File record deleted");

        Ok(())
    }

    /// List directory contents
    pub fn list_directory(&self) -> FilesystemResult<Vec<(String, FileRecordNumber)>> {
        kprintln!("Listing directory contents");
        let contents = self.btree_manager.list_directory()?;
        kprintln!("Found {} directory entries", contents.len());
        Ok(contents)
    }

    /// Find file by name
    pub fn find_file(&self, name: &str) -> FilesystemResult<Option<FileRecordNumber>> {
        kprintln!("Searching for file '{}'", name);
        let result = self.btree_manager.search(name)?;
        match result {
            Some(record) => kprintln!("File '{}' found at record {}", name, record),
            None => kprintln!("File '{}' not found", name),
        }
        Ok(result)
    }

    /// Get filesystem statistics
    pub fn get_stats(&mut self) -> FilesystemResult<FilesystemStats> {
        kprintln!("Getting filesystem statistics");
        let free_space = self.allocator.get_free_space()?;
        let total_space = self.super_block.legacy_super_block.total_blocks * self.super_block.cluster_size as u64;
        let used_space = total_space - free_space;

        kprintln!("Stats - Total: {}, Free: {}, Used: {}", total_space, free_space, used_space);
        Ok(FilesystemStats {
            total_space,
            free_space,
            used_space,
            cluster_size: self.super_block.cluster_size,
            total_clusters: self.super_block.legacy_super_block.total_blocks,
        })
    }

    /// Defragment the filesystem
    pub fn defragment(&mut self) -> FilesystemResult<()> {
        kprintln!("Starting filesystem defragmentation");
        // Get all file records and defragment them
        // This is a simplified implementation
        for file_record in 10..1000 { // Skip system files
            if let Err(_) = self.allocator.defragment_file(file_record) {
                // Ignore errors for non-existent files
                continue;
            }
        }
        kprintln!("Filesystem defragmentation completed");
        Ok(())
    }

    /// Force sync all data to disk
    pub fn sync(&mut self) -> FilesystemResult<()> {
        kprintln!("Syncing filesystem to disk");
        self.allocator.bitmap.flush_bitmap()?;
        self.journal_manager.cleanup_completed_transactions();
        kprintln!("Filesystem sync completed");
        Ok(())
    }
}

/// Filesystem statistics
#[derive(Debug, Clone)]
pub struct FilesystemStats {
    pub total_space: u64,
    pub free_space: u64,
    pub used_space: u64,
    pub cluster_size: u32,
    pub total_clusters: u64,
}

impl Clone for MftManager {
    fn clone(&self) -> Self {
        Self {
            drive: self.drive,
            mft_start_cluster: self.mft_start_cluster,
            cluster_size: self.cluster_size,
        }
    }
}

impl Clone for JournalManager {
    fn clone(&self) -> Self {
        Self {
            drive: self.drive,
            journal_start_sector: self.journal_start_sector,
            journal_size_sectors: self.journal_size_sectors,
            current_sequence: self.current_sequence,
            active_transactions: alloc::collections::VecDeque::new(),
            next_transaction_id: self.next_transaction_id,
        }
    }
}