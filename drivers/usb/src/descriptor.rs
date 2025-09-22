//! USB Descriptor Parsing and Management

use alloc::{vec, vec::Vec, string::String, boxed::Box};
use core::fmt;
use crate::{Result, UsbDriverError, endpoint::{Endpoint, EndpointType, EndpointDirection}};

/// USB Descriptor trait
pub trait UsbDescriptor: fmt::Debug + Send + Sync {
    /// Get descriptor type
    fn descriptor_type(&self) -> u8;

    /// Get descriptor length
    fn length(&self) -> u8;

    /// Serialize descriptor to bytes
    fn to_bytes(&self) -> Vec<u8>;

    /// Parse descriptor from bytes - returns specific descriptor type
    fn from_bytes(data: &[u8]) -> Result<Box<dyn UsbDescriptor>>
    where
        Self: Sized;
}

/// USB Descriptor Types
pub mod descriptor_types {
    pub const DEVICE: u8 = 0x01;
    pub const CONFIGURATION: u8 = 0x02;
    pub const STRING: u8 = 0x03;
    pub const INTERFACE: u8 = 0x04;
    pub const ENDPOINT: u8 = 0x05;
    pub const DEVICE_QUALIFIER: u8 = 0x06;
    pub const OTHER_SPEED_CONFIGURATION: u8 = 0x07;
    pub const INTERFACE_POWER: u8 = 0x08;
    pub const OTG: u8 = 0x09;
    pub const DEBUG: u8 = 0x0A;
    pub const INTERFACE_ASSOCIATION: u8 = 0x0B;
    pub const BOS: u8 = 0x0F;
    pub const DEVICE_CAPABILITY: u8 = 0x10;
    pub const HUB: u8 = 0x29;
    pub const SUPERSPEED_HUB: u8 = 0x2A;
    pub const ENDPOINT_COMPANION: u8 = 0x30;
}

/// Standard Device Descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DeviceDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub bcd_usb: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub max_packet_size_0: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bcd_device: u16,
    pub manufacturer_string: u8,
    pub product_string: u8,
    pub serial_number_string: u8,
    pub num_configurations: u8,
}

impl DeviceDescriptor {
    pub const LENGTH: u8 = 18;

    pub fn new(
        bcd_usb: u16,
        device_class: u8,
        device_subclass: u8,
        device_protocol: u8,
        max_packet_size_0: u8,
        vendor_id: u16,
        product_id: u16,
        bcd_device: u16,
        manufacturer_string: u8,
        product_string: u8,
        serial_number_string: u8,
        num_configurations: u8,
    ) -> Self {
        Self {
            length: Self::LENGTH,
            descriptor_type: descriptor_types::DEVICE,
            bcd_usb,
            device_class,
            device_subclass,
            device_protocol,
            max_packet_size_0,
            vendor_id,
            product_id,
            bcd_device,
            manufacturer_string,
            product_string,
            serial_number_string,
            num_configurations,
        }
    }
}

impl UsbDescriptor for DeviceDescriptor {
    fn descriptor_type(&self) -> u8 {
        descriptor_types::DEVICE
    }

    fn length(&self) -> u8 {
        Self::LENGTH
    }

    fn to_bytes(&self) -> Vec<u8> {
        unsafe {
            let ptr = self as *const Self as *const u8;
            core::slice::from_raw_parts(ptr, Self::LENGTH as usize).to_vec()
        }
    }

    fn from_bytes(data: &[u8]) -> Result<Box<dyn UsbDescriptor>> {
        if data.len() < Self::LENGTH as usize {
            return Err(UsbDriverError::InvalidParameter);
        }

        let descriptor = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const Self)
        };

        if descriptor.length != Self::LENGTH || descriptor.descriptor_type != descriptor_types::DEVICE {
            return Err(UsbDriverError::InvalidParameter);
        }

        Ok(Box::new(descriptor))
    }

}

/// Configuration Descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ConfigurationDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub total_length: u16,
    pub num_interfaces: u8,
    pub configuration_value: u8,
    pub configuration_string: u8,
    pub attributes: u8,
    pub max_power: u8,
}

impl ConfigurationDescriptor {
    pub const LENGTH: u8 = 9;

    /// Self-powered device
    pub const SELF_POWERED: u8 = 0x40;
    /// Remote wakeup capable
    pub const REMOTE_WAKEUP: u8 = 0x20;
    /// Must be set
    pub const RESERVED: u8 = 0x80;

