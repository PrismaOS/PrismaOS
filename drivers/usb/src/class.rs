//! USB Class Driver Interfaces

use alloc::{vec::Vec, boxed::Box, string::{String, ToString}, sync::Arc};
use core::{fmt, future::Future, pin::Pin};
use spin::Mutex;
use crate::{
    Result, UsbDriverError,
    device::{UsbDevice, DeviceClass},
    descriptor::class_codes,
};

/// USB Class Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassType {
    Audio,
    CommAndCdcControl,
    Hid,
    Physical,
    Image,
    Printer,
    MassStorage,
    Hub,
    CdcData,
    SmartCard,
    ContentSecurity,
    Video,
    PersonalHealthcare,
    AudioVideo,
    Billboard,
    Diagnostic,
    Wireless,
    Miscellaneous,
    ApplicationSpecific,
    VendorSpecific,
}

impl From<u8> for ClassType {
    fn from(class_code: u8) -> Self {
        match class_code {
            class_codes::AUDIO => ClassType::Audio,
            class_codes::COMM_AND_CDC_CONTROL => ClassType::CommAndCdcControl,
            class_codes::HID => ClassType::Hid,
            class_codes::PHYSICAL => ClassType::Physical,
            class_codes::IMAGE => ClassType::Image,
            class_codes::PRINTER => ClassType::Printer,
            class_codes::MASS_STORAGE => ClassType::MassStorage,
            class_codes::HUB => ClassType::Hub,
            class_codes::CDC_DATA => ClassType::CdcData,
            class_codes::SMART_CARD => ClassType::SmartCard,
            class_codes::CONTENT_SECURITY => ClassType::ContentSecurity,
            class_codes::VIDEO => ClassType::Video,
            class_codes::PERSONAL_HEALTHCARE => ClassType::PersonalHealthcare,
            class_codes::AUDIO_VIDEO => ClassType::AudioVideo,
            class_codes::BILLBOARD => ClassType::Billboard,
            class_codes::DIAGNOSTIC => ClassType::Diagnostic,
            class_codes::WIRELESS => ClassType::Wireless,
            class_codes::MISCELLANEOUS => ClassType::Miscellaneous,
            class_codes::APPLICATION_SPECIFIC => ClassType::ApplicationSpecific,
            _ => ClassType::VendorSpecific,
        }
    }
}

impl Into<u8> for ClassType {
    fn into(self) -> u8 {
        match self {
            ClassType::Audio => class_codes::AUDIO,
            ClassType::CommAndCdcControl => class_codes::COMM_AND_CDC_CONTROL,
            ClassType::Hid => class_codes::HID,
            ClassType::Physical => class_codes::PHYSICAL,
            ClassType::Image => class_codes::IMAGE,
            ClassType::Printer => class_codes::PRINTER,
            ClassType::MassStorage => class_codes::MASS_STORAGE,
            ClassType::Hub => class_codes::HUB,
            ClassType::CdcData => class_codes::CDC_DATA,
            ClassType::SmartCard => class_codes::SMART_CARD,
            ClassType::ContentSecurity => class_codes::CONTENT_SECURITY,
            ClassType::Video => class_codes::VIDEO,
            ClassType::PersonalHealthcare => class_codes::PERSONAL_HEALTHCARE,
            ClassType::AudioVideo => class_codes::AUDIO_VIDEO,
            ClassType::Billboard => class_codes::BILLBOARD,
            ClassType::Diagnostic => class_codes::DIAGNOSTIC,
            ClassType::Wireless => class_codes::WIRELESS,
            ClassType::Miscellaneous => class_codes::MISCELLANEOUS,
            ClassType::ApplicationSpecific => class_codes::APPLICATION_SPECIFIC,
            ClassType::VendorSpecific => class_codes::VENDOR_SPECIFIC,
        }
    }
}

/// USB Class Driver trait
pub trait UsbClassDriver: fmt::Debug + Send + Sync {
    /// Get the class type this driver supports
    fn class_type(&self) -> ClassType;

    /// Check if this driver supports the given device
    fn supports_device(&self, device: &UsbDevice) -> bool;

