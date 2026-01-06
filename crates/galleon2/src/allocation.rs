//! Extent-based Cluster Allocation System
//!
//! Implements extent allocation with run lists for efficient space management.
//! Uses cluster bitmaps and allocation strategies for optimal performance.

use alloc::{vec, vec::Vec, collections::BTreeMap};
use lib_kernel::kprintln;
use crate::{FilesystemResult, FilesystemError, ide_read_sectors, ide_write_sectors};

pub const CLUSTER_SIZE: u32 = 4096; // 4KB clusters
pub const SECTORS_PER_CLUSTER: u32 = CLUSTER_SIZE / 512;

/// Cluster run representing a contiguous range of clusters
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterRun {
    pub start_cluster: u64,
    pub cluster_count: u64,
}

impl ClusterRun {
    pub fn new(start_cluster: u64, cluster_count: u64) -> Self {
        Self {
            start_cluster,
            cluster_count,
        }
    }

    pub fn end_cluster(&self) -> u64 {
        self.start_cluster + self.cluster_count - 1
    }

    pub fn contains_cluster(&self, cluster: u64) -> bool {
        cluster >= self.start_cluster && cluster <= self.end_cluster()
    }

    pub fn overlaps(&self, other: &ClusterRun) -> bool {
        !(self.end_cluster() < other.start_cluster || other.end_cluster() < self.start_cluster)
    }

    pub fn can_merge(&self, other: &ClusterRun) -> bool {
        self.end_cluster() + 1 == other.start_cluster || other.end_cluster() + 1 == self.start_cluster
    }

    pub fn merge(&self, other: &ClusterRun) -> Option<ClusterRun> {
        if self.can_merge(other) {
            let start = self.start_cluster.min(other.start_cluster);
            let end = self.end_cluster().max(other.end_cluster());
            Some(ClusterRun::new(start, end - start + 1))
        } else {
            None
        }
    }
}

/// Run list for non-resident file data
#[derive(Debug, Clone)]
pub struct RunList {
    pub runs: Vec<ClusterRun>,
    pub total_clusters: u64,
}

impl RunList {
    pub fn new() -> Self {
        Self {
            runs: Vec::new(),
            total_clusters: 0,
        }
    }

    pub fn add_run(&mut self, run: ClusterRun) {
        self.runs.push(run);
        self.total_clusters += run.cluster_count;
        self.sort_and_merge();
    }

    pub fn remove_run(&mut self, run: ClusterRun) -> bool {
        if let Some(pos) = self.runs.iter().position(|r| *r == run) {
            self.runs.remove(pos);
            self.total_clusters -= run.cluster_count;
            true
        } else {
            false
        }
    }

    fn sort_and_merge(&mut self) {
        // Sort by start cluster
        self.runs.sort_by_key(|r| r.start_cluster);

        // Merge adjacent runs
        let mut merged: Vec<ClusterRun> = Vec::new();
        for run in &self.runs {
            if let Some(last) = merged.last_mut() {
                if let Some(merged_run) = last.merge(run) {
                    *last = merged_run;
                } else {
                    merged.push(*run);
                }
            } else {
                merged.push(*run);
            }
        }

        self.runs = merged;
    }

    pub fn find_cluster(&self, virtual_cluster: u64) -> Option<u64> {
        let mut vcn = 0;
        for run in &self.runs {
            if virtual_cluster >= vcn && virtual_cluster < vcn + run.cluster_count {
                let offset = virtual_cluster - vcn;
                return Some(run.start_cluster + offset);
            }
            vcn += run.cluster_count;
        }
        None
    }

    pub fn get_total_size(&self) -> u64 {
        self.total_clusters * CLUSTER_SIZE as u64
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&(self.runs.len() as u32).to_le_bytes());

        for run in &self.runs {
            data.extend_from_slice(&run.start_cluster.to_le_bytes());
            data.extend_from_slice(&run.cluster_count.to_le_bytes());
        }

        data
    }

    pub fn deserialize(data: &[u8]) -> FilesystemResult<Self> {
        if data.len() < 4 {
            return Err(FilesystemError::InvalidParameter);
        }

        let run_count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        if data.len() < 4 + run_count * 16 {
            return Err(FilesystemError::InvalidParameter);
        }

        let mut runs = Vec::new();
        let mut total_clusters = 0;

        for i in 0..run_count {
            let offset = 4 + i * 16;
            let start_cluster = u64::from_le_bytes(data[offset..offset+8].try_into().unwrap());
            let cluster_count = u64::from_le_bytes(data[offset+8..offset+16].try_into().unwrap());

            let run = ClusterRun::new(start_cluster, cluster_count);
            runs.push(run);
            total_clusters += cluster_count;
        }

        Ok(Self {
            runs,
            total_clusters,
        })
    }
}