    pub fn new(
        total_length: u16,
        num_interfaces: u8,
        configuration_value: u8,
        configuration_string: u8,
        attributes: u8,
        max_power: u8,
    ) -> Self {
        Self {
            length: Self::LENGTH,
            descriptor_type: descriptor_types::CONFIGURATION,
            total_length,
            num_interfaces,
            configuration_value,
            configuration_string,
            attributes: attributes | Self::RESERVED,
            max_power,
        }
    }
}

impl UsbDescriptor for ConfigurationDescriptor {
    fn descriptor_type(&self) -> u8 {
        descriptor_types::CONFIGURATION
    }

    fn length(&self) -> u8 {
        Self::LENGTH
    }

    fn to_bytes(&self) -> Vec<u8> {
        unsafe {
            let ptr = self as *const Self as *const u8;
            core::slice::from_raw_parts(ptr, Self::LENGTH as usize).to_vec()
        }
    }

    fn from_bytes(data: &[u8]) -> Result<Box<dyn UsbDescriptor>> {
        if data.len() < Self::LENGTH as usize {
            return Err(UsbDriverError::InvalidParameter);
        }

        let descriptor = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const Self)
        };

        if descriptor.length != Self::LENGTH || descriptor.descriptor_type != descriptor_types::CONFIGURATION {
            return Err(UsbDriverError::InvalidParameter);
        }

        Ok(Box::new(descriptor))
    }
}

/// Interface Descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct InterfaceDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub num_endpoints: u8,
    pub interface_class: u8,
    pub interface_subclass: u8,
    pub interface_protocol: u8,
    pub interface_string: u8,
}

impl InterfaceDescriptor {
    pub const LENGTH: u8 = 9;

    pub fn new(
        interface_number: u8,
        alternate_setting: u8,
        num_endpoints: u8,
        interface_class: u8,
        interface_subclass: u8,
        interface_protocol: u8,
        interface_string: u8,
    ) -> Self {
        Self {
            length: Self::LENGTH,
            descriptor_type: descriptor_types::INTERFACE,
            interface_number,
            alternate_setting,
            num_endpoints,
            interface_class,
            interface_subclass,
            interface_protocol,
            interface_string,
        }
    }
}

impl UsbDescriptor for InterfaceDescriptor {
    fn descriptor_type(&self) -> u8 {
        descriptor_types::INTERFACE
    }

    fn length(&self) -> u8 {
        Self::LENGTH
    }

    fn to_bytes(&self) -> Vec<u8> {
        unsafe {
            let ptr = self as *const Self as *const u8;
            core::slice::from_raw_parts(ptr, Self::LENGTH as usize).to_vec()
        }
    }

    fn from_bytes(data: &[u8]) -> Result<Box<dyn UsbDescriptor>> {
        if data.len() < Self::LENGTH as usize {
            return Err(UsbDriverError::InvalidParameter);
        }

        let descriptor = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const Self)
        };

        if descriptor.length != Self::LENGTH || descriptor.descriptor_type != descriptor_types::INTERFACE {
            return Err(UsbDriverError::InvalidParameter);
        }

        Ok(Box::new(descriptor))
    }
}

/// Endpoint Descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct EndpointDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub endpoint_address: u8,
    pub attributes: u8,
    pub max_packet_size: u16,
    pub interval: u8,
}

impl EndpointDescriptor {
    pub const LENGTH: u8 = 7;

    /// Endpoint attributes
    pub const TRANSFER_TYPE_MASK: u8 = 0x03;
    pub const TRANSFER_TYPE_CONTROL: u8 = 0x00;
    pub const TRANSFER_TYPE_ISOCHRONOUS: u8 = 0x01;
    pub const TRANSFER_TYPE_BULK: u8 = 0x02;
    pub const TRANSFER_TYPE_INTERRUPT: u8 = 0x03;

    pub fn new(
        endpoint_address: u8,
        attributes: u8,
        max_packet_size: u16,
        interval: u8,
    ) -> Self {
        Self {
            length: Self::LENGTH,
            descriptor_type: descriptor_types::ENDPOINT,
            endpoint_address,
            attributes,
            max_packet_size,
            interval,
        }
    }

