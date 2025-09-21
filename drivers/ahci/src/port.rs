//! AHCI Port Management
//!
//! This module handles AHCI port initialization, device detection,
//! and low-level command execution. Each port can connect to a single
//! SATA device and manages the command list and FIS structures.

use alloc::{vec, vec::Vec, sync::Arc, boxed::Box};
use spin::Mutex;
use core::ptr::{read_volatile, write_volatile};
use core::mem;
use x86_64::{PhysAddr, VirtAddr};
use crate::memory::dma::{DmaBuffer, BufferId};
use super::{AhciError, AhciResult, consts::*, device::*, command::*};
use super::AhciHba;

/// AHCI Port structure
///
/// Manages a single AHCI port including command list, received FIS area,
/// and command execution. Each port can have one connected device.
pub struct AhciPort {
    /// Port number (0-31)
    port_num: u8,
    /// Reference to HBA for accessing port registers
    hba: Arc<AhciHba>,
    /// Command list DMA buffer
    command_list: Option<Arc<DmaBuffer>>,
    /// Received FIS DMA buffer
    fis_buffer: Option<Arc<DmaBuffer>>,
    /// Command table buffers (one per slot)
    command_tables: Vec<Option<Arc<DmaBuffer>>>,
    /// Available command slots
    available_slots: u32,
    /// Port state
    state: PortState,
    /// Connected device signature
    device_signature: Option<u32>,
    /// Port capabilities
    capabilities: PortCapabilities,
}

/// Port operational state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    /// Port is not initialized
    Uninitialized,
    /// Port is initializing
    Initializing,
    /// Port is ready for commands
    Ready,
    /// Port has detected a device
    DevicePresent,
    /// Port is in error state
    Error,
    /// Port is being reset
    Resetting,
}

/// Port capabilities
#[derive(Debug, Clone)]
struct PortCapabilities {
    /// Maximum number of command slots
    max_command_slots: u8,
    /// Supports command list override
    command_list_override: bool,
    /// Supports FIS-based switching
    fis_based_switching: bool,
    /// Supports external SATA
    external_sata: bool,
    /// Supports hot plug
    hot_plug_capable: bool,
}

impl AhciPort {
    /// Create a new AHCI port
    pub fn new(port_num: u8, hba: Arc<AhciHba>) -> Self {
        Self {
            port_num,
            hba,
            command_list: None,
            fis_buffer: None,
            command_tables: vec![None; HBA_CMD_SLOT_MAX],
            available_slots: if HBA_CMD_SLOT_MAX < 32 {
                (1u32 << HBA_CMD_SLOT_MAX) - 1
            } else {
                u32::MAX
            }, // All slots available
            state: PortState::Uninitialized,
            device_signature: None,
            capabilities: PortCapabilities {
                max_command_slots: HBA_CMD_SLOT_MAX as u8,
                command_list_override: false,
                fis_based_switching: false,
                external_sata: false,
                hot_plug_capable: false,
            },
        }
    }

    /// Initialize the port
    pub fn initialize(&mut self) -> AhciResult<()> {
        self.state = PortState::Initializing;

        // Read port capabilities from HBA
        self.read_capabilities()?;

        // Stop any ongoing operations
        self.stop_engine()?;

        // Allocate and set up command list
        self.setup_command_list()?;

        // Allocate and set up FIS buffer
        self.setup_fis_buffer()?;

        // Clear any pending interrupts
        self.clear_interrupts();

        // Enable FIS receive
        self.enable_fis_receive()?;

        // Start the command engine
        self.start_engine()?;

        self.state = PortState::Ready;
        Ok(())
    }

    /// Read port capabilities from hardware
    fn read_capabilities(&mut self) -> AhciResult<()> {
        let port_base = self.hba.get_port_registers(self.port_num)?;
        
        // Read command register to determine capabilities
        let cmd = unsafe { read_volatile(&(*port_base).cmd) };
        
        self.capabilities.command_list_override = (cmd & PORT_CMD_CLO) != 0;
        self.capabilities.fis_based_switching = false; // Determined from port registers
        self.capabilities.hot_plug_capable = true; // Assume supported
        
        Ok(())
    }