/// Cluster allocation strategy
#[derive(Debug, Clone, Copy)]
pub enum AllocationStrategy {
    FirstFit,    // Find first available space
    BestFit,     // Find smallest space that fits
    NextFit,     // Continue from last allocation
}

/// Free space manager using cluster bitmap
pub struct ClusterBitmap {
    drive: u8,
    bitmap_start_sector: u64,
    bitmap_size_sectors: u64,
    total_clusters: u64,
    clusters_per_sector: u64,
    cached_bitmap: Option<Vec<u8>>,
    dirty: bool,
}

impl ClusterBitmap {
    pub fn new(drive: u8, bitmap_start_sector: u64, total_clusters: u64) -> Self {
        let bits_needed = total_clusters;
        let bytes_needed = (bits_needed + 7) / 8;
        let bitmap_size_sectors = (bytes_needed + 511) / 512;
        let clusters_per_sector = 512 * 8; // 512 bytes * 8 bits per byte

        Self {
            drive,
            bitmap_start_sector,
            bitmap_size_sectors,
            total_clusters,
            clusters_per_sector,
            cached_bitmap: None,
            dirty: false,
        }
    }

    pub fn load_bitmap(&mut self) -> FilesystemResult<()> {
        kprintln!("Loading cluster bitmap from drive {}", self.drive);
        if self.cached_bitmap.is_none() {
            // Allocate bitmap in smaller chunks to avoid early allocator failures
            // Read 2 sectors (1KB) at a time to keep allocations very small
            const CHUNK_SIZE_SECTORS: u64 = 2;
            let mut bitmap_data = Vec::new(); // Don't pre-allocate to avoid large allocation
            
            kprintln!("Reading {} sectors starting at sector {} for bitmap", 
                self.bitmap_size_sectors, self.bitmap_start_sector);
            
            let mut sectors_read = 0u64;
            while sectors_read < self.bitmap_size_sectors {
                let sectors_to_read = CHUNK_SIZE_SECTORS.min(self.bitmap_size_sectors - sectors_read);
                let mut chunk = vec![0u8; (sectors_to_read * 512) as usize];
                
                ide_read_sectors(
                    self.drive,
                    sectors_to_read as u8,
                    (self.bitmap_start_sector + sectors_read) as u32,
                    &mut chunk,
                )?;
                
                bitmap_data.extend_from_slice(&chunk);
                sectors_read += sectors_to_read;
            }
            
            self.cached_bitmap = Some(bitmap_data);
            self.dirty = false;
        }
        kprintln!("Cluster bitmap loaded successfully");
        Ok(())
    }

    pub fn flush_bitmap(&mut self) -> FilesystemResult<()> {
        if self.dirty {
            if let Some(ref bitmap) = self.cached_bitmap {
                ide_write_sectors(
                    self.drive,
                    self.bitmap_size_sectors as u8,
                    self.bitmap_start_sector as u32,
                    bitmap,
                )?;
                self.dirty = false;
            }
        }
        Ok(())
    }