    /// Attach to a device
    fn attach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Detach from a device
    fn detach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Handle device events
    fn handle_event(&self, device_address: u8, event: ClassEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Get driver name
    fn name(&self) -> &str;

    /// Get driver version
    fn version(&self) -> &str {
        "1.0.0"
    }
}

/// USB Class Events
#[derive(Debug, Clone)]
pub enum ClassEvent {
    /// Device configuration changed
    ConfigurationChanged { config: u8 },
    /// Interface alternate setting changed
    AlternateSettingChanged { interface: u8, alternate: u8 },
    /// Endpoint error occurred
    EndpointError { endpoint: u8, error: UsbDriverError },
    /// Transfer completed
    TransferCompleted { endpoint: u8, bytes: usize },
    /// Device suspended
    Suspended,
    /// Device resumed
    Resumed,
    /// Device reset
    Reset,
}

/// Mass Storage Class Driver
#[derive(Debug)]
pub struct MassStorageDriver {
    name: String,
    attached_devices: Arc<Mutex<Vec<u8>>>, // Device addresses
}

impl MassStorageDriver {
    /// Create a new mass storage driver
    pub fn new() -> Self {
        Self {
            name: "USB Mass Storage Driver".to_string(),
            attached_devices: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Read from storage device
    pub async fn read_blocks(
        &self,
        device_address: u8,
        start_block: u64,
        block_count: u32,
        buffer: &mut [u8],
    ) -> Result<usize> {
        // Implement SCSI READ(10) command
        // This is simplified
        Ok(buffer.len())
    }

    /// Write to storage device
    pub async fn write_blocks(
        &self,
        device_address: u8,
        start_block: u64,
        block_count: u32,
        buffer: &[u8],
    ) -> Result<usize> {
        // Implement SCSI WRITE(10) command
        // This is simplified
        Ok(buffer.len())
    }
}

impl UsbClassDriver for MassStorageDriver {
    fn class_type(&self) -> ClassType {
        ClassType::MassStorage
    }

    fn supports_device(&self, device: &UsbDevice) -> bool {
        device.device_class() == DeviceClass::MassStorage ||
        device.supports_class(DeviceClass::MassStorage)
    }

    fn attach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            // Initialize mass storage device
            let mut attached = attached_devices.lock();
            attached.push(device_address);

            // Send INQUIRY command to get device info
            // This is simplified

            Ok(())
        })
    }

