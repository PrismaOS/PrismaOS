//! AHCI Command Processing
//!
//! This module handles the construction and execution of ATA commands
//! through the AHCI interface. It provides high-level command abstractions
//! and manages the low-level FIS (Frame Information Structure) construction.

extern crate alloc;
use alloc::{vec::Vec, sync::Arc};
use crate::memory::dma::{DmaBuffer, BufferId};
use super::{AhciError, AhciResult, consts::*};
/// ATA command representation
///
/// Encapsulates all information needed to execute an ATA command
/// including the command code, parameters, and associated data buffers.
#[derive(Debug, Clone)]
pub struct AtaCommand {
    /// ATA command code
    pub command: u8,
    /// Features register (low 8 bits)
    pub features: u8,
    /// Features register (high 8 bits, for 48-bit commands)
    pub features_ext: u8,
    /// Sector count (low 8 bits)
    pub count: u8,
    /// Sector count (high 8 bits, for 48-bit commands)
    pub count_ext: u8,
    /// LBA address (48-bit)
    pub lba: u64,
    /// Device register
    pub device: u8,
    /// Command type
    pub command_type: CommandType,
    /// Associated DMA buffer for data transfer
    pub buffer: Option<BufferId>,
    /// Transfer direction
    pub direction: TransferDirection,
    /// Expected transfer size in bytes
    pub transfer_size: u32,
    /// Command timeout in milliseconds
    pub timeout_ms: u32,
}

/// Command type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    /// Non-data command (no data transfer)
    NonData,
    /// PIO data command (CPU-driven data transfer)
    PioData,
    /// DMA data command (DMA-driven data transfer)
    DmaData,
    /// ATAPI packet command
    Packet,
}

/// Data transfer direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    /// No data transfer
    None,
    /// Data transfer from device to host
    DeviceToHost,
    /// Data transfer from host to device
    HostToDevice,
}

/// Command execution result
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// Command completed successfully
    Success,
    /// Command failed with error code
    Error(u8),
    /// Command timed out
    Timeout,
    /// Command was aborted
    Aborted,
}

impl CommandResult {
    /// Check if command was successful
    pub fn is_success(&self) -> bool {
        matches!(self, CommandResult::Success)
    }

    /// Get error code if available
    pub fn error_code(&self) -> Option<u8> {
        match self {
            CommandResult::Error(code) => Some(*code),
            _ => None,
        }
    }
}

impl AtaCommand {
    /// Create a new ATA command
    pub fn new(command: u8) -> Self {
        Self {
            command,
            features: 0,
            features_ext: 0,
            count: 0,
            count_ext: 0,
            lba: 0,
            device: 0xE0, // LBA mode, master drive
            command_type: CommandType::NonData,
            buffer: None,
            direction: TransferDirection::None,
            transfer_size: 0,
            timeout_ms: 5000, // 5 second default timeout
        }
    }

    /// Create IDENTIFY DEVICE command
    pub fn identify(buffer: BufferId) -> Self {
        Self {
            command: ATA_CMD_IDENTIFY,
            features: 0,
            features_ext: 0,
            count: 0,
            count_ext: 0,
            lba: 0,
            device: 0xE0,
            command_type: CommandType::PioData,
            buffer: Some(buffer),
            direction: TransferDirection::DeviceToHost,
            transfer_size: 512,
            timeout_ms: 3000,
        }
    }

    /// Create READ DMA command (28-bit LBA)
    pub fn read_dma(lba: u32, sectors: u8, buffer: BufferId) -> Self {
        Self {
            command: ATA_CMD_READ_DMA,
            features: 0,
            features_ext: 0,
            count: sectors,
            count_ext: 0,
            lba: lba as u64,
            device: 0xE0 | ((lba >> 24) & 0x0F) as u8,
            command_type: CommandType::DmaData,
            buffer: Some(buffer),
            direction: TransferDirection::DeviceToHost,
            transfer_size: sectors as u32 * 512,
            timeout_ms: 10000,
        }
    }

