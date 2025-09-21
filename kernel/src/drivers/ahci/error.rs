//! AHCI Driver Error Types
//!
//! This module defines all error types and error handling functionality
//! for the AHCI driver. It provides detailed error information for
//! debugging and recovery operations.

use core::fmt;

/// AHCI driver error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AhciError {
    /// No AHCI controllers found during PCI scan
    NoControllersFound,

    /// Invalid or missing ABAR (AHCI Base Address Register)
    InvalidBar,

    /// Memory mapping failed for HBA registers
    MappingFailed,

    /// Device not found or invalid device ID
    DeviceNotFound,

    /// Port is not implemented or invalid
    InvalidPort,

    /// Device is not ready or unresponsive
    DeviceNotReady,

    /// Command timeout occurred
    Timeout,

    /// Hardware error reported by device
    HardwareError,

    /// DMA buffer allocation or access failed
    DmaError,

    /// Invalid parameters passed to function
    InvalidParameters,

    /// Command slot allocation failed (all slots busy)
    NoAvailableSlots,

    /// Unsupported device type
    UnsupportedDevice,

    /// Port reset failed
    ResetFailed,

    /// Interface communication error
    InterfaceError,

    /// Task file error (device-reported error)
    TaskFileError(u8),

    /// AHCI capability not supported
    UnsupportedCapability,

    /// Buffer size mismatch or alignment error
    BufferError,

    /// Operation interrupted
    Interrupted,

    /// Resource busy (try again later)
    Busy,

    /// Critical firmware/hardware incompatibility
    FirmwareError,
}

impl AhciError {
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            AhciError::Timeout => true,
            AhciError::Busy => true,
            AhciError::DeviceNotReady => true,
            AhciError::NoAvailableSlots => true,
            AhciError::Interrupted => true,
            _ => false,
        }
    }

    /// Check if error indicates hardware failure
    pub fn is_hardware_error(&self) -> bool {
        match self {
            AhciError::HardwareError => true,
            AhciError::TaskFileError(_) => true,
            AhciError::ResetFailed => true,
            AhciError::InterfaceError => true,
            AhciError::FirmwareError => true,
            _ => false,
        }
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AhciError::NoControllersFound => ErrorSeverity::Critical,
            AhciError::FirmwareError => ErrorSeverity::Critical,
            AhciError::HardwareError => ErrorSeverity::Error,
            AhciError::TaskFileError(_) => ErrorSeverity::Error,
            AhciError::ResetFailed => ErrorSeverity::Error,
            AhciError::DeviceNotFound => ErrorSeverity::Warning,
            AhciError::Timeout => ErrorSeverity::Warning,
            AhciError::Busy => ErrorSeverity::Info,
            _ => ErrorSeverity::Warning,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Critical system error - driver cannot continue
    Critical,
    /// Serious error - device may be unusable
    Error,
    /// Warning - operation failed but driver can continue
    Warning,
    /// Informational - temporary condition
    Info,
}

