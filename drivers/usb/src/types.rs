/// USB device speed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    /// Low speed (1.5 Mbit/s)
    Low,
    /// Full speed (12 Mbit/s)
    Full,
    /// High speed (480 Mbit/s)
    High,
    /// Super speed (5 Gbit/s)
    Super,
    /// Super speed+ (10 Gbit/s)
    SuperPlus,
}

impl UsbSpeed {
    /// Get the maximum packet size for control endpoints
    pub fn max_packet_size_control(self) -> u16 {
        match self {
            UsbSpeed::Low => 8,
            UsbSpeed::Full => 64,
            UsbSpeed::High => 64,
            UsbSpeed::Super => 512,
            UsbSpeed::SuperPlus => 512,
        }
    }

    /// Get the speed value for xHCI slot context
    pub fn to_xhci_speed(self) -> u8 {
        match self {
            UsbSpeed::Low => 2,
            UsbSpeed::Full => 1,
            UsbSpeed::High => 3,
            UsbSpeed::Super => 4,
            UsbSpeed::SuperPlus => 5,
        }
    }
}

/// USB endpoint direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbDirection {
    /// Data flows from host to device
    Out = 0,
    /// Data flows from device to host
    In = 1,
}

/// USB endpoint type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbEndpointType {
    /// Control endpoint
    Control = 0,
    /// Isochronous endpoint
    Isochronous = 1,
    /// Bulk endpoint
    Bulk = 2,
    /// Interrupt endpoint
    Interrupt = 3,
}

/// USB transfer type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbTransferType {
    /// Control transfer
    Control,
    /// Bulk transfer
    Bulk,
    /// Interrupt transfer
    Interrupt,
    /// Isochronous transfer
    Isochronous,
}

/// USB device state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbDeviceState {
    /// Device is attached but not powered
    Attached,
    /// Device is powered but not reset
    Powered,
    /// Device is reset and has default address
    Default,
    /// Device has been assigned an address
    Addressed,
    /// Device is configured and ready for use
    Configured,
    /// Device is suspended
    Suspended,
}

/// USB endpoint information
#[derive(Debug, Clone, Copy)]
pub struct UsbEndpoint {
    /// Endpoint number (0-15)
    pub number: u8,
    /// Transfer direction
    pub direction: UsbDirection,
    /// Endpoint type
    pub endpoint_type: UsbEndpointType,
    /// Maximum packet size
    pub max_packet_size: u16,
    /// Polling interval for interrupt/isochronous endpoints
    pub interval: u8,
}

impl UsbEndpoint {
    /// Create a new control endpoint
    pub fn control(max_packet_size: u16) -> Self {
        Self {
            number: 0,
            direction: UsbDirection::Out, // Control endpoints are bidirectional
            endpoint_type: UsbEndpointType::Control,
            max_packet_size,
            interval: 0,
        }
    }

    /// Create a new bulk endpoint
    pub fn bulk(number: u8, direction: UsbDirection, max_packet_size: u16) -> Self {
        Self {
            number,
            direction,
            endpoint_type: UsbEndpointType::Bulk,
            max_packet_size,
            interval: 0,
        }
    }

    /// Create a new interrupt endpoint
    pub fn interrupt(number: u8, direction: UsbDirection, max_packet_size: u16, interval: u8) -> Self {
        Self {
            number,
            direction,
            endpoint_type: UsbEndpointType::Interrupt,
            max_packet_size,
            interval,
        }
    }

    /// Get the endpoint address (combines number and direction)
    pub fn address(self) -> u8 {
        self.number | if matches!(self.direction, UsbDirection::In) { 0x80 } else { 0 }
    }

    /// Get the xHCI endpoint index (used for context addressing)
    pub fn xhci_index(self) -> u8 {
        if self.number == 0 {
            1 // Control endpoint is always index 1
        } else {
            (self.number * 2) + match self.direction {
                UsbDirection::Out => 0,
                UsbDirection::In => 1,
            }
        }
    }
}