    /// Create READ DMA EXT command (48-bit LBA)
    pub fn read_dma_ext(lba: u64, sectors: u32, buffer: BufferId) -> Self {
        let sector_count = if sectors > 65535 { 65535 } else { sectors };
        
        Self {
            command: ATA_CMD_READ_DMA_EXT,
            features: 0,
            features_ext: 0,
            count: (sector_count & 0xFF) as u8,
            count_ext: ((sector_count >> 8) & 0xFF) as u8,
            lba,
            device: 0x40, // LBA48 mode
            command_type: CommandType::DmaData,
            buffer: Some(buffer),
            direction: TransferDirection::DeviceToHost,
            transfer_size: sector_count * 512,
            timeout_ms: 15000,
        }
    }

    /// Create WRITE DMA command (28-bit LBA)
    pub fn write_dma(lba: u32, sectors: u8, buffer: BufferId) -> Self {
        Self {
            command: ATA_CMD_WRITE_DMA,
            features: 0,
            features_ext: 0,
            count: sectors,
            count_ext: 0,
            lba: lba as u64,
            device: 0xE0 | ((lba >> 24) & 0x0F) as u8,
            command_type: CommandType::DmaData,
            buffer: Some(buffer),
            direction: TransferDirection::HostToDevice,
            transfer_size: sectors as u32 * 512,
            timeout_ms: 10000,
        }
    }

    /// Create WRITE DMA EXT command (48-bit LBA)
    pub fn write_dma_ext(lba: u64, sectors: u32, buffer: BufferId) -> Self {
        let sector_count = if sectors > 65535 { 65535 } else { sectors };
        
        Self {
            command: ATA_CMD_WRITE_DMA_EXT,
            features: 0,
            features_ext: 0,
            count: (sector_count & 0xFF) as u8,
            count_ext: ((sector_count >> 8) & 0xFF) as u8,
            lba,
            device: 0x40, // LBA48 mode
            command_type: CommandType::DmaData,
            buffer: Some(buffer),
            direction: TransferDirection::HostToDevice,
            transfer_size: sector_count * 512,
            timeout_ms: 15000,
        }
    }

    /// Create FLUSH CACHE command
    pub fn flush_cache() -> Self {
        Self {
            command: 0xE7, // FLUSH CACHE
            features: 0,
            features_ext: 0,
            count: 0,
            count_ext: 0,
            lba: 0,
            device: 0xE0,
            command_type: CommandType::NonData,
            buffer: None,
            direction: TransferDirection::None,
            transfer_size: 0,
            timeout_ms: 30000, // Flush can take a while
        }
    }

    /// Create FLUSH CACHE EXT command
    pub fn flush_cache_ext() -> Self {
        Self {
            command: 0xEA, // FLUSH CACHE EXT
            features: 0,
            features_ext: 0,
            count: 0,
            count_ext: 0,
            lba: 0,
            device: 0x40,
            command_type: CommandType::NonData,
            buffer: None,
            direction: TransferDirection::None,
            transfer_size: 0,
            timeout_ms: 30000,
        }
    }

    /// Create STANDBY IMMEDIATE command
    pub fn standby_immediate() -> Self {
        Self {
            command: 0xE0, // STANDBY IMMEDIATE
            features: 0,
            features_ext: 0,
            count: 0,
            count_ext: 0,
            lba: 0,
            device: 0xE0,
            command_type: CommandType::NonData,
            buffer: None,
            direction: TransferDirection::None,
            transfer_size: 0,
            timeout_ms: 5000,
        }
    }

    /// Create SET FEATURES command
    pub fn set_features(feature: u8, count: u8) -> Self {
        Self {
            command: 0xEF, // SET FEATURES
            features: feature,
            features_ext: 0,
            count,
            count_ext: 0,
            lba: 0,
            device: 0xE0,
            command_type: CommandType::NonData,
            buffer: None,
            direction: TransferDirection::None,
            transfer_size: 0,
            timeout_ms: 5000,
        }
    }