    /// Set up command list DMA buffer
    fn setup_command_list(&mut self) -> AhciResult<()> {
        // Command list: 32 command headers × 32 bytes = 1KB (1K aligned)
        let cl_size = HBA_CMD_SLOT_MAX * HBA_CMD_HEADER_SIZE;
        let buffer = DmaBuffer::new(cl_size).map_err(|_| AhciError::DmaError)?;
        
        // Zero out the command list
        // In a real implementation, we'd map and zero the buffer
        
        // Get physical address for the first page
        let phys_addr = buffer.physical_pages()[0].start_address();
        
        // Set command list base address in port registers
        let port_base = self.hba.get_port_registers(self.port_num)?;
        unsafe {
            write_volatile(&mut (*port_base).clb, phys_addr.as_u64() as u32);
            write_volatile(&mut (*port_base).clbu, (phys_addr.as_u64() >> 32) as u32);
        }
        
    self.command_list = Some(buffer);
        Ok(())
    }

    /// Set up received FIS buffer
    fn setup_fis_buffer(&mut self) -> AhciResult<()> {
        // FIS buffer: 256 bytes (256-byte aligned)
        let buffer = DmaBuffer::new(HBA_FIS_SIZE).map_err(|_| AhciError::DmaError)?;
        
        // Get physical address
        let phys_addr = buffer.physical_pages()[0].start_address();
        
        // Set FIS base address in port registers
        let port_base = self.hba.get_port_registers(self.port_num)?;
        unsafe {
            write_volatile(&mut (*port_base).fb, phys_addr.as_u64() as u32);
            write_volatile(&mut (*port_base).fbu, (phys_addr.as_u64() >> 32) as u32);
        }
        
    self.fis_buffer = Some(buffer);
        Ok(())
    }

    /// Stop the command engine
    fn stop_engine(&self) -> AhciResult<()> {
        let port_base = self.hba.get_port_registers(self.port_num)?;
        
        unsafe {
            // Clear ST (start) bit
            let mut cmd = read_volatile(&(*port_base).cmd);
            cmd &= !PORT_CMD_ST;
            write_volatile(&mut (*port_base).cmd, cmd);
            
            // Wait for CR (command list running) to clear
            let mut timeout = 1000;
            while timeout > 0 {
                let cmd = read_volatile(&(*port_base).cmd);
                if (cmd & PORT_CMD_CR) == 0 {
                    break;
                }
                timeout -= 1;
                // Small delay
                for _ in 0..1000 { core::hint::spin_loop(); }
            }
            
            if timeout == 0 {
                return Err(AhciError::Timeout);
            }
            
            // Clear FRE (FIS receive enable) bit
            let mut cmd = read_volatile(&(*port_base).cmd);
            cmd &= !PORT_CMD_FRE;
            write_volatile(&mut (*port_base).cmd, cmd);
            
            // Wait for FR (FIS receive running) to clear
            timeout = 1000;
            while timeout > 0 {
                let cmd = read_volatile(&(*port_base).cmd);
                if (cmd & PORT_CMD_FR) == 0 {
                    break;
                }
                timeout -= 1;
                for _ in 0..1000 { core::hint::spin_loop(); }
            }
            
            if timeout == 0 {
                return Err(AhciError::Timeout);
            }
        }
        
        Ok(())
    }

    /// Enable FIS receive
    fn enable_fis_receive(&self) -> AhciResult<()> {
        let port_base = self.hba.get_port_registers(self.port_num)?;
        
        unsafe {
            let mut cmd = read_volatile(&(*port_base).cmd);
            cmd |= PORT_CMD_FRE;
            write_volatile(&mut (*port_base).cmd, cmd);
        }
        
        Ok(())
    }

    /// Start the command engine
    fn start_engine(&self) -> AhciResult<()> {
        let port_base = self.hba.get_port_registers(self.port_num)?;
        
        unsafe {
            // Make sure FIS receive is running first
            let cmd = read_volatile(&(*port_base).cmd);
            if (cmd & PORT_CMD_FR) == 0 {
                return Err(AhciError::DeviceNotReady);
            }
            
            // Set ST (start) bit
            let mut cmd = read_volatile(&(*port_base).cmd);
            cmd |= PORT_CMD_ST;
            write_volatile(&mut (*port_base).cmd, cmd);
        }
        
        Ok(())
    }