impl fmt::Display for AhciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AhciError::NoControllersFound =>
                write!(f, "No AHCI controllers found during PCI scan"),
            AhciError::InvalidBar =>
                write!(f, "Invalid or missing AHCI Base Address Register"),
            AhciError::MappingFailed =>
                write!(f, "Failed to map HBA memory registers"),
            AhciError::DeviceNotFound =>
                write!(f, "Storage device not found or invalid device ID"),
            AhciError::InvalidPort =>
                write!(f, "Port is not implemented or invalid"),
            AhciError::DeviceNotReady =>
                write!(f, "Device is not ready or unresponsive"),
            AhciError::Timeout =>
                write!(f, "Command execution timeout"),
            AhciError::HardwareError =>
                write!(f, "Hardware error reported by device"),
            AhciError::DmaError =>
                write!(f, "DMA buffer allocation or access failed"),
            AhciError::InvalidParameters =>
                write!(f, "Invalid parameters passed to function"),
            AhciError::NoAvailableSlots =>
                write!(f, "All command slots are busy"),
            AhciError::UnsupportedDevice =>
                write!(f, "Unsupported device type"),
            AhciError::ResetFailed =>
                write!(f, "Port reset operation failed"),
            AhciError::InterfaceError =>
                write!(f, "SATA interface communication error"),
            AhciError::TaskFileError(code) =>
                write!(f, "Device task file error: 0x{:02X}", code),
            AhciError::UnsupportedCapability =>
                write!(f, "Required AHCI capability not supported"),
            AhciError::BufferError =>
                write!(f, "Buffer size mismatch or alignment error"),
            AhciError::Interrupted =>
                write!(f, "Operation was interrupted"),
            AhciError::Busy =>
                write!(f, "Resource is busy, try again later"),
            AhciError::FirmwareError =>
                write!(f, "Critical firmware or hardware incompatibility"),
        }
    }
}

/// AHCI Result type alias
pub type AhciResult<T> = Result<T, AhciError>;

/// Error context for detailed error reporting
#[derive(Debug)]
pub struct ErrorContext {
    pub error: AhciError,
    pub controller_id: Option<u32>,
    pub port_id: Option<u8>,
    pub command_slot: Option<u8>,
    pub lba: Option<u64>,
    pub sector_count: Option<u32>,
    pub register_state: Option<RegisterSnapshot>,
}

/// Snapshot of relevant AHCI registers for error analysis
#[derive(Debug, Clone)]
pub struct RegisterSnapshot {
    pub port_status: u32,
    pub port_error: u32,
    pub port_interrupt_status: u32,
    pub task_file_data: u32,
    pub command_issue: u32,
    pub sata_status: u32,
    pub sata_error: u32,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(error: AhciError) -> Self {
        Self {
            error,
            controller_id: None,
            port_id: None,
            command_slot: None,
            lba: None,
            sector_count: None,
            register_state: None,
        }
    }

    /// Add controller context
    pub fn with_controller(mut self, controller_id: u32) -> Self {
        self.controller_id = Some(controller_id);
        self
    }

    /// Add port context
    pub fn with_port(mut self, port_id: u8) -> Self {
        self.port_id = Some(port_id);
        self
    }

    /// Add command context
    pub fn with_command(mut self, slot: u8, lba: u64, sectors: u32) -> Self {
        self.command_slot = Some(slot);
        self.lba = Some(lba);
        self.sector_count = Some(sectors);
        self
    }

    /// Add register snapshot
    pub fn with_registers(mut self, snapshot: RegisterSnapshot) -> Self {
        self.register_state = Some(snapshot);
        self
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AHCI Error: {}", self.error)?;

        if let Some(ctrl_id) = self.controller_id {
            write!(f, " [Controller {}]", ctrl_id)?;
        }

        if let Some(port_id) = self.port_id {
            write!(f, " [Port {}]", port_id)?;
        }

        if let Some(slot) = self.command_slot {
            write!(f, " [Slot {}]", slot)?;
        }

        if let Some(lba) = self.lba {
            write!(f, " [LBA 0x{:X}]", lba)?;
        }

        if let Some(sectors) = self.sector_count {
            write!(f, " [Sectors {}]", sectors)?;
        }

        Ok(())
    }
}

/// Macro for creating error contexts with location information
#[macro_export]
macro_rules! ahci_error {
    ($error:expr) => {
        ErrorContext::new($error)
    };
    ($error:expr, controller = $ctrl:expr) => {
        ErrorContext::new($error).with_controller($ctrl)
    };
    ($error:expr, port = $port:expr) => {
        ErrorContext::new($error).with_port($port)
    };
    ($error:expr, controller = $ctrl:expr, port = $port:expr) => {
        ErrorContext::new($error).with_controller($ctrl).with_port($port)
    };
}