    /// Create from endpoint
    pub fn from_endpoint(endpoint: &Endpoint) -> Self {
        let attributes = match endpoint.endpoint_type() {
            EndpointType::Control => Self::TRANSFER_TYPE_CONTROL,
            EndpointType::Isochronous => Self::TRANSFER_TYPE_ISOCHRONOUS,
            EndpointType::Bulk => Self::TRANSFER_TYPE_BULK,
            EndpointType::Interrupt => Self::TRANSFER_TYPE_INTERRUPT,
        };

        Self::new(
            endpoint.address(),
            attributes,
            endpoint.max_packet_size(),
            endpoint.interval(),
        )
    }

    /// Convert to endpoint
    pub fn to_endpoint(&self) -> Result<Endpoint> {
        let number = self.endpoint_address & 0x0F;
        let direction = if self.endpoint_address & 0x80 != 0 {
            EndpointDirection::In
        } else {
            EndpointDirection::Out
        };

        let endpoint_type = match self.attributes & Self::TRANSFER_TYPE_MASK {
            Self::TRANSFER_TYPE_CONTROL => EndpointType::Control,
            Self::TRANSFER_TYPE_ISOCHRONOUS => EndpointType::Isochronous,
            Self::TRANSFER_TYPE_BULK => EndpointType::Bulk,
            Self::TRANSFER_TYPE_INTERRUPT => EndpointType::Interrupt,
            _ => return Err(UsbDriverError::InvalidParameter),
        };

        Ok(Endpoint::new(
            number,
            direction,
            endpoint_type,
            self.max_packet_size,
            self.interval,
        ))
    }
}

impl UsbDescriptor for EndpointDescriptor {
    fn descriptor_type(&self) -> u8 {
        descriptor_types::ENDPOINT
    }

    fn length(&self) -> u8 {
        Self::LENGTH
    }

    fn to_bytes(&self) -> Vec<u8> {
        unsafe {
            let ptr = self as *const Self as *const u8;
            core::slice::from_raw_parts(ptr, Self::LENGTH as usize).to_vec()
        }
    }

    fn from_bytes(data: &[u8]) -> Result<Box<dyn UsbDescriptor>> {
        if data.len() < Self::LENGTH as usize {
            return Err(UsbDriverError::InvalidParameter);
        }

        let descriptor = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const Self)
        };

        if descriptor.length != Self::LENGTH || descriptor.descriptor_type != descriptor_types::ENDPOINT {
            return Err(UsbDriverError::InvalidParameter);
        }

        Ok(Box::new(descriptor))
    }
}

/// String Descriptor
#[derive(Debug, Clone)]
pub struct StringDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub string: String,
}

impl StringDescriptor {
    pub fn new(string: String) -> Self {
        Self {
            length: 2 + (string.len() * 2) as u8, // UTF-16 encoding
            descriptor_type: descriptor_types::STRING,
            string,
        }
    }

    /// Create language ID descriptor (index 0)
    pub fn language_ids(lang_ids: &[u16]) -> Self {
        let mut descriptor = Self {
            length: 2 + (lang_ids.len() * 2) as u8,
            descriptor_type: descriptor_types::STRING,
            string: String::new(),
        };

        // Store language IDs in the string field as a hack
        // In practice, you'd want a separate type for this
        for &lang_id in lang_ids {
            descriptor.string.push(char::from(lang_id as u8));
            descriptor.string.push(char::from((lang_id >> 8) as u8));
        }

        descriptor
    }
}

impl UsbDescriptor for StringDescriptor {
    fn descriptor_type(&self) -> u8 {
        descriptor_types::STRING
    }

    fn length(&self) -> u8 {
        self.length
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.length, self.descriptor_type];

        // Convert string to UTF-16LE
        for ch in self.string.chars() {
            let utf16 = ch as u16;
            bytes.push(utf16 as u8);
            bytes.push((utf16 >> 8) as u8);
        }

        bytes
    }

    fn from_bytes(data: &[u8]) -> Result<Box<dyn UsbDescriptor>> {
        if data.len() < 2 {
            return Err(UsbDriverError::InvalidParameter);
        }

        let length = data[0];
        let descriptor_type = data[1];

        if descriptor_type != descriptor_types::STRING || data.len() < length as usize {
            return Err(UsbDriverError::InvalidParameter);
        }

        // Convert UTF-16LE to string
        let mut string = String::new();
        let string_data = &data[2..length as usize];

        for chunk in string_data.chunks(2) {
            if chunk.len() == 2 {
                let utf16_char = u16::from_le_bytes([chunk[0], chunk[1]]);
                if let Some(ch) = char::from_u32(utf16_char as u32) {
                    string.push(ch);
                }
            }
        }

        Ok(Box::new(StringDescriptor {
            length,
            descriptor_type,
            string,
        }))
    }
}