    /// Clear all pending interrupts
    fn clear_interrupts(&self) {
        if let Ok(port_base) = self.hba.get_port_registers(self.port_num) {
            unsafe {
                // Clear interrupt status register (write 1 to clear)
                let is = read_volatile(&(*port_base).is);
                write_volatile(&mut (*port_base).is, is);
                
                // Clear SATA error register
                let serr = read_volatile(&(*port_base).serr);
                write_volatile(&mut (*port_base).serr, serr);
            }
        }
    }

    /// Detect connected device
    pub fn detect_device(&mut self) -> AhciResult<Option<AhciDevice>> {
        let port_base = self.hba.get_port_registers(self.port_num)?;
        
        let ssts = unsafe { read_volatile(&(*port_base).ssts) };
        let det = ssts & SSTS_DET_MASK;
        let ipm = ssts & SSTS_IPM_MASK;
        
        // Check if device is present and active
        if det == SSTS_DET_PRESENT && ipm == SSTS_IPM_ACTIVE {
            let signature = unsafe { read_volatile(&(*port_base).sig) };
            self.device_signature = Some(signature);
            self.state = PortState::DevicePresent;
            
            // Create device object
            let device = AhciDevice::new(Arc::new(Mutex::new(self.clone())), signature)?;
            Ok(Some(device))
        } else {
            self.device_signature = None;
            Ok(None)
        }
    }

    /// Execute an ATA command
    pub fn execute_command(&mut self, command: AtaCommand) -> AhciResult<CommandResult> {
        if self.state != PortState::Ready && self.state != PortState::DevicePresent {
            return Err(AhciError::DeviceNotReady);
        }

        // Find available command slot
        let slot = self.allocate_slot().ok_or(AhciError::NoAvailableSlots)?;

        // Set up command table
        self.setup_command_table(slot, &command)?;

        // Set up command header
        self.setup_command_header(slot, &command)?;

        // Issue command
        self.issue_command(slot)?;

        // Wait for completion
        let result = self.wait_for_completion(slot)?;

        // Release command slot
        self.release_slot(slot);

        Ok(result)
    }

    /// Allocate a command slot
    fn allocate_slot(&mut self) -> Option<u8> {
        for slot in 0..HBA_CMD_SLOT_MAX {
            if (self.available_slots & (1 << slot)) != 0 {
                self.available_slots &= !(1 << slot);
                return Some(slot as u8);
            }
        }
        None
    }

    /// Release a command slot
    fn release_slot(&mut self, slot: u8) {
        self.available_slots |= 1 << slot;
    }

    /// Set up command table for a command
    fn setup_command_table(&mut self, slot: u8, command: &AtaCommand) -> AhciResult<()> {
        // Allocate command table if not already allocated
        if self.command_tables[slot as usize].is_none() {
            // Command table size: 128 bytes header + PRDT entries
            let prdt_entries = command.get_prdt_count();
            let table_size = HBA_CMD_TBL_HEADER + (prdt_entries * HBA_PRDT_ENTRY_SIZE);
            
            let buffer = DmaBuffer::new(table_size).map_err(|_| AhciError::DmaError)?;
            self.command_tables[slot as usize] = Some(buffer);
        }

        let table_buffer = self.command_tables[slot as usize].as_ref().unwrap();
        
        // In a real implementation, we'd map the buffer and set up the command table
        // For now, we'll just verify the buffer exists
        
        Ok(())
    }

    /// Set up command header
    fn setup_command_header(&self, slot: u8, command: &AtaCommand) -> AhciResult<()> {
        let command_list = self.command_list.as_ref().ok_or(AhciError::InvalidPort)?;
        
        // In a real implementation, we'd map the command list buffer and set up the header
        // This would include setting the command FIS length, flags, and PRDT count
        
        Ok(())
    }

    /// Issue command to hardware
    fn issue_command(&self, slot: u8) -> AhciResult<()> {
        let port_base = self.hba.get_port_registers(self.port_num)?;
        
        unsafe {
            // Set the command bit in the command issue register
            write_volatile(&mut (*port_base).ci, 1 << slot);
        }
        
        Ok(())
    }

