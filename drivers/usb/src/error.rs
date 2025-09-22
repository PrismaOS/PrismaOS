//! USB Driver Error Types

use core::fmt;
use usb_device::UsbError;

/// USB Driver Error Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbDriverError {
    /// Device enumeration failed
    EnumerationFailed,
    /// Invalid device configuration
    InvalidConfiguration,
    /// Transfer timeout
    TransferTimeout,
    /// Transfer failed
    TransferFailed,
    /// Device not found
    DeviceNotFound,
    /// Endpoint error
    EndpointError,
    /// Hub error
    HubError,
    /// Controller error
    ControllerError,
    /// Memory allocation error
    MemoryError,
    /// Invalid parameter
    InvalidParameter,
    /// Operation not supported
    NotSupported,
    /// Device disconnected
    DeviceDisconnected,
    /// Bus error
    BusError,
    /// Protocol error
    ProtocolError,
    /// Timeout
    Timeout,
    /// Buffer overflow
    BufferOverflow,
    /// Underlying USB error
    UsbError(UsbError),
    /// xHCI specific error
    XhciError(&'static str),
    /// Power management error
    PowerError,
    /// Initialization error
    InitializationError,
}

impl fmt::Display for UsbDriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsbDriverError::EnumerationFailed => write!(f, "Device enumeration failed"),
            UsbDriverError::InvalidConfiguration => write!(f, "Invalid device configuration"),
            UsbDriverError::TransferTimeout => write!(f, "Transfer timeout"),
            UsbDriverError::TransferFailed => write!(f, "Transfer failed"),
            UsbDriverError::DeviceNotFound => write!(f, "Device not found"),
            UsbDriverError::EndpointError => write!(f, "Endpoint error"),
            UsbDriverError::HubError => write!(f, "Hub error"),
            UsbDriverError::ControllerError => write!(f, "Controller error"),
            UsbDriverError::MemoryError => write!(f, "Memory allocation error"),
            UsbDriverError::InvalidParameter => write!(f, "Invalid parameter"),
            UsbDriverError::NotSupported => write!(f, "Operation not supported"),
            UsbDriverError::DeviceDisconnected => write!(f, "Device disconnected"),
            UsbDriverError::BusError => write!(f, "Bus error"),
            UsbDriverError::ProtocolError => write!(f, "Protocol error"),
            UsbDriverError::Timeout => write!(f, "Timeout"),
            UsbDriverError::BufferOverflow => write!(f, "Buffer overflow"),
            UsbDriverError::UsbError(e) => write!(f, "USB error: {:?}", e),
            UsbDriverError::XhciError(msg) => write!(f, "xHCI error: {}", msg),
            UsbDriverError::PowerError => write!(f, "Power management error"),
            UsbDriverError::InitializationError => write!(f, "Initialization error"),
        }
    }
}

impl From<UsbError> for UsbDriverError {
    fn from(error: UsbError) -> Self {
        UsbDriverError::UsbError(error)
    }
}