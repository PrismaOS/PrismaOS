//! AHCI Device Abstraction
//!
//! This module provides high-level device abstractions for AHCI storage devices.
//! It handles device identification, capability detection, and provides a unified
//! interface for read/write operations regardless of the underlying device type.

extern crate alloc;

use alloc::{string::String, sync::Arc};
use spin::Mutex;
use lib_kernel::memory::dma::{DmaBuffer, BufferId};
use super::{AhciError, AhciResult, consts::*};
use super::port::AhciPort;
use super::command::AtaCommand;

/// Device type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Standard SATA hard drive or SSD
    Ata,
    /// ATAPI device (CD/DVD/BD drives)
    Atapi,
    /// Enclosure management bridge
    Semb,
    /// Port multiplier
    PortMultiplier,
    /// Unknown device type
    Unknown(u32),
}

impl DeviceType {
    /// Create device type from signature
    pub fn from_signature(sig: u32) -> Self {
        match sig {
            SATA_SIG_ATA => DeviceType::Ata,
            SATA_SIG_ATAPI => DeviceType::Atapi,
            SATA_SIG_SEMB => DeviceType::Semb,
            SATA_SIG_PM => DeviceType::PortMultiplier,
            other => DeviceType::Unknown(other),
        }
    }

    /// Check if device supports standard ATA commands
    pub fn supports_ata_commands(&self) -> bool {
        matches!(self, DeviceType::Ata)
    }

    /// Check if device supports ATAPI commands
    pub fn supports_atapi_commands(&self) -> bool {
        matches!(self, DeviceType::Atapi)
    }
}

/// Device capability flags
#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    /// Supports 48-bit LBA addressing
    pub lba48: bool,
    /// Supports Native Command Queuing
    pub ncq: bool,
    /// Maximum queue depth for NCQ
    pub max_queue_depth: u8,
    /// Supports TRIM/UNMAP command
    pub trim_support: bool,
    /// Supports SMART monitoring
    pub smart_support: bool,
    /// Supports write cache
    pub write_cache: bool,
    /// Supports read look-ahead
    pub read_lookahead: bool,
    /// Device is removable
    pub removable: bool,
    /// Support for Advanced Power Management
    pub apm_support: bool,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            lba48: false,
            ncq: false,
            max_queue_depth: 1,
            trim_support: false,
            smart_support: false,
            write_cache: false,
            read_lookahead: false,
            removable: false,
            apm_support: false,
        }
    }
}

/// Device geometry and identification information
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Device type
    pub device_type: DeviceType,
    /// Device model string
    pub model: String,
    /// Device serial number
    pub serial: String,
    /// Firmware revision
    pub firmware: String,
    /// Total sectors (LBA mode)
    pub total_sectors: u64,
    /// Sector size in bytes
    pub sector_size: u32,
    /// Physical sector size (for 4K native drives)
    pub physical_sector_size: u32,
    /// Device capabilities
    pub capabilities: DeviceCapabilities,
    /// Is device currently present and ready
    pub present: bool,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            device_type: DeviceType::Unknown(0),
            model: String::new(),
            serial: String::new(),
            firmware: String::new(),
            total_sectors: 0,
            sector_size: 512,
            physical_sector_size: 512,
            capabilities: DeviceCapabilities::default(),
            present: false,
        }
    }
}

/// AHCI storage device
///
/// Represents a single storage device connected to an AHCI port.
/// Provides high-level read/write interface and device management.
pub struct AhciDevice {
    /// Associated AHCI port
    port: Arc<Mutex<AhciPort>>,
    /// Device information and capabilities
    info: DeviceInfo,
    /// Device is currently accessible
    online: bool,
    /// Error count for health monitoring
    error_count: u32,
    /// Last successful operation timestamp
    last_activity: u64,
}

impl AhciDevice {
    /// Create a new AHCI device
    pub fn new(port: Arc<Mutex<AhciPort>>, signature: u32) -> AhciResult<Self> {
        let mut device = Self {
            port,
            info: DeviceInfo {
                device_type: DeviceType::from_signature(signature),
                ..Default::default()
            },
            online: false,
            error_count: 0,
            last_activity: 0,
        };

        // Identify the device to get detailed information
        device.identify()?;
        device.online = true;

        Ok(device)
    }

    /// Identify device and populate device information
    fn identify(&mut self) -> AhciResult<()> {
        // Only ATA devices support standard IDENTIFY command
        if !self.info.device_type.supports_ata_commands() {
            return Ok(()); // ATAPI devices need different identification
        }

        // Allocate buffer for IDENTIFY DATA
        let buffer = DmaBuffer::new(512).map_err(|_| AhciError::DmaError)?;
        let buffer_id = buffer.id();

        // Issue IDENTIFY DEVICE command
        let command = AtaCommand::identify(buffer_id);
        let result = {
            let mut port = self.port.lock();
            port.execute_command(command)?
        };

        if !result.is_success() {
            return Err(AhciError::DeviceNotReady);
        }

        // Parse IDENTIFY data
        self.parse_identify_data(&buffer)?;

        Ok(())
    }

    /// Parse ATA IDENTIFY DEVICE data
    fn parse_identify_data(&mut self, buffer: &DmaBuffer) -> AhciResult<()> {
        // Map buffer to read identification data
        // This is a simplified implementation - real code would need proper mapping
        
        // For now, set some default values
        self.info.model = "AHCI Storage Device".into();
        self.info.serial = "DEV001".into();
        self.info.firmware = "1.0".into();
        self.info.total_sectors = 1024 * 1024; // 512MB default
        self.info.sector_size = 512;
        self.info.physical_sector_size = 512;
        
        // Set basic capabilities
        self.info.capabilities.lba48 = true;
        self.info.capabilities.ncq = false;
        self.info.capabilities.max_queue_depth = 1;
        self.info.capabilities.smart_support = true;
        
        self.info.present = true;

        Ok(())
    }

