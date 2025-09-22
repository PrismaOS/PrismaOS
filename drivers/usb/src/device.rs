//! USB Device Management

use alloc::{vec::Vec, string::String, boxed::Box};
use core::{
    fmt,
    sync::atomic::{AtomicU8, Ordering},
};
use crate::{Result, UsbDriverError, endpoint::Endpoint, descriptor::UsbDescriptor};

/// USB Device State
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Attached = 0,
    Powered = 1,
    Default = 2,
    Address = 3,
    Configured = 4,
    Suspended = 5,
    Disconnected = 6,
}

impl From<u8> for DeviceState {
    fn from(value: u8) -> Self {
        match value {
            0 => DeviceState::Attached,
            1 => DeviceState::Powered,
            2 => DeviceState::Default,
            3 => DeviceState::Address,
            4 => DeviceState::Configured,
            5 => DeviceState::Suspended,
            _ => DeviceState::Disconnected,
        }
    }
}

/// USB Device Class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// Use class information in the Interface Descriptors
    PerInterface = 0x00,
    /// Audio
    Audio = 0x01,
    /// Communications and CDC Control
    CommAndCdcControl = 0x02,
    /// Human Interface Device
    Hid = 0x03,
    /// Physical
    Physical = 0x05,
    /// Image
    Image = 0x06,
    /// Printer
    Printer = 0x07,
    /// Mass Storage
    MassStorage = 0x08,
    /// Hub
    Hub = 0x09,
    /// CDC Data
    CdcData = 0x0A,
    /// Smart Card
    SmartCard = 0x0B,
    /// Content Security
    ContentSecurity = 0x0D,
    /// Video
    Video = 0x0E,
    /// Personal Healthcare
    PersonalHealthcare = 0x0F,
    /// Audio/Video Devices
    AudioVideo = 0x10,
    /// Billboard Device
    Billboard = 0x11,
    /// USB Type-C Bridge
    TypeCBridge = 0x12,
    /// Diagnostic Device
    Diagnostic = 0xDC,
    /// Wireless Controller
    Wireless = 0xE0,
    /// Miscellaneous
    Miscellaneous = 0xEF,
    /// Application Specific
    ApplicationSpecific = 0xFE,
    /// Vendor Specific
    VendorSpecific = 0xFF,
}

impl From<u8> for DeviceClass {
    fn from(value: u8) -> Self {
        match value {
            0x00 => DeviceClass::PerInterface,
            0x01 => DeviceClass::Audio,
            0x02 => DeviceClass::CommAndCdcControl,
            0x03 => DeviceClass::Hid,
            0x05 => DeviceClass::Physical,
            0x06 => DeviceClass::Image,
            0x07 => DeviceClass::Printer,
            0x08 => DeviceClass::MassStorage,
            0x09 => DeviceClass::Hub,
            0x0A => DeviceClass::CdcData,
            0x0B => DeviceClass::SmartCard,
            0x0D => DeviceClass::ContentSecurity,
            0x0E => DeviceClass::Video,
            0x0F => DeviceClass::PersonalHealthcare,
            0x10 => DeviceClass::AudioVideo,
            0x11 => DeviceClass::Billboard,
            0x12 => DeviceClass::TypeCBridge,
            0xDC => DeviceClass::Diagnostic,
            0xE0 => DeviceClass::Wireless,
            0xEF => DeviceClass::Miscellaneous,
            0xFE => DeviceClass::ApplicationSpecific,
            _ => DeviceClass::VendorSpecific,
        }
    }
}

/// USB Device Speed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceSpeed {
    Low,    // 1.5 Mbps
    Full,   // 12 Mbps
    High,   // 480 Mbps
    Super,  // 5 Gbps
    SuperPlus, // 10+ Gbps
}

/// USB Configuration
#[derive(Debug)]
pub struct UsbConfiguration {
    pub value: u8,
    pub string_index: u8,
    pub attributes: u8,
    pub max_power: u8,
    pub interfaces: Vec<UsbInterface>,
}

/// USB Interface
#[derive(Debug)]
pub struct UsbInterface {
    pub number: u8,
    pub alternate: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub string_index: u8,
    pub endpoints: Vec<Endpoint>,
}

/// USB Device
pub struct UsbDevice {
    /// Device address (assigned by host)
    address: AtomicU8,
    /// Device state
    state: AtomicU8,
    /// Device speed
    speed: DeviceSpeed,
    /// Port number on hub
    port: u8,
    /// Hub address (0 for root hub)
    hub_address: u8,
    /// Device descriptor
    device_descriptor: Box<dyn UsbDescriptor>,
    /// Configuration descriptors
    configurations: Vec<UsbConfiguration>,
    /// Current configuration
    current_configuration: AtomicU8,
    /// String descriptors
    string_descriptors: Vec<String>,
    /// Vendor ID
    vendor_id: u16,
    /// Product ID
    product_id: u16,
    /// Device release number
    device_release: u16,
    /// Device class
    device_class: DeviceClass,
    /// Device subclass
    device_subclass: u8,
    /// Device protocol
    device_protocol: u8,
    /// Maximum packet size for endpoint 0
    max_packet_size_0: u16,
}