/// USB device information
#[derive(Debug, Clone)]
pub struct UsbDevice {
    /// Device address (1-127)
    pub address: u8,
    /// Port number the device is connected to
    pub port: u8,
    /// Device speed
    pub speed: UsbSpeed,
    /// Current device state
    pub state: UsbDeviceState,
    /// Hub device this device is connected to (None for root hub)
    pub hub_address: Option<u8>,
    /// Hub port this device is connected to
    pub hub_port: Option<u8>,
    /// Device descriptor
    pub device_descriptor: Option<DeviceDescriptor>,
    /// Configuration descriptors
    pub config_descriptors: heapless::Vec<ConfigDescriptor, 8>,
    /// Active configuration number
    pub active_config: Option<u8>,
    /// Available endpoints
    pub endpoints: heapless::Vec<UsbEndpoint, 32>,
}

impl UsbDevice {
    /// Create a new USB device
    pub fn new(address: u8, port: u8, speed: UsbSpeed) -> Self {
        let mut device = Self {
            address,
            port,
            speed,
            state: UsbDeviceState::Default,
            hub_address: None,
            hub_port: None,
            device_descriptor: None,
            config_descriptors: heapless::Vec::new(),
            active_config: None,
            endpoints: heapless::Vec::new(),
        };

        // Add default control endpoint
        let _ = device.endpoints.push(UsbEndpoint::control(speed.max_packet_size_control()));
        device
    }

    /// Get the control endpoint
    pub fn control_endpoint(&self) -> &UsbEndpoint {
        &self.endpoints[0] // Control endpoint is always first
    }

    /// Find an endpoint by address
    pub fn find_endpoint(&self, address: u8) -> Option<&UsbEndpoint> {
        self.endpoints.iter().find(|ep| ep.address() == address)
    }

    /// Add an endpoint to the device
    pub fn add_endpoint(&mut self, endpoint: UsbEndpoint) -> Result<(), ()> {
        self.endpoints.push(endpoint)
    }
}

/// Standard USB device descriptor
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DeviceDescriptor {
    /// Size of this descriptor in bytes
    pub length: u8,
    /// Descriptor type (DEVICE = 1)
    pub descriptor_type: u8,
    /// USB specification version (BCD)
    pub usb_version: u16,
    /// Device class code
    pub device_class: u8,
    /// Device subclass code
    pub device_subclass: u8,
    /// Device protocol code
    pub device_protocol: u8,
    /// Maximum packet size for endpoint 0
    pub max_packet_size0: u8,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Device release number (BCD)
    pub device_version: u16,
    /// Index of manufacturer string descriptor
    pub manufacturer_index: u8,
    /// Index of product string descriptor
    pub product_index: u8,
    /// Index of serial number string descriptor
    pub serial_index: u8,
    /// Number of possible configurations
    pub num_configurations: u8,
}

/// Standard USB configuration descriptor
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ConfigDescriptor {
    /// Size of this descriptor in bytes
    pub length: u8,
    /// Descriptor type (CONFIGURATION = 2)
    pub descriptor_type: u8,
    /// Total length of configuration data
    pub total_length: u16,
    /// Number of interfaces in this configuration
    pub num_interfaces: u8,
    /// Configuration value
    pub configuration_value: u8,
    /// Index of configuration string descriptor
    pub configuration_index: u8,
    /// Configuration characteristics
    pub attributes: u8,
    /// Maximum power consumption (in 2mA units)
    pub max_power: u8,
}

/// Standard USB interface descriptor
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct InterfaceDescriptor {
    /// Size of this descriptor in bytes
    pub length: u8,
    /// Descriptor type (INTERFACE = 4)
    pub descriptor_type: u8,
    /// Interface number
    pub interface_number: u8,
    /// Alternate setting number
    pub alternate_setting: u8,
    /// Number of endpoints in this interface
    pub num_endpoints: u8,
    /// Interface class code
    pub interface_class: u8,
    /// Interface subclass code
    pub interface_subclass: u8,
    /// Interface protocol code
    pub interface_protocol: u8,
    /// Index of interface string descriptor
    pub interface_index: u8,
}

/// Standard USB endpoint descriptor
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct EndpointDescriptor {
    /// Size of this descriptor in bytes
    pub length: u8,
    /// Descriptor type (ENDPOINT = 5)
    pub descriptor_type: u8,
    /// Endpoint address (includes direction bit)
    pub endpoint_address: u8,
    /// Endpoint attributes (transfer type)
    pub attributes: u8,
    /// Maximum packet size
    pub max_packet_size: u16,
    /// Polling interval
    pub interval: u8,
}

impl EndpointDescriptor {
    /// Get the endpoint number
    pub fn endpoint_number(&self) -> u8 {
        self.endpoint_address & 0x0f
    }