    pub fn is_cluster_free(&mut self, cluster: u64) -> FilesystemResult<bool> {
        if cluster >= self.total_clusters {
            return Ok(false);
        }

        self.load_bitmap()?;
        if let Some(ref bitmap) = self.cached_bitmap {
            let byte_index = (cluster / 8) as usize;
            let bit_index = (cluster % 8) as u8;

            if byte_index < bitmap.len() {
                Ok((bitmap[byte_index] & (1 << bit_index)) == 0)
            } else {
                Ok(false)
            }
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }

    pub fn set_cluster_used(&mut self, cluster: u64) -> FilesystemResult<()> {
        if cluster >= self.total_clusters {
            return Err(FilesystemError::InvalidParameter);
        }

        self.load_bitmap()?;
        if let Some(ref mut bitmap) = self.cached_bitmap {
            let byte_index = (cluster / 8) as usize;
            let bit_index = (cluster % 8) as u8;

            if byte_index < bitmap.len() {
                bitmap[byte_index] |= 1 << bit_index;
                self.dirty = true;
                Ok(())
            } else {
                Err(FilesystemError::InvalidParameter)
            }
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }

    pub fn set_cluster_free(&mut self, cluster: u64) -> FilesystemResult<()> {
        if cluster >= self.total_clusters {
            return Err(FilesystemError::InvalidParameter);
        }

        self.load_bitmap()?;
        if let Some(ref mut bitmap) = self.cached_bitmap {
            let byte_index = (cluster / 8) as usize;
            let bit_index = (cluster % 8) as u8;

            if byte_index < bitmap.len() {
                bitmap[byte_index] &= !(1 << bit_index);
                self.dirty = true;
                Ok(())
            } else {
                Err(FilesystemError::InvalidParameter)
            }
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }

    pub fn find_free_clusters(&mut self, count: u64, strategy: AllocationStrategy) -> FilesystemResult<Option<ClusterRun>> {
        self.load_bitmap()?;

        match strategy {
            AllocationStrategy::FirstFit => self.find_first_fit(count),
            AllocationStrategy::BestFit => self.find_best_fit(count),
            AllocationStrategy::NextFit => self.find_next_fit(count),
        }
    }

    fn find_first_fit(&self, count: u64) -> FilesystemResult<Option<ClusterRun>> {
        if let Some(ref bitmap) = self.cached_bitmap {
            let mut current_start = None;
            let mut current_count = 0;

            for cluster in 0..self.total_clusters {
                let byte_index = (cluster / 8) as usize;
                let bit_index = (cluster % 8) as u8;

                if byte_index >= bitmap.len() {
                    break;
                }

                let is_free = (bitmap[byte_index] & (1 << bit_index)) == 0;

                if is_free {
                    if current_start.is_none() {
                        current_start = Some(cluster);
                        current_count = 1;
                    } else {
                        current_count += 1;
                    }

                    if current_count >= count {
                        return Ok(Some(ClusterRun::new(current_start.unwrap(), count)));
                    }
                } else {
                    current_start = None;
                    current_count = 0;
                }
            }

            Ok(None)
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }

    fn find_best_fit(&self, count: u64) -> FilesystemResult<Option<ClusterRun>> {
        if let Some(ref bitmap) = self.cached_bitmap {
            let mut best_run: Option<ClusterRun> = None;
            let mut current_start = None;
            let mut current_count = 0;

            for cluster in 0..self.total_clusters {
                let byte_index = (cluster / 8) as usize;
                let bit_index = (cluster % 8) as u8;

                if byte_index >= bitmap.len() {
                    break;
                }

                let is_free = (bitmap[byte_index] & (1 << bit_index)) == 0;

                if is_free {
                    if current_start.is_none() {
                        current_start = Some(cluster);
                        current_count = 1;
                    } else {
                        current_count += 1;
                    }
                } else {
                    if let Some(start) = current_start {
                        if current_count >= count {
                            let run = ClusterRun::new(start, current_count);
                            if best_run.is_none() || current_count < best_run.unwrap().cluster_count {
                                best_run = Some(run);
                            }
                        }
                    }
                    current_start = None;
                    current_count = 0;
                }
            }

            // Check final run
            if let Some(start) = current_start {
                if current_count >= count {
                    let run = ClusterRun::new(start, current_count);
                    if best_run.is_none() || current_count < best_run.unwrap().cluster_count {
                        best_run = Some(run);
                    }
                }
            }

            if let Some(run) = best_run {
                Ok(Some(ClusterRun::new(run.start_cluster, count)))
            } else {
                Ok(None)
            }
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }

    fn find_next_fit(&self, count: u64) -> FilesystemResult<Option<ClusterRun>> {
        // For simplicity, implement as first fit
        // In a real implementation, this would remember the last allocation point
        self.find_first_fit(count)
    }

    pub fn mark_run_used(&mut self, run: &ClusterRun) -> FilesystemResult<()> {
        for cluster in run.start_cluster..run.start_cluster + run.cluster_count {
            self.set_cluster_used(cluster)?;
        }
        Ok(())
    }

    pub fn mark_run_free(&mut self, run: &ClusterRun) -> FilesystemResult<()> {
        for cluster in run.start_cluster..run.start_cluster + run.cluster_count {
            self.set_cluster_free(cluster)?;
        }
        Ok(())
    }

    pub fn get_free_cluster_count(&mut self) -> FilesystemResult<u64> {
        self.load_bitmap()?;
        if let Some(ref bitmap) = self.cached_bitmap {
            let mut free_count = 0;
            for cluster in 0..self.total_clusters {
                let byte_index = (cluster / 8) as usize;
                let bit_index = (cluster % 8) as u8;

                if byte_index < bitmap.len() && (bitmap[byte_index] & (1 << bit_index)) == 0 {
                    free_count += 1;
                }
            }
            Ok(free_count)
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }
}

/// Cluster allocation manager
pub struct ClusterAllocator {
    pub bitmap: ClusterBitmap,
    strategy: AllocationStrategy,
    allocated_runs: BTreeMap<u64, RunList>, // file_record_number -> run_list
}

impl ClusterAllocator {
    pub fn new(bitmap: ClusterBitmap, strategy: AllocationStrategy) -> Self {
        Self {
            bitmap,
            strategy,
            allocated_runs: BTreeMap::new(),
        }
    }

    pub fn allocate_clusters(&mut self, file_record_number: u64, cluster_count: u64) -> FilesystemResult<ClusterRun> {
        if let Some(run) = self.bitmap.find_free_clusters(cluster_count, self.strategy)? {
            self.bitmap.mark_run_used(&run)?;

            // Track allocation
            let run_list = self.allocated_runs.entry(file_record_number).or_insert_with(RunList::new);
            run_list.add_run(run);

            self.bitmap.flush_bitmap()?;
            Ok(run)
        } else {
            Err(FilesystemError::InsufficientSpace)
        }
    }

    pub fn deallocate_clusters(&mut self, file_record_number: u64, run: ClusterRun) -> FilesystemResult<()> {
        self.bitmap.mark_run_free(&run)?;

        // Remove from tracking
        if let Some(run_list) = self.allocated_runs.get_mut(&file_record_number) {
            run_list.remove_run(run);
            if run_list.runs.is_empty() {
                self.allocated_runs.remove(&file_record_number);
            }
        }

        self.bitmap.flush_bitmap()?;
        Ok(())
    }

    pub fn deallocate_all_clusters(&mut self, file_record_number: u64) -> FilesystemResult<()> {
        if let Some(run_list) = self.allocated_runs.remove(&file_record_number) {
            for run in &run_list.runs {
                self.bitmap.mark_run_free(run)?;
            }
            self.bitmap.flush_bitmap()?;
        }
        Ok(())
    }

    pub fn get_file_runs(&self, file_record_number: u64) -> Option<&RunList> {
        self.allocated_runs.get(&file_record_number)
    }

    pub fn extend_allocation(&mut self, file_record_number: u64, additional_clusters: u64) -> FilesystemResult<ClusterRun> {
        // Try to extend existing allocation first
        if let Some(run_list) = self.allocated_runs.get(&file_record_number) {
            if let Some(last_run) = run_list.runs.last() {
                // Try to extend the last run
                let next_cluster = last_run.end_cluster() + 1;
                let mut can_extend = true;

                for cluster in next_cluster..next_cluster + additional_clusters {
                    if !self.bitmap.is_cluster_free(cluster)? {
                        can_extend = false;
                        break;
                    }
                }

                if can_extend {
                    let extended_run = ClusterRun::new(next_cluster, additional_clusters);
                    self.bitmap.mark_run_used(&extended_run)?;

                    // Merge with last run
                    if let Some(run_list) = self.allocated_runs.get_mut(&file_record_number) {
                        run_list.add_run(extended_run);
                    }

                    self.bitmap.flush_bitmap()?;
                    return Ok(extended_run);
                }
            }
        }

        // If can't extend, allocate new run
        self.allocate_clusters(file_record_number, additional_clusters)
    }

    pub fn get_total_allocated(&self, file_record_number: u64) -> u64 {
        if let Some(run_list) = self.allocated_runs.get(&file_record_number) {
            run_list.total_clusters
        } else {
            0
        }
    }

    pub fn get_free_space(&mut self) -> FilesystemResult<u64> {
        let free_clusters = self.bitmap.get_free_cluster_count()?;
        Ok(free_clusters * CLUSTER_SIZE as u64)
    }

    pub fn defragment_file(&mut self, file_record_number: u64) -> FilesystemResult<()> {
        // Get current runs
        if let Some(current_runs) = self.allocated_runs.get(&file_record_number).cloned() {
            let total_clusters = current_runs.total_clusters;

            // Try to allocate contiguous space
            if let Some(new_run) = self.bitmap.find_free_clusters(total_clusters, AllocationStrategy::FirstFit)? {
                // Mark new space as used
                self.bitmap.mark_run_used(&new_run)?;

                // Free old space
                for old_run in &current_runs.runs {
                    self.bitmap.mark_run_free(old_run)?;
                }

                // Update tracking
                let mut new_run_list = RunList::new();
                new_run_list.add_run(new_run);
                self.allocated_runs.insert(file_record_number, new_run_list);

                self.bitmap.flush_bitmap()?;
            }
        }

        Ok(())
    }

    pub fn read_cluster(&self, cluster: u64, buffer: &mut [u8]) -> FilesystemResult<()> {
        if buffer.len() < CLUSTER_SIZE as usize {
            return Err(FilesystemError::InvalidParameter);
        }

        let sector_start = cluster * SECTORS_PER_CLUSTER as u64;
        ide_read_sectors(self.bitmap.drive, SECTORS_PER_CLUSTER as u8, sector_start as u32, buffer)?;
        Ok(())
    }

    pub fn write_cluster(&self, cluster: u64, buffer: &[u8]) -> FilesystemResult<()> {
        if buffer.len() < CLUSTER_SIZE as usize {
            return Err(FilesystemError::InvalidParameter);
        }

        let sector_start = cluster * SECTORS_PER_CLUSTER as u64;
        ide_write_sectors(self.bitmap.drive, SECTORS_PER_CLUSTER as u8, sector_start as u32, buffer)?;
        Ok(())
    }
}