    /// Get the number of PRDT entries needed for this command
    pub fn get_prdt_count(&self) -> usize {
        if self.transfer_size == 0 {
            return 0;
        }
        
        // Each PRDT entry can handle up to 4MB
        // For simplicity, we'll use one entry per 64KB
        let entries = (self.transfer_size + 65535) / 65536;
        core::cmp::max(1, entries as usize)
    }

    /// Check if this is a 48-bit LBA command
    pub fn is_lba48(&self) -> bool {
        matches!(self.command, 
            ATA_CMD_READ_DMA_EXT |
            ATA_CMD_WRITE_DMA_EXT |
            0xEA // FLUSH CACHE EXT
        )
    }

    /// Build Command FIS (Frame Information Structure)
    pub fn build_fis(&self) -> FisRegH2D {
        let mut fis = FisRegH2D {
            fis_type: FIS_TYPE_REG_H2D,
            pmport_c: 0x80, // Command bit set
            command: self.command,
            featurel: self.features,
            
            lba0: (self.lba & 0xFF) as u8,
            lba1: ((self.lba >> 8) & 0xFF) as u8,
            lba2: ((self.lba >> 16) & 0xFF) as u8,
            device: self.device,
            
            lba3: ((self.lba >> 24) & 0xFF) as u8,
            lba4: ((self.lba >> 32) & 0xFF) as u8,
            lba5: ((self.lba >> 40) & 0xFF) as u8,
            featureh: self.features_ext,
            
            countl: self.count,
            counth: self.count_ext,
            icc: 0,
            control: 0,
            
            rsv: [0; 4],
        };
        
        fis.set_cmd();
        fis
    }

    /// Set command timeout
    pub fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set device register
    pub fn with_device(mut self, device: u8) -> Self {
        self.device = device;
        self
    }

    /// Set features register
    pub fn with_features(mut self, features: u8, features_ext: u8) -> Self {
        self.features = features;
        self.features_ext = features_ext;
        self
    }
}

/// Command queue for managing multiple concurrent commands
///
/// AHCI supports up to 32 concurrent commands per port.
/// This structure manages command slot allocation and tracking.
pub struct CommandQueue {
    /// Active commands by slot
    active_commands: [Option<ActiveCommand>; HBA_CMD_SLOT_MAX],
    /// Available slot mask
    available_slots: u32,
    /// Commands completed but not yet processed
    completed_commands: Vec<(u8, CommandResult)>,
}

/// Active command tracking
#[derive(Clone)]
struct ActiveCommand {
    /// The command being executed
    command: AtaCommand,
    /// Timestamp when command was issued
    start_time: u64,
    /// Command completion callback
    callback: Option<Arc<dyn Fn(CommandResult) + Send + Sync>>,
}

impl CommandQueue {
    /// Create a new command queue
    pub fn new() -> Self {
        Self {
            active_commands: [const { None }; HBA_CMD_SLOT_MAX],
            available_slots: 0xFFFFFFFF, // All slots available
            completed_commands: Vec::new(),
        }
    }

    /// Allocate a command slot
    pub fn allocate_slot(&mut self) -> Option<u8> {
        for slot in 0..HBA_CMD_SLOT_MAX {
            if (self.available_slots & (1 << slot)) != 0 {
                self.available_slots &= !(1 << slot);
                return Some(slot as u8);
            }
        }
        None
    }

    /// Release a command slot
    pub fn release_slot(&mut self, slot: u8) {
        if (slot as usize) < HBA_CMD_SLOT_MAX {
            self.available_slots |= 1 << slot;
            self.active_commands[slot as usize] = None;
        }
    }

    /// Submit a command to a slot
    pub fn submit_command(&mut self, slot: u8, command: AtaCommand) -> AhciResult<()> {
        let slot_idx = slot as usize;
        if slot_idx >= HBA_CMD_SLOT_MAX {
            return Err(AhciError::InvalidParameters);
        }

        if self.active_commands[slot_idx].is_some() {
            return Err(AhciError::Busy);
        }

        let active_cmd = ActiveCommand {
            command,
            start_time: crate::time::get_timestamp(),
            callback: None,
        };

        self.active_commands[slot_idx] = Some(active_cmd);
        Ok(())
    }

