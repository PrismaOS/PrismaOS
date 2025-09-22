/// USB driver error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbError {
    /// Hardware not found or inaccessible
    HardwareNotFound,
    /// Invalid controller type
    InvalidController,
    /// Controller initialization failed
    InitializationFailed,
    /// Device enumeration failed
    EnumerationFailed,
    /// Transfer timeout
    TransferTimeout,
    /// Transfer failed
    TransferFailed,
    /// Device not found
    DeviceNotFound,
    /// Invalid endpoint
    InvalidEndpoint,
    /// Invalid descriptor
    InvalidDescriptor,
    /// Buffer too small
    BufferTooSmall,
    /// Invalid request
    InvalidRequest,
    /// Not supported
    NotSupported,
    /// Power management error
    PowerError,
    /// Hub error
    HubError,
    /// Memory allocation error
    OutOfMemory,
    /// Generic hardware error
    HardwareError,
    /// Controller halted
    ControllerHalted,
    /// Device disconnected
    DeviceDisconnected,
    /// Stall condition
    Stall,
    /// Babble condition
    Babble,
    /// Data buffer error
    DataBuffer,
    /// Transaction error
    TransactionError,
    /// Missing acknowledge
    MissedMicroFrame,
    /// Split transaction error
    SplitTransactionError,
    /// Transfer was cancelled
    TransferCancelled,
}

impl UsbError {
    /// Get a human-readable description of the error
    pub fn description(self) -> &'static str {
        match self {
            UsbError::HardwareNotFound => "USB hardware not found or inaccessible",
            UsbError::InvalidController => "Invalid USB controller type",
            UsbError::InitializationFailed => "USB controller initialization failed",
            UsbError::EnumerationFailed => "USB device enumeration failed",
            UsbError::TransferTimeout => "USB transfer timeout",
            UsbError::TransferFailed => "USB transfer failed",
            UsbError::DeviceNotFound => "USB device not found",
            UsbError::InvalidEndpoint => "Invalid USB endpoint",
            UsbError::InvalidDescriptor => "Invalid USB descriptor",
            UsbError::BufferTooSmall => "Buffer too small for USB operation",
            UsbError::InvalidRequest => "Invalid USB request",
            UsbError::NotSupported => "USB operation not supported",
            UsbError::PowerError => "USB power management error",
            UsbError::HubError => "USB hub error",
            UsbError::OutOfMemory => "USB memory allocation error",
            UsbError::HardwareError => "USB hardware error",
            UsbError::ControllerHalted => "USB controller halted",
            UsbError::DeviceDisconnected => "USB device disconnected",
            UsbError::Stall => "USB endpoint stall condition",
            UsbError::Babble => "USB babble condition",
            UsbError::DataBuffer => "USB data buffer error",
            UsbError::TransactionError => "USB transaction error",
            UsbError::MissedMicroFrame => "USB missed microframe",
            UsbError::SplitTransactionError => "USB split transaction error",
            UsbError::TransferCancelled => "USB transfer was cancelled",
        }
    }
}

/// Result type for USB operations
pub type Result<T> = core::result::Result<T, UsbError>;