impl UsbDevice {
    /// Create a new USB device
    pub fn new(
        speed: DeviceSpeed,
        port: u8,
        hub_address: u8,
        device_descriptor: Box<dyn UsbDescriptor>,
    ) -> Self {
        Self {
            address: AtomicU8::new(0),
            state: AtomicU8::new(DeviceState::Attached as u8),
            speed,
            port,
            hub_address,
            device_descriptor,
            configurations: Vec::new(),
            current_configuration: AtomicU8::new(0),
            string_descriptors: Vec::new(),
            vendor_id: 0,
            product_id: 0,
            device_release: 0,
            device_class: DeviceClass::PerInterface,
            device_subclass: 0,
            device_protocol: 0,
            max_packet_size_0: 8,
        }
    }

    /// Get device address
    pub fn address(&self) -> u8 {
        self.address.load(Ordering::Acquire)
    }

    /// Set device address
    pub fn set_address(&self, address: u8) {
        self.address.store(address, Ordering::Release);
        if address > 0 {
            self.set_state(DeviceState::Address);
        }
    }

    /// Get device state
    pub fn state(&self) -> DeviceState {
        DeviceState::from(self.state.load(Ordering::Acquire))
    }

    /// Set device state
    pub fn set_state(&self, state: DeviceState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Get device speed
    pub fn speed(&self) -> DeviceSpeed {
        self.speed
    }

    /// Get port number
    pub fn port(&self) -> u8 {
        self.port
    }

    /// Get hub address
    pub fn hub_address(&self) -> u8 {
        self.hub_address
    }

    /// Get device class
    pub fn device_class(&self) -> DeviceClass {
        self.device_class
    }

    /// Get vendor ID
    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    /// Get product ID
    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    /// Get device release
    pub fn device_release(&self) -> u16 {
        self.device_release
    }

    /// Get maximum packet size for endpoint 0
    pub fn max_packet_size_0(&self) -> u16 {
        self.max_packet_size_0
    }

    /// Get current configuration
    pub fn current_configuration(&self) -> u8 {
        self.current_configuration.load(Ordering::Acquire)
    }

    /// Set current configuration
    pub fn set_configuration(&self, config: u8) -> Result<()> {
        if config == 0 || self.configurations.iter().any(|c| c.value == config) {
            self.current_configuration.store(config, Ordering::Release);
            if config > 0 {
                self.set_state(DeviceState::Configured);
            } else {
                self.set_state(DeviceState::Address);
            }
            Ok(())
        } else {
            Err(UsbDriverError::InvalidConfiguration)
        }
    }

    /// Get configurations
    pub fn configurations(&self) -> &[UsbConfiguration] {
        &self.configurations
    }

    /// Add configuration
    pub fn add_configuration(&mut self, config: UsbConfiguration) {
        self.configurations.push(config);
    }

    /// Get string descriptor
    pub fn get_string(&self, index: u8) -> Option<&str> {
        if index == 0 || index as usize > self.string_descriptors.len() {
            None
        } else {
            Some(&self.string_descriptors[index as usize - 1])
        }
    }

    /// Add string descriptor
    pub fn add_string(&mut self, string: String) -> u8 {
        self.string_descriptors.push(string);
        self.string_descriptors.len() as u8
    }

    /// Configure the device (called during enumeration)
    pub async fn configure(&mut self) -> Result<()> {
        // This would typically involve:
        // 1. Reading device descriptor
        // 2. Assigning device address
        // 3. Reading configuration descriptors
        // 4. Setting configuration
        // For now, this is a placeholder

        self.set_state(DeviceState::Default);
        Ok(())
    }

    /// Reset the device
    pub async fn reset(&mut self) -> Result<()> {
        self.address.store(0, Ordering::Release);
        self.current_configuration.store(0, Ordering::Release);
        self.set_state(DeviceState::Default);
        Ok(())
    }

    /// Suspend the device
    pub async fn suspend(&mut self) -> Result<()> {
        self.set_state(DeviceState::Suspended);
        Ok(())
    }

    /// Resume the device
    pub async fn resume(&mut self) -> Result<()> {
        // Return to previous state (typically Configured)
        if self.current_configuration() > 0 {
            self.set_state(DeviceState::Configured);
        } else {
            self.set_state(DeviceState::Address);
        }
        Ok(())
    }

    /// Check if device supports a specific class
    pub fn supports_class(&self, class: DeviceClass) -> bool {
        self.device_class == class ||
        self.configurations.iter().any(|config| {
            config.interfaces.iter().any(|iface| iface.class == class as u8)
        })
    }

    /// Get interface by number
    pub fn get_interface(&self, interface_num: u8) -> Option<&UsbInterface> {
        if let Some(config) = self.configurations.iter()
            .find(|c| c.value == self.current_configuration()) {
            config.interfaces.iter().find(|i| i.number == interface_num)
        } else {
            None
        }
    }

    /// Get endpoint by address
    pub fn get_endpoint(&self, endpoint_addr: u8) -> Option<&Endpoint> {
        if let Some(config) = self.configurations.iter()
            .find(|c| c.value == self.current_configuration()) {
            for interface in &config.interfaces {
                if let Some(endpoint) = interface.endpoints.iter()
                    .find(|ep| ep.address() == endpoint_addr) {
                    return Some(endpoint);
                }
            }
        }
        None
    }
}

impl fmt::Debug for UsbDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsbDevice")
            .field("address", &self.address())
            .field("state", &self.state())
            .field("speed", &self.speed)
            .field("vendor_id", &format_args!("{:#06x}", self.vendor_id))
            .field("product_id", &format_args!("{:#06x}", self.product_id))
            .field("device_class", &self.device_class)
            .finish()
    }
}

unsafe impl Send for UsbDevice {}
unsafe impl Sync for UsbDevice {}