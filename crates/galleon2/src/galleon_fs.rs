//! Complete Advanced Filesystem Implementation
//!
//! Combines all components (MFT, journaling, B+trees, allocation) into a cohesive filesystem.

use alloc::{vec, string::String, vec::Vec};
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

        let legacy_super_block = SuperBlock::new(
            cluster_size,
            total_clusters,
            MFT_RECORD_ROOT,
        );

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
        let mut sector = self.legacy_super_block.as_sector();

        // Add extended fields at the end of the sector
        let mut offset = 256; // Start after legacy fields

        sector[offset..offset+8].copy_from_slice(&self.mft_start_cluster.to_le_bytes());
        offset += 8;
        sector[offset..offset+8].copy_from_slice(&self.mft_mirror_cluster.to_le_bytes());
        offset += 8;
        sector[offset..offset+8].copy_from_slice(&self.journal_start_cluster.to_le_bytes());
        offset += 8;
        sector[offset..offset+8].copy_from_slice(&self.journal_size_clusters.to_le_bytes());
        offset += 8;
        sector[offset..offset+8].copy_from_slice(&self.bitmap_start_cluster.to_le_bytes());
        offset += 8;
        sector[offset..offset+8].copy_from_slice(&self.bitmap_size_clusters.to_le_bytes());
        offset += 8;
        sector[offset..offset+4].copy_from_slice(&self.cluster_size.to_le_bytes());
        offset += 4;
        sector[offset..offset+8].copy_from_slice(&self.index_allocation_start.to_le_bytes());

        sector
    }

    pub fn deserialize(sector: &[u8; 512]) -> FilesystemResult<Self> {
        let legacy_super_block = SuperBlock::from_sector(sector);

        let mut offset = 256;
        let mft_start_cluster = u64::from_le_bytes(sector[offset..offset+8].try_into().unwrap());
        offset += 8;
        let mft_mirror_cluster = u64::from_le_bytes(sector[offset..offset+8].try_into().unwrap());
        offset += 8;
        let journal_start_cluster = u64::from_le_bytes(sector[offset..offset+8].try_into().unwrap());
        offset += 8;
        let journal_size_clusters = u64::from_le_bytes(sector[offset..offset+8].try_into().unwrap());
        offset += 8;
        let bitmap_start_cluster = u64::from_le_bytes(sector[offset..offset+8].try_into().unwrap());
        offset += 8;
        let bitmap_size_clusters = u64::from_le_bytes(sector[offset..offset+8].try_into().unwrap());
        offset += 8;
        let cluster_size = u32::from_le_bytes(sector[offset..offset+4].try_into().unwrap());
        offset += 4;
        let index_allocation_start = u64::from_le_bytes(sector[offset..offset+8].try_into().unwrap());

        Ok(Self {
            legacy_super_block,
            mft_start_cluster,
            mft_mirror_cluster,
            journal_start_cluster,
            journal_size_clusters,
            bitmap_start_cluster,
            bitmap_size_clusters,
            cluster_size,
            index_allocation_start,
        })
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
        let disk_size_bytes = return_drive_size_bytes(drive)?;
        let cluster_size = 4096u32;
        let total_clusters = disk_size_bytes / cluster_size as u64;

        if total_clusters < 100 {
            return Err(FilesystemError::InsufficientSpace);
        }

        // Create super block
        let super_block = GalleonSuperBlock::new(total_clusters, cluster_size);

        // Write super block
        let sector = super_block.serialize();
        ide_write_sectors(drive, 1, 0, &sector)?;

        // Initialize bitmap
        let bitmap = ClusterBitmap::new(
            drive,
            super_block.bitmap_start_cluster * (cluster_size / 512) as u64,
            total_clusters,
        );
        let mut allocator = ClusterAllocator::new(bitmap, AllocationStrategy::FirstFit);

        // Mark system areas as used
        let system_clusters = super_block.index_allocation_start + 100; // Reserve some for indexes
        for cluster in 0..system_clusters {
            allocator.bitmap.set_cluster_used(cluster)?;
        }
        allocator.bitmap.flush_bitmap()?;

        // Initialize managers
        let mft_manager = MftManager::new(
            drive,
            super_block.mft_start_cluster,
            cluster_size,
        );

        let journal_manager = JournalManager::new(
            drive,
            super_block.journal_start_cluster * (cluster_size / 512) as u64,
            super_block.journal_size_clusters * (cluster_size / 512) as u64,
        );

        let file_manager = FileRecordManager::new(mft_manager.clone(), journal_manager.clone());

        let btree_manager = BTreeManager::new(
            drive,
            0, // Root directory index
            super_block.index_allocation_start,
            cluster_size,
        );

        // Create system files in MFT
        Self::create_system_files(&mft_manager, &super_block)?;

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
        // Read super block
        let mut sector = [0u8; 512];
        ide_read_sectors(drive, 1, 0, &mut sector)?;

        if !SuperBlock::is_valid(&sector) {
            return Err(FilesystemError::InvalidBootBlock);
        }

        let super_block = GalleonSuperBlock::deserialize(&sector)?;

        // Initialize bitmap
        let bitmap = ClusterBitmap::new(
            drive,
            super_block.bitmap_start_cluster * (super_block.cluster_size / 512) as u64,
            super_block.legacy_super_block.total_blocks,
        );
        let allocator = ClusterAllocator::new(bitmap, AllocationStrategy::FirstFit);

        // Initialize managers
        let mft_manager = MftManager::new(
            drive,
            super_block.mft_start_cluster,
            super_block.cluster_size,
        );

        let mut journal_manager = JournalManager::new(
            drive,
            super_block.journal_start_cluster * (super_block.cluster_size / 512) as u64,
            super_block.journal_size_clusters * (super_block.cluster_size / 512) as u64,
        );

        // Perform recovery if needed
        journal_manager.recover()?;

        let file_manager = FileRecordManager::new(mft_manager.clone(), journal_manager.clone());

        let btree_manager = BTreeManager::new(
            drive,
            0, // Root directory index
            super_block.index_allocation_start,
            super_block.cluster_size,
        );

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
        // Create basic system file records in MFT
        // In a full implementation, these would have proper attributes

        // MFT record for MFT itself (record 0)
        let mft_record = crate::mft::MftRecord::new(0);
        mft_manager.write_record(&mft_record)?;

        // Root directory record (record 5)
        let root_record = crate::mft::MftRecord::new(MFT_RECORD_ROOT);
        mft_manager.write_record(&root_record)?;

        Ok(())
    }

    /// Create a new file
    pub fn create_file(&mut self, parent_dir: FileRecordNumber, name: String, data: Option<Vec<u8>>) -> FilesystemResult<FileRecordNumber> {
        let file_record = self.file_manager.create_file(parent_dir, name.clone(), data)?;

        // Add to directory index
        let file_name = FileName::new(parent_dir, name, false);
        self.btree_manager.insert(file_name.name.clone(), file_record, file_name)?;

        Ok(file_record)
    }

    /// Create a new directory
    pub fn create_directory(&mut self, parent_dir: FileRecordNumber, name: String) -> FilesystemResult<FileRecordNumber> {
        let dir_record = self.file_manager.create_directory(parent_dir, name.clone())?;

        // Add to parent directory index
        let file_name = FileName::new(parent_dir, name, true);
        self.btree_manager.insert(file_name.name.clone(), dir_record, file_name)?;

        Ok(dir_record)
    }

    /// Read file data
    pub fn read_file(&mut self, file_record: FileRecordNumber) -> FilesystemResult<Vec<u8>> {
        self.file_manager.read_file(file_record)
    }

    /// Write file data
    pub fn write_file(&mut self, file_record: FileRecordNumber, data: Vec<u8>) -> FilesystemResult<()> {
        self.file_manager.write_file(file_record, data)
    }

    /// Delete a file
    pub fn delete_file(&mut self, file_record: FileRecordNumber, name: &str) -> FilesystemResult<()> {
        // Remove from directory index
        self.btree_manager.delete(name)?;

        // Deallocate clusters
        self.allocator.deallocate_all_clusters(file_record)?;

        // Delete file record
        self.file_manager.delete_file(file_record)?;

        Ok(())
    }

    /// List directory contents
    pub fn list_directory(&self) -> FilesystemResult<Vec<(String, FileRecordNumber)>> {
        self.btree_manager.list_directory()
    }

    /// Find file by name
    pub fn find_file(&self, name: &str) -> FilesystemResult<Option<FileRecordNumber>> {
        self.btree_manager.search(name)
    }

    /// Get filesystem statistics
    pub fn get_stats(&mut self) -> FilesystemResult<FilesystemStats> {
        let free_space = self.allocator.get_free_space()?;
        let total_space = self.super_block.legacy_super_block.total_blocks * self.super_block.cluster_size as u64;

        Ok(FilesystemStats {
            total_space,
            free_space,
            used_space: total_space - free_space,
            cluster_size: self.super_block.cluster_size,
            total_clusters: self.super_block.legacy_super_block.total_blocks,
        })
    }

    /// Defragment the filesystem
    pub fn defragment(&mut self) -> FilesystemResult<()> {
        // Get all file records and defragment them
        // This is a simplified implementation
        for file_record in 10..1000 { // Skip system files
            self.allocator.defragment_file(file_record)?;
        }
        Ok(())
    }

    /// Force sync all data to disk
    pub fn sync(&mut self) -> FilesystemResult<()> {
        self.allocator.bitmap.flush_bitmap()?;
        self.journal_manager.cleanup_completed_transactions();
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