    /// Read sectors from device
    ///
    /// # Arguments
    /// * `lba` - Starting logical block address
    /// * `sectors` - Number of sectors to read
    /// * `buffer` - DMA buffer to store read data
    ///
    /// # Returns
    /// `Ok(())` on success, `AhciError` on failure
    pub fn read(&mut self, lba: u64, sectors: u32, buffer: BufferId) -> AhciResult<()> {
        if !self.online {
            return Err(AhciError::DeviceNotReady);
        }

        if !self.info.device_type.supports_ata_commands() {
            return Err(AhciError::UnsupportedDevice);
        }

        // Validate parameters
        if lba + sectors as u64 > self.info.total_sectors {
            return Err(AhciError::InvalidParameters);
        }

        if sectors == 0 {
            return Err(AhciError::InvalidParameters);
        }

        // Choose appropriate read command based on capabilities
        let command = if self.info.capabilities.lba48 && (lba > 0xFFFFFFF || sectors > 255) {
            AtaCommand::read_dma_ext(lba, sectors, buffer)
        } else {
            AtaCommand::read_dma(lba as u32, sectors as u8, buffer)
        };

        // Execute command
        let mut port = self.port.lock();
        let result = port.execute_command(command)?;

        if result.is_success() {
            self.last_activity = lib_kernel::time::get_timestamp();
            Ok(())
        } else {
            self.error_count += 1;
            Err(AhciError::HardwareError)
        }
    }

    /// Write sectors to device
    ///
    /// # Arguments
    /// * `lba` - Starting logical block address
    /// * `sectors` - Number of sectors to write
    /// * `buffer` - DMA buffer containing data to write
    ///
    /// # Returns
    /// `Ok(())` on success, `AhciError` on failure
    pub fn write(&mut self, lba: u64, sectors: u32, buffer: BufferId) -> AhciResult<()> {
        if !self.online {
            return Err(AhciError::DeviceNotReady);
        }

        if !self.info.device_type.supports_ata_commands() {
            return Err(AhciError::UnsupportedDevice);
        }

        // Validate parameters
        if lba + sectors as u64 > self.info.total_sectors {
            return Err(AhciError::InvalidParameters);
        }

        if sectors == 0 {
            return Err(AhciError::InvalidParameters);
        }

        // Choose appropriate write command based on capabilities
        let command = if self.info.capabilities.lba48 && (lba > 0xFFFFFFF || sectors > 255) {
            AtaCommand::write_dma_ext(lba, sectors, buffer)
        } else {
            AtaCommand::write_dma(lba as u32, sectors as u8, buffer)
        };

        // Execute command
        let mut port = self.port.lock();
        let result = port.execute_command(command)?;

        if result.is_success() {
            self.last_activity = lib_kernel::time::get_timestamp();
            Ok(())
        } else {
            self.error_count += 1;
            Err(AhciError::HardwareError)
        }
    }

    /// Flush write cache to ensure data persistence
    pub fn flush(&mut self) -> AhciResult<()> {
        if !self.online {
            return Err(AhciError::DeviceNotReady);
        }

        if !self.info.capabilities.write_cache {
            return Ok(()); // No cache to flush
        }

        let command = AtaCommand::flush_cache();
        let mut port = self.port.lock();
        let result = port.execute_command(command)?;

        if result.is_success() {
            Ok(())
        } else {
            self.error_count += 1;
            Err(AhciError::HardwareError)
        }
    }

    /// Get device information
    pub fn get_info(&self) -> DeviceInfo {
        self.info.clone()
    }

    /// Check if device is online and ready
    pub fn is_online(&self) -> bool {
        self.online
    }

    /// Get device error count
    pub fn error_count(&self) -> u32 {
        self.error_count
    }

    /// Reset error count
    pub fn reset_error_count(&mut self) {
        self.error_count = 0;
    }

    /// Perform device health check
    pub fn health_check(&mut self) -> AhciResult<DeviceHealth> {
        if !self.online {
            return Ok(DeviceHealth::Offline);
        }

        // Simple health assessment based on error count
        let health = match self.error_count {
            0..=5 => DeviceHealth::Good,
            6..=20 => DeviceHealth::Degraded,
            _ => DeviceHealth::Poor,
        };

        Ok(health)
    }

    /// Handle device hotplug events
    pub fn handle_hotplug(&mut self, connected: bool) -> AhciResult<()> {
        if connected && !self.online {
            // Device reconnected - re-identify
            self.identify()?;
            self.online = true;
            self.error_count = 0;
        } else if !connected && self.online {
            // Device disconnected
            self.online = false;
            self.info.present = false;
        }

        Ok(())
    }
}

/// Device health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceHealth {
    /// Device is operating normally
    Good,
    /// Device has some errors but is functional
    Degraded,
    /// Device has many errors and may fail soon
    Poor,
    /// Device is not responding
    Offline,
}

impl DeviceHealth {
    /// Get health as a percentage (0-100)
    pub fn as_percentage(&self) -> u8 {
        match self {
            DeviceHealth::Good => 100,
            DeviceHealth::Degraded => 60,
            DeviceHealth::Poor => 20,
            DeviceHealth::Offline => 0,
        }
    }

    /// Check if device health is acceptable for normal operations
    pub fn is_acceptable(&self) -> bool {
        matches!(self, DeviceHealth::Good | DeviceHealth::Degraded)
    }
}