/// Descriptor Parser for parsing configuration descriptor hierarchies
pub struct DescriptorParser;

impl DescriptorParser {
    /// Parse a configuration descriptor and all its subordinate descriptors
    pub fn parse_configuration(data: &[u8]) -> Result<(ConfigurationDescriptor, Vec<Box<dyn UsbDescriptor>>)> {
        if data.len() < ConfigurationDescriptor::LENGTH as usize {
            return Err(UsbDriverError::InvalidParameter);
        }

        // Parse configuration descriptor directly
        let config_desc = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ConfigurationDescriptor)
        };

        if config_desc.length != ConfigurationDescriptor::LENGTH ||
           config_desc.descriptor_type != descriptor_types::CONFIGURATION {
            return Err(UsbDriverError::InvalidParameter);
        }

        let mut descriptors = Vec::new();
        let mut offset = ConfigurationDescriptor::LENGTH as usize;

        while offset < data.len() && offset < config_desc.total_length as usize {
            if offset + 2 > data.len() {
                break;
            }

            let desc_len = data[offset];
            let desc_type = data[offset + 1];

            if desc_len == 0 || offset + desc_len as usize > data.len() {
                break;
            }

            let desc_data = &data[offset..offset + desc_len as usize];

            let descriptor = match desc_type {
                descriptor_types::INTERFACE => InterfaceDescriptor::from_bytes(desc_data)?,
                descriptor_types::ENDPOINT => EndpointDescriptor::from_bytes(desc_data)?,
                _ => {
                    // Skip unknown descriptor types
                    offset += desc_len as usize;
                    continue;
                }
            };

            descriptors.push(descriptor);
            offset += desc_len as usize;
        }

        Ok((config_desc, descriptors))
    }

    /// Parse device descriptor
    pub fn parse_device(data: &[u8]) -> Result<DeviceDescriptor> {
        if data.len() < DeviceDescriptor::LENGTH as usize {
            return Err(UsbDriverError::InvalidParameter);
        }

        let descriptor = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const DeviceDescriptor)
        };

        if descriptor.length != DeviceDescriptor::LENGTH ||
           descriptor.descriptor_type != descriptor_types::DEVICE {
            return Err(UsbDriverError::InvalidParameter);
        }

        Ok(descriptor)
    }

    /// Parse string descriptor
    pub fn parse_string(data: &[u8]) -> Result<StringDescriptor> {
        if data.len() < 2 {
            return Err(UsbDriverError::InvalidParameter);
        }

        let length = data[0];
        let descriptor_type = data[1];

        if descriptor_type != descriptor_types::STRING || data.len() < length as usize {
            return Err(UsbDriverError::InvalidParameter);
        }

        // Convert UTF-16LE to string
        let mut string = String::new();
        let string_data = &data[2..length as usize];

        for chunk in string_data.chunks(2) {
            if chunk.len() == 2 {
                let utf16_char = u16::from_le_bytes([chunk[0], chunk[1]]);
                if let Some(ch) = char::from_u32(utf16_char as u32) {
                    string.push(ch);
                }
            }
        }

        Ok(StringDescriptor {
            length,
            descriptor_type,
            string,
        })
    }
}

/// USB Language IDs
pub mod language_ids {
    pub const ENGLISH_US: u16 = 0x0409;
    pub const ENGLISH_UK: u16 = 0x0809;
    pub const GERMAN: u16 = 0x0407;
    pub const FRENCH: u16 = 0x040C;
    pub const SPANISH: u16 = 0x0C0A;
    pub const JAPANESE: u16 = 0x0411;
    pub const CHINESE_SIMPLIFIED: u16 = 0x0804;
    pub const CHINESE_TRADITIONAL: u16 = 0x0404;
}

/// Common USB Class Codes
pub mod class_codes {
    pub const PER_INTERFACE: u8 = 0x00;
    pub const AUDIO: u8 = 0x01;
    pub const COMM_AND_CDC_CONTROL: u8 = 0x02;
    pub const HID: u8 = 0x03;
    pub const PHYSICAL: u8 = 0x05;
    pub const IMAGE: u8 = 0x06;
    pub const PRINTER: u8 = 0x07;
    pub const MASS_STORAGE: u8 = 0x08;
    pub const HUB: u8 = 0x09;
    pub const CDC_DATA: u8 = 0x0A;
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