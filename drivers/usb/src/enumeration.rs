/// USB Device Enumeration and Management
///
/// This module implements the USB device enumeration process and provides
/// high-level device management functionality.

use crate::error::{UsbError, Result};
use crate::types::*;
use alloc::{vec::Vec, string::String};

/// USB Device Manager
pub struct UsbDeviceManager {
    /// List of enumerated devices
    devices: Vec<UsbDevice>,
    /// Next available device address
    next_address: u8,
    /// Device drivers
    device_drivers: Vec<Box<dyn UsbDeviceDriver>>,
}

impl UsbDeviceManager {
    /// Create a new device manager
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            next_address: 1,
            device_drivers: Vec::new(),
        }
    }

    /// Register a device driver
    pub fn register_driver(&mut self, driver: Box<dyn UsbDeviceDriver>) {
        self.device_drivers.push(driver);
    }

    /// Enumerate a new device
    pub fn enumerate_device(&mut self, port: u8, speed: UsbSpeed) -> Result<u8> {
        // Allocate device address
        let address = self.allocate_address()?;

        // Create device with default address first
        let mut device = UsbDevice::new(0, port, speed);

        // Perform enumeration steps
        self.get_device_descriptor(&mut device)?;
        self.set_device_address(&mut device, address)?;
        self.get_full_device_descriptor(&mut device)?;
        self.get_configuration_descriptors(&mut device)?;
        self.set_configuration(&mut device)?;

        // Update device address
        device.address = address;
        device.state = UsbDeviceState::Configured;

        // Find and bind device driver
        self.bind_device_driver(&mut device)?;

        // Add to device list
        self.devices.push(device);

        Ok(address)
    }

    /// Remove a device
    pub fn remove_device(&mut self, address: u8) -> Result<()> {
        if let Some(pos) = self.devices.iter().position(|d| d.address == address) {
            let device = self.devices.remove(pos);

            // Notify driver of device removal
            for driver in &mut self.device_drivers {
                driver.device_removed(&device);
            }

            // Free the address
            self.free_address(address);

            Ok(())
        } else {
            Err(UsbError::DeviceNotFound)
        }
    }

    /// Get device by address
    pub fn get_device(&self, address: u8) -> Option<&UsbDevice> {
        self.devices.iter().find(|d| d.address == address)
    }

    /// Get all devices
    pub fn get_devices(&self) -> &[UsbDevice] {
        &self.devices
    }

    /// Find devices by class
    pub fn find_devices_by_class(&self, class_code: u8) -> Vec<&UsbDevice> {
        self.devices
            .iter()
            .filter(|d| {
                if let Some(desc) = &d.device_descriptor {
                    desc.device_class == class_code
                } else {
                    false
                }
            })
            .collect()
    }

    /// Find devices by vendor/product ID
    pub fn find_devices_by_id(&self, vendor_id: u16, product_id: u16) -> Vec<&UsbDevice> {
        self.devices
            .iter()
            .filter(|d| {
                if let Some(desc) = &d.device_descriptor {
                    desc.vendor_id == vendor_id && desc.product_id == product_id
                } else {
                    false
                }
            })
            .collect()
    }

    /// Allocate a device address
    fn allocate_address(&mut self) -> Result<u8> {
        if self.next_address > 127 {
            // Find a free address
            for addr in 1..=127 {
                if !self.devices.iter().any(|d| d.address == addr) {
                    return Ok(addr);
                }
            }
            return Err(UsbError::OutOfMemory);
        }

        let address = self.next_address;
        self.next_address += 1;
        Ok(address)
    }

    /// Free a device address
    fn free_address(&mut self, _address: u8) {
        // In a more sophisticated implementation, we might track free addresses
        // For now, we just rely on the search in allocate_address
    }

    /// Get initial device descriptor (first 8 bytes)
    fn get_device_descriptor(&mut self, device: &mut UsbDevice) -> Result<()> {
        // This would normally involve sending a GET_DESCRIPTOR request
        // For now, we'll create a mock descriptor
        let descriptor = DeviceDescriptor {
            length: 18,
            descriptor_type: descriptor_types::DEVICE,
            usb_version: 0x0200, // USB 2.0
            device_class: class_codes::PER_INTERFACE,
            device_subclass: 0,
            device_protocol: 0,
            max_packet_size0: device.speed.max_packet_size_control() as u8,
            vendor_id: 0x1234,
            product_id: 0x5678,
            device_version: 0x0100,
            manufacturer_index: 1,
            product_index: 2,
            serial_index: 3,
            num_configurations: 1,
        };

        device.device_descriptor = Some(descriptor);
        Ok(())
    }

    /// Set device address
    fn set_device_address(&mut self, device: &mut UsbDevice, address: u8) -> Result<()> {
        // This would normally involve sending a SET_ADDRESS request
        device.state = UsbDeviceState::Addressed;
        Ok(())
    }

    /// Get full device descriptor
    fn get_full_device_descriptor(&mut self, device: &mut UsbDevice) -> Result<()> {
        // This would normally involve sending another GET_DESCRIPTOR request
        // The descriptor should already be populated from the initial request
        Ok(())
    }

    /// Get configuration descriptors
    fn get_configuration_descriptors(&mut self, device: &mut UsbDevice) -> Result<()> {
        // This would normally involve GET_DESCRIPTOR requests for each configuration
        let config_descriptor = ConfigDescriptor {
            length: 9,
            descriptor_type: descriptor_types::CONFIGURATION,
            total_length: 34, // Mock total length
            num_interfaces: 1,
            configuration_value: 1,
            configuration_index: 0,
            attributes: 0x80, // Bus powered
            max_power: 50,    // 100mA
        };

        let _ = device.config_descriptors.push(config_descriptor);
        Ok(())
    }

    /// Set device configuration
    fn set_configuration(&mut self, device: &mut UsbDevice) -> Result<()> {
        // This would normally involve sending a SET_CONFIGURATION request
        if let Some(config) = device.config_descriptors.first() {
            device.active_config = Some(config.configuration_value);
            device.state = UsbDeviceState::Configured;
        }
        Ok(())
    }

    /// Bind device driver
    fn bind_device_driver(&mut self, device: &mut UsbDevice) -> Result<()> {
        if let Some(desc) = &device.device_descriptor {
            for driver in &mut self.device_drivers {
                if driver.supports_device(desc) {
                    driver.bind_device(device)?;
                    break;
                }
            }
        }
        Ok(())
    }
}