    fn detach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.retain(|&addr| addr != device_address);
            Ok(())
        })
    }

    fn handle_event(&self, _device_address: u8, event: ClassEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            match event {
                ClassEvent::ConfigurationChanged { .. } => {
                    // Reconfigure endpoints
                },
                ClassEvent::EndpointError { endpoint, error } => {
                    log::error!("Mass storage endpoint {} error: {:?}", endpoint, error);
                },
                _ => {},
            }
            Ok(())
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Human Interface Device (HID) Driver
#[derive(Debug)]
pub struct HidDriver {
    name: String,
    attached_devices: Arc<Mutex<Vec<u8>>>,
}

impl HidDriver {
    /// Create a new HID driver
    pub fn new() -> Self {
        Self {
            name: "USB HID Driver".to_string(),
            attached_devices: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get HID report
    pub async fn get_report(
        &self,
        device_address: u8,
        report_type: u8,
        report_id: u8,
        buffer: &mut [u8],
    ) -> Result<usize> {
        // Implement GET_REPORT request
        // This is simplified
        Ok(0)
    }

    /// Set HID report
    pub async fn set_report(
        &self,
        device_address: u8,
        report_type: u8,
        report_id: u8,
        buffer: &[u8],
    ) -> Result<()> {
        // Implement SET_REPORT request
        // This is simplified
        Ok(())
    }
}

impl UsbClassDriver for HidDriver {
    fn class_type(&self) -> ClassType {
        ClassType::Hid
    }

    fn supports_device(&self, device: &UsbDevice) -> bool {
        device.device_class() == DeviceClass::Hid ||
        device.supports_class(DeviceClass::Hid)
    }

    fn attach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.push(device_address);

            // Get HID descriptor and report descriptor
            // This is simplified

            Ok(())
        })
    }

    fn detach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.retain(|&addr| addr != device_address);
            Ok(())
        })
    }

    fn handle_event(&self, _device_address: u8, event: ClassEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            match event {
                ClassEvent::TransferCompleted { endpoint, bytes } => {
                    // Process HID input report
                    log::debug!("HID input report received on endpoint {}: {} bytes", endpoint, bytes);
                },
                _ => {},
            }
            Ok(())
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Communications Device Class (CDC) Driver
#[derive(Debug)]
pub struct CdcDriver {
    name: String,
    attached_devices: Arc<Mutex<Vec<u8>>>,
}

impl CdcDriver {
    /// Create a new CDC driver
    pub fn new() -> Self {
        Self {
            name: "USB CDC Driver".to_string(),
            attached_devices: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Send data over CDC interface
    pub async fn send_data(&self, device_address: u8, data: &[u8]) -> Result<usize> {
        // Send data over bulk OUT endpoint
        // This is simplified
        Ok(data.len())
    }

    /// Receive data from CDC interface
    pub async fn receive_data(&self, device_address: u8, buffer: &mut [u8]) -> Result<usize> {
        // Receive data from bulk IN endpoint
        // This is simplified
        Ok(0)
    }

    /// Set line coding
    pub async fn set_line_coding(
        &self,
        device_address: u8,
        _baud_rate: u32,
        _data_bits: u8,
        _parity: u8,
        _stop_bits: u8,
    ) -> Result<()> {
        // Implement SET_LINE_CODING request
        // This is simplified
        Ok(())
    }
}

impl UsbClassDriver for CdcDriver {
    fn class_type(&self) -> ClassType {
        ClassType::CommAndCdcControl
    }

    fn supports_device(&self, device: &UsbDevice) -> bool {
        device.device_class() == DeviceClass::CommAndCdcControl ||
        device.supports_class(DeviceClass::CommAndCdcControl)
    }

    fn attach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.push(device_address);

            // Set default line coding
            // This is simplified

            Ok(())
        })
    }

    fn detach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.retain(|&addr| addr != device_address);
            Ok(())
        })
    }

    fn handle_event(&self, _device_address: u8, event: ClassEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            match event {
                ClassEvent::TransferCompleted { endpoint, bytes } => {
                    log::debug!("CDC data received on endpoint {}: {} bytes", endpoint, bytes);
                },
                _ => {},
            }
            Ok(())
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Hub Class Driver
#[derive(Debug)]
pub struct HubDriver {
    name: String,
    attached_devices: Arc<Mutex<Vec<u8>>>,
}

impl HubDriver {
    /// Create a new hub driver
    pub fn new() -> Self {
        Self {
            name: "USB Hub Driver".to_string(),
            attached_devices: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl UsbClassDriver for HubDriver {
    fn class_type(&self) -> ClassType {
        ClassType::Hub
    }

    fn supports_device(&self, device: &UsbDevice) -> bool {
        device.device_class() == DeviceClass::Hub ||
        device.supports_class(DeviceClass::Hub)
    }

    fn attach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.push(device_address);

            // Initialize hub (get hub descriptor, power ports, etc.)
            // This is simplified

            Ok(())
        })
    }

    fn detach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.retain(|&addr| addr != device_address);
            Ok(())
        })
    }

    fn handle_event(&self, _device_address: u8, event: ClassEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            match event {
                ClassEvent::TransferCompleted { endpoint, .. } => {
                    // Process hub status change
                    log::debug!("Hub status change on endpoint {}", endpoint);
                },
                _ => {},
            }
            Ok(())
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Generic/Vendor-specific Class Driver
#[derive(Debug)]
pub struct GenericDriver {
    name: String,
    class_type: ClassType,
    attached_devices: Arc<Mutex<Vec<u8>>>,
}

impl GenericDriver {
    /// Create a new generic driver
    pub fn new(name: String, class_type: ClassType) -> Self {
        Self {
            name,
            class_type,
            attached_devices: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl UsbClassDriver for GenericDriver {
    fn class_type(&self) -> ClassType {
        self.class_type
    }

    fn supports_device(&self, device: &UsbDevice) -> bool {
        let device_class: u8 = self.class_type.into();
        device.device_class() as u8 == device_class ||
        device.supports_class(device.device_class())
    }

    fn attach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.push(device_address);
            Ok(())
        })
    }

    fn detach_device(&self, device_address: u8) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attached_devices = self.attached_devices.clone();
        Box::pin(async move {
            let mut attached = attached_devices.lock();
            attached.retain(|&addr| addr != device_address);
            Ok(())
        })
    }

    fn handle_event(&self, _device_address: u8, _event: ClassEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // Generic handling
            Ok(())
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Class Driver Registry
pub struct ClassDriverRegistry {
    drivers: Vec<Box<dyn UsbClassDriver>>,
}

impl ClassDriverRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            drivers: Vec::new(),
        }
    }

    /// Register a class driver
    pub fn register(&mut self, driver: Box<dyn UsbClassDriver>) {
        self.drivers.push(driver);
    }

    /// Find a driver for a device
    pub fn find_driver(&self, device: &UsbDevice) -> Option<&dyn UsbClassDriver> {
        self.drivers.iter()
            .find(|driver| driver.supports_device(device))
            .map(|driver| driver.as_ref())
    }

    /// Get all drivers
    pub fn drivers(&self) -> &[Box<dyn UsbClassDriver>] {
        &self.drivers
    }

    /// Create default registry with common drivers
    pub fn with_default_drivers() -> Self {
        let mut registry = Self::new();

        registry.register(Box::new(MassStorageDriver::new()));
        registry.register(Box::new(HidDriver::new()));
        registry.register(Box::new(CdcDriver::new()));
        registry.register(Box::new(HubDriver::new()));

        registry
    }
}

impl fmt::Debug for ClassDriverRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassDriverRegistry")
            .field("driver_count", &self.drivers.len())
            .finish()
    }
}