    /// Get the endpoint direction
    pub fn direction(&self) -> UsbDirection {
        if (self.endpoint_address & 0x80) != 0 {
            UsbDirection::In
        } else {
            UsbDirection::Out
        }
    }

    /// Get the transfer type
    pub fn transfer_type(&self) -> UsbEndpointType {
        match self.attributes & 0x03 {
            0 => UsbEndpointType::Control,
            1 => UsbEndpointType::Isochronous,
            2 => UsbEndpointType::Bulk,
            3 => UsbEndpointType::Interrupt,
            _ => unreachable!(),
        }
    }

    /// Convert to UsbEndpoint
    pub fn to_usb_endpoint(&self) -> UsbEndpoint {
        UsbEndpoint {
            number: self.endpoint_number(),
            direction: self.direction(),
            endpoint_type: self.transfer_type(),
            max_packet_size: self.max_packet_size,
            interval: self.interval,
        }
    }
}

/// USB setup packet for control transfers
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct SetupPacket {
    /// Request type and direction
    pub request_type: u8,
    /// Specific request
    pub request: u8,
    /// Request-specific value
    pub value: u16,
    /// Request-specific index
    pub index: u16,
    /// Length of data phase
    pub length: u16,
}

impl SetupPacket {
    /// Create a GET_DESCRIPTOR request
    pub fn get_descriptor(descriptor_type: u8, descriptor_index: u8, length: u16) -> Self {
        Self {
            request_type: 0x80, // Device to host, standard, device
            request: 0x06,      // GET_DESCRIPTOR
            value: ((descriptor_type as u16) << 8) | (descriptor_index as u16),
            index: 0,
            length,
        }
    }

    /// Create a SET_ADDRESS request
    pub fn set_address(address: u8) -> Self {
        Self {
            request_type: 0x00, // Host to device, standard, device
            request: 0x05,      // SET_ADDRESS
            value: address as u16,
            index: 0,
            length: 0,
        }
    }

    /// Create a SET_CONFIGURATION request
    pub fn set_configuration(config_value: u8) -> Self {
        Self {
            request_type: 0x00, // Host to device, standard, device
            request: 0x09,      // SET_CONFIGURATION
            value: config_value as u16,
            index: 0,
            length: 0,
        }
    }

    /// Create a GET_STATUS request
    pub fn get_status() -> Self {
        Self {
            request_type: 0x80, // Device to host, standard, device
            request: 0x00,      // GET_STATUS
            value: 0,
            index: 0,
            length: 2,
        }
    }
}

/// USB descriptor types
#[allow(dead_code)]
pub mod descriptor_types {
    pub const DEVICE: u8 = 1;
    pub const CONFIGURATION: u8 = 2;
    pub const STRING: u8 = 3;
    pub const INTERFACE: u8 = 4;
    pub const ENDPOINT: u8 = 5;
    pub const DEVICE_QUALIFIER: u8 = 6;
    pub const OTHER_SPEED_CONFIGURATION: u8 = 7;
    pub const INTERFACE_POWER: u8 = 8;
    pub const HUB: u8 = 41;
}

/// USB class codes
#[allow(dead_code)]
pub mod class_codes {
    pub const PER_INTERFACE: u8 = 0x00;
    pub const AUDIO: u8 = 0x01;
    pub const COMMUNICATIONS: u8 = 0x02;
    pub const HID: u8 = 0x03;
    pub const PHYSICAL: u8 = 0x05;
    pub const IMAGE: u8 = 0x06;
    pub const PRINTER: u8 = 0x07;
    pub const MASS_STORAGE: u8 = 0x08;
    pub const HUB: u8 = 0x09;
    pub const DATA: u8 = 0x0A;
    pub const SMART_CARD: u8 = 0x0B;
    pub const CONTENT_SECURITY: u8 = 0x0D;
    pub const VIDEO: u8 = 0x0E;
    pub const PERSONAL_HEALTHCARE: u8 = 0x0F;
    pub const AUDIO_VIDEO: u8 = 0x10;
    pub const BILLBOARD: u8 = 0x11;
    pub const DIAGNOSTIC: u8 = 0xDC;
    pub const WIRELESS: u8 = 0xE0;
    pub const MISCELLANEOUS: u8 = 0xEF;
    pub const APPLICATION_SPECIFIC: u8 = 0xFE;
    pub const VENDOR_SPECIFIC: u8 = 0xFF;
}