    /// Wait for command completion
    fn wait_for_completion(&self, slot: u8) -> AhciResult<CommandResult> {
        let port_base = self.hba.get_port_registers(self.port_num)?;
        let slot_mask = 1 << slot;
        
        // Wait for command to complete (simplified polling)
        let mut timeout = 10000; // Timeout in iterations
        
        while timeout > 0 {
            let ci = unsafe { read_volatile(&(*port_base).ci) };
            
            // Command completed when bit is cleared
            if (ci & slot_mask) == 0 {
                // Check for errors
                let is = unsafe { read_volatile(&(*port_base).is) };
                let tfd = unsafe { read_volatile(&(*port_base).tfd) };
                
                if (is & (PORT_IS_TFES | PORT_IS_HBFS | PORT_IS_HBDS | PORT_IS_IFS)) != 0 {
                    // Clear error interrupts
                    unsafe { write_volatile(&mut (*port_base).is, is); }
                    return Ok(CommandResult::Error((tfd & 0xFF) as u8));
                }
                
                // Clear completion interrupts
                unsafe { write_volatile(&mut (*port_base).is, is); }
                return Ok(CommandResult::Success);
            }
            
            timeout -= 1;
            // Small delay
            for _ in 0..100 { core::hint::spin_loop(); }
        }
        
        Err(AhciError::Timeout)
    }

    /// Reset the port
    pub fn reset(&mut self) -> AhciResult<()> {
        self.state = PortState::Resetting;
        
        // Stop the command engine
        self.stop_engine()?;
        
        // Perform COMRESET
        let port_base = self.hba.get_port_registers(self.port_num)?;
        unsafe {
            let mut sctl = read_volatile(&(*port_base).sctl);
            sctl = (sctl & !0xF) | 1; // DET = 1 (COMRESET)
            write_volatile(&mut (*port_base).sctl, sctl);
            
            // Wait 1ms
            for _ in 0..10000 { core::hint::spin_loop(); }
            
            sctl &= !0xF; // DET = 0 (no action)
            write_volatile(&mut (*port_base).sctl, sctl);
        }
        
        // Wait for device to become ready
        let mut timeout = 1000;
        while timeout > 0 {
            let ssts = unsafe { read_volatile(&(*port_base).ssts) };
            let det = ssts & SSTS_DET_MASK;
            let ipm = ssts & SSTS_IPM_MASK;
            
            if det == SSTS_DET_PRESENT && ipm == SSTS_IPM_ACTIVE {
                break;
            }
            
            timeout -= 1;
            for _ in 0..1000 { core::hint::spin_loop(); }
        }
        
        if timeout == 0 {
            self.state = PortState::Error;
            return Err(AhciError::ResetFailed);
        }
        
        // Re-initialize the port
        self.initialize()?;
        
        Ok(())
    }

    /// Get port number
    pub fn port_number(&self) -> u8 {
        self.port_num
    }

    /// Get port state
    pub fn state(&self) -> PortState {
        self.state
    }

    /// Get device signature if present
    pub fn device_signature(&self) -> Option<u32> {
        self.device_signature
    }
}

// We need Clone for the device creation, but this is a simplified implementation
impl Clone for AhciPort {
    fn clone(&self) -> Self {
        Self {
            port_num: self.port_num,
            hba: self.hba.clone(),
            command_list: self.command_list.clone(),
            fis_buffer: self.fis_buffer.clone(),
            command_tables: self.command_tables.clone(),
            available_slots: self.available_slots,
            state: self.state,
            device_signature: self.device_signature,
            capabilities: self.capabilities.clone(),
        }
    }
}

/// Port statistics for monitoring
#[derive(Debug, Clone)]
pub struct PortStats {
    pub commands_completed: u64,
    pub commands_failed: u64,
    pub bytes_transferred: u64,
    pub average_latency_us: u32,
    pub queue_depth: u8,
    pub error_rate: f32,
}

impl Default for PortStats {
    fn default() -> Self {
        Self {
            commands_completed: 0,
            commands_failed: 0,
            bytes_transferred: 0,
            average_latency_us: 0,
            queue_depth: 0,
            error_rate: 0.0,
        }
    }
}