    /// Mark command as completed
    pub fn complete_command(&mut self, slot: u8, result: CommandResult) {
        self.completed_commands.push((slot, result));
    }

    /// Process completed commands
    pub fn process_completions(&mut self) -> Vec<(u8, CommandResult)> {
        let completed = self.completed_commands.clone();
        self.completed_commands.clear();
        
        for (slot, _) in &completed {
            self.release_slot(*slot);
        }
        
        completed
    }

    /// Check for timed out commands
    pub fn check_timeouts(&mut self, current_time: u64) -> Vec<u8> {
        let mut timed_out = Vec::new();
        
        for (slot, active_cmd) in self.active_commands.iter().enumerate() {
            if let Some(cmd) = active_cmd {
                let elapsed_ms = current_time.saturating_sub(cmd.start_time);
                if elapsed_ms > cmd.command.timeout_ms as u64 {
                    timed_out.push(slot as u8);
                }
            }
        }
        
        // Mark timed out commands as completed
        for slot in &timed_out {
            self.complete_command(*slot, CommandResult::Timeout);
        }
        
        timed_out
    }

    /// Get active command count
    pub fn active_count(&self) -> usize {
        self.active_commands.iter().filter(|cmd| cmd.is_some()).count()
    }

    /// Get available slot count
    pub fn available_count(&self) -> usize {
        self.available_slots.count_ones() as usize
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.active_count() == 0
    }

    /// Check if queue is full
    pub fn is_full(&self) -> bool {
        self.available_count() == 0
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// PRDT (Physical Region Descriptor Table) builder
///
/// Helps construct scatter-gather lists for DMA transfers.
pub struct PrdtBuilder {
    entries: Vec<HbaPrdtEntry>,
    total_bytes: u32,
}

impl PrdtBuilder {
    /// Create a new PRDT builder
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            total_bytes: 0,
        }
    }

    /// Add a memory region to the PRDT
    pub fn add_region(&mut self, phys_addr: u64, byte_count: u32, interrupt_on_completion: bool) -> AhciResult<()> {
        if byte_count == 0 || byte_count > PRDT_MAX_BYTES {
            return Err(AhciError::InvalidParameters);
        }

        if self.entries.len() >= 65535 {
            return Err(AhciError::InvalidParameters); // Too many entries
        }

        let mut dbc = byte_count - 1; // Hardware expects count - 1
        if interrupt_on_completion {
            dbc |= 0x80000000; // Set interrupt bit
        }

        let entry = HbaPrdtEntry {
            dba: (phys_addr & 0xFFFFFFFF) as u32,
            dbau: (phys_addr >> 32) as u32,
            rsv: 0,
            dbc,
        };

        self.entries.push(entry);
        self.total_bytes += byte_count;
        Ok(())
    }

    /// Add a DMA buffer to the PRDT
    pub fn add_buffer(&mut self, buffer: &DmaBuffer, interrupt_on_completion: bool) -> AhciResult<()> {
        let pages = buffer.physical_pages();
        let buffer_size = buffer.size();
        let mut remaining = buffer_size;

        for (i, page) in pages.iter().enumerate() {
            let page_addr = page.start_address().as_u64();
            let page_size = core::cmp::min(remaining, 4096);
            
            // Set interrupt only on the last entry
            let is_last = (i == pages.len() - 1) || (remaining <= 4096);
            let interrupt = interrupt_on_completion && is_last;
            
            self.add_region(page_addr, page_size as u32, interrupt)?;
            
            if remaining <= 4096 {
                break;
            }
            remaining -= 4096;
        }

        Ok(())
    }

    /// Build the final PRDT
    pub fn build(self) -> (Vec<HbaPrdtEntry>, u32) {
        (self.entries, self.total_bytes)
    }

    /// Get entry count
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Get total transfer size
    pub fn total_size(&self) -> u32 {
        self.total_bytes
    }
}

impl Default for PrdtBuilder {
    fn default() -> Self {
        Self::new()
    }
}