impl Default for UsbDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// USB Device Driver trait
pub trait UsbDeviceDriver: Send + Sync {
    /// Get driver name
    fn name(&self) -> &'static str;

    /// Check if this driver supports the given device
    fn supports_device(&self, device_descriptor: &DeviceDescriptor) -> bool;

    /// Bind to a device
    fn bind_device(&mut self, device: &mut UsbDevice) -> Result<()>;

    /// Handle device removal
    fn device_removed(&mut self, device: &UsbDevice);

    /// Handle endpoint data
    fn handle_data(&mut self, device_address: u8, endpoint: u8, data: &[u8]) -> Result<()>;
}

/// HID (Human Interface Device) Driver
pub struct HidDriver {
    /// Bound devices
    bound_devices: Vec<u8>,
}

impl HidDriver {
    /// Create a new HID driver
    pub fn new() -> Self {
        Self {
            bound_devices: Vec::new(),
        }
    }

    /// Process HID report
    fn process_hid_report(&mut self, device_address: u8, data: &[u8]) -> Result<()> {
        // Process HID input report
        if data.len() >= 3 {
            let buttons = data[0];
            let x_movement = data[1] as i8;
            let y_movement = data[2] as i8;

            // Handle mouse input (example)
            if buttons != 0 || x_movement != 0 || y_movement != 0 {
                // Send input event to system
                self.send_input_event(device_address, buttons, x_movement, y_movement)?;
            }
        }

        Ok(())
    }

    /// Send input event to system
    fn send_input_event(&mut self, _device_address: u8, _buttons: u8, _x: i8, _y: i8) -> Result<()> {
        // In a real implementation, this would send events to the input subsystem
        Ok(())
    }
}

impl Default for HidDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl UsbDeviceDriver for HidDriver {
    fn name(&self) -> &'static str {
        "USB HID Driver"
    }

    fn supports_device(&self, device_descriptor: &DeviceDescriptor) -> bool {
        device_descriptor.device_class == class_codes::HID
    }

    fn bind_device(&mut self, device: &mut UsbDevice) -> Result<()> {
        self.bound_devices.push(device.address);

        // Set up interrupt endpoint for HID reports
        if let Some(config) = device.config_descriptors.first() {
            // In a real implementation, we would parse interface and endpoint descriptors
            // and set up interrupt transfers for HID reports
            let interrupt_endpoint = UsbEndpoint::interrupt(1, UsbDirection::In, 8, 10);
            device.add_endpoint(interrupt_endpoint).map_err(|_| UsbError::InvalidEndpoint)?;
        }

        Ok(())
    }

    fn device_removed(&mut self, device: &UsbDevice) {
        self.bound_devices.retain(|&addr| addr != device.address);
    }

    fn handle_data(&mut self, device_address: u8, _endpoint: u8, data: &[u8]) -> Result<()> {
        if self.bound_devices.contains(&device_address) {
            self.process_hid_report(device_address, data)?;
        }
        Ok(())
    }
}

/// Mass Storage Device Driver
pub struct MassStorageDriver {
    /// Bound devices
    bound_devices: Vec<u8>,
}

impl MassStorageDriver {
    /// Create a new mass storage driver
    pub fn new() -> Self {
        Self {
            bound_devices: Vec::new(),
        }
    }

    /// Execute SCSI command
    fn execute_scsi_command(&mut self, device_address: u8, command: &[u8]) -> Result<Vec<u8>> {
        // In a real implementation, this would send SCSI commands via USB bulk transfers
        // For now, return empty response
        Ok(Vec::new())
    }

    /// Read sectors from storage device
    pub fn read_sectors(&mut self, device_address: u8, lba: u64, sector_count: u32) -> Result<Vec<u8>> {
        // Create READ(10) SCSI command
        let mut command = [0u8; 10];
        command[0] = 0x28; // READ(10) opcode
        command[2] = (lba >> 24) as u8;
        command[3] = (lba >> 16) as u8;
        command[4] = (lba >> 8) as u8;
        command[5] = lba as u8;
        command[7] = (sector_count >> 8) as u8;
        command[8] = sector_count as u8;

        self.execute_scsi_command(device_address, &command)
    }

    /// Write sectors to storage device
    pub fn write_sectors(&mut self, device_address: u8, lba: u64, data: &[u8]) -> Result<()> {
        let sector_count = (data.len() / 512) as u32;

        // Create WRITE(10) SCSI command
        let mut command = [0u8; 10];
        command[0] = 0x2A; // WRITE(10) opcode
        command[2] = (lba >> 24) as u8;
        command[3] = (lba >> 16) as u8;
        command[4] = (lba >> 8) as u8;
        command[5] = lba as u8;
        command[7] = (sector_count >> 8) as u8;
        command[8] = sector_count as u8;

        self.execute_scsi_command(device_address, &command)?;
        Ok(())
    }
}

impl Default for MassStorageDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl UsbDeviceDriver for MassStorageDriver {
    fn name(&self) -> &'static str {
        "USB Mass Storage Driver"
    }

    fn supports_device(&self, device_descriptor: &DeviceDescriptor) -> bool {
        device_descriptor.device_class == class_codes::MASS_STORAGE
    }

    fn bind_device(&mut self, device: &mut UsbDevice) -> Result<()> {
        self.bound_devices.push(device.address);

        // Set up bulk endpoints for mass storage
        if let Some(config) = device.config_descriptors.first() {
            // In a real implementation, we would parse interface and endpoint descriptors
            let bulk_in = UsbEndpoint::bulk(1, UsbDirection::In, 512);
            let bulk_out = UsbEndpoint::bulk(2, UsbDirection::Out, 512);

            device.add_endpoint(bulk_in).map_err(|_| UsbError::InvalidEndpoint)?;
            device.add_endpoint(bulk_out).map_err(|_| UsbError::InvalidEndpoint)?;
        }

        Ok(())
    }

    fn device_removed(&mut self, device: &UsbDevice) {
        self.bound_devices.retain(|&addr| addr != device.address);
    }

    fn handle_data(&mut self, device_address: u8, _endpoint: u8, data: &[u8]) -> Result<()> {
        if self.bound_devices.contains(&device_address) {
            // Process mass storage response data
            // This would typically be CSW (Command Status Wrapper) or data
        }
        Ok(())
    }
}

/// Hub Driver for USB hubs
pub struct HubDriver {
    /// Bound hubs
    bound_hubs: Vec<u8>,
}

impl HubDriver {
    /// Create a new hub driver
    pub fn new() -> Self {
        Self {
            bound_hubs: Vec::new(),
        }
    }

    /// Get hub descriptor
    fn get_hub_descriptor(&mut self, device_address: u8) -> Result<()> {
        // In a real implementation, this would send GET_DESCRIPTOR request for hub
        Ok(())
    }

    /// Set port power
    pub fn set_port_power(&mut self, device_address: u8, port: u8, power: bool) -> Result<()> {
        // In a real implementation, this would send SET_PORT_FEATURE or CLEAR_PORT_FEATURE
        Ok(())
    }

    /// Get port status
    pub fn get_port_status(&mut self, device_address: u8, port: u8) -> Result<u16> {
        // In a real implementation, this would send GET_PORT_STATUS request
        Ok(0)
    }

    /// Reset port
    pub fn reset_port(&mut self, device_address: u8, port: u8) -> Result<()> {
        // In a real implementation, this would send SET_PORT_FEATURE(PORT_RESET)
        Ok(())
    }
}

impl Default for HubDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl UsbDeviceDriver for HubDriver {
    fn name(&self) -> &'static str {
        "USB Hub Driver"
    }

    fn supports_device(&self, device_descriptor: &DeviceDescriptor) -> bool {
        device_descriptor.device_class == class_codes::HUB
    }

    fn bind_device(&mut self, device: &mut UsbDevice) -> Result<()> {
        self.bound_hubs.push(device.address);

        // Get hub descriptor and configure hub
        self.get_hub_descriptor(device.address)?;

        // Set up interrupt endpoint for hub status changes
        let interrupt_endpoint = UsbEndpoint::interrupt(1, UsbDirection::In, 1, 255);
        device.add_endpoint(interrupt_endpoint).map_err(|_| UsbError::InvalidEndpoint)?;

        Ok(())
    }

    fn device_removed(&mut self, device: &UsbDevice) {
        self.bound_hubs.retain(|&addr| addr != device.address);
    }

    fn handle_data(&mut self, device_address: u8, _endpoint: u8, data: &[u8]) -> Result<()> {
        if self.bound_hubs.contains(&device_address) {
            // Process hub status change data
            // This indicates which ports have status changes
            for (port, &status_change) in data.iter().enumerate() {
                if status_change != 0 {
                    // Handle port status change
                    let _port_status = self.get_port_status(device_address, port as u8 + 1)?;
                    // Process port status...
                }
            }
        }
        Ok(())
    }
}