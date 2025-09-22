#![no_std]
#![deny(missing_docs)]
#![warn(clippy::all)]

//! Comprehensive USB Driver for PrismaOS
//!
//! This crate provides a complete USB host controller driver supporting
//! xHCI (USB 3.0+) with full device enumeration and management capabilities.
//! It integrates with the lib_kernel driver framework.

extern crate alloc;

pub mod error;
pub mod types;
pub mod enumeration;
pub mod xhci;

// Legacy constants for backward compatibility
mod consts;

use error::{UsbError, Result};
use types::*;
use enumeration::*;
use xhci::{XhciController, UsbTransferManager, UsbTransferRequest, UsbTransferStatus, UsbTransferResult, UsbTransferStats, ControlTransfer};
use xhci::transfer::UsbTransferType;
use lib_kernel::drivers::{Driver, DriverError, device_manager};
use alloc::{sync::Arc, boxed::Box, vec::Vec};
use spin::RwLock;

/// USB subsystem manager
pub struct UsbSubsystem {
    /// xHCI controllers
    xhci_controllers: Vec<Arc<RwLock<XhciController>>>,
    /// Device manager
    device_manager: UsbDeviceManager,
    /// Initialization state
    initialized: bool,
}

impl UsbSubsystem {
    /// Create a new USB subsystem
    pub fn new() -> Self {
        Self {
            xhci_controllers: Vec::new(),
            device_manager: UsbDeviceManager::new(),
            initialized: false,
        }
    }

    /// Initialize the USB subsystem
    pub fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Register built-in device drivers
        self.register_builtin_drivers();

        // Scan for USB controllers
        self.scan_controllers()?;

        // Initialize found controllers
        self.initialize_controllers()?;

        self.initialized = true;
        Ok(())
    }

    /// Register built-in device drivers
    fn register_builtin_drivers(&mut self) {
        self.device_manager.register_driver(Box::new(HidDriver::new()));
        self.device_manager.register_driver(Box::new(MassStorageDriver::new()));
        self.device_manager.register_driver(Box::new(HubDriver::new()));
    }

    /// Scan for USB controllers
    fn scan_controllers(&mut self) -> Result<()> {
        // In a real implementation, this would scan PCI for USB controllers
        // For now, we'll create a mock xHCI controller
        let controller = Arc::new(RwLock::new(XhciController::new(
            "xHCI Controller 0",
            0xF0000000, // Mock base address
            Some(16),   // Mock IRQ
        )));

        self.xhci_controllers.push(controller);
        Ok(())
    }

    /// Initialize all found controllers
    fn initialize_controllers(&mut self) -> Result<()> {
        for controller in &self.xhci_controllers {
            // Register with device manager
            let driver_arc: Arc<RwLock<dyn Driver>> = controller.clone() as Arc<RwLock<dyn Driver>>;
            device_manager()
                .register_driver(driver_arc)
                .map_err(|_| UsbError::InitializationFailed)?;

            // Register IRQ handler if available
            if let Some(irq) = controller.read().irq_line {
                device_manager().register_irq_handler(irq, controller.clone() as Arc<RwLock<dyn Driver>>);
            }
        }

        Ok(())
    }

    /// Get all connected USB devices
    pub fn get_devices(&self) -> &[UsbDevice] {
        self.device_manager.get_devices()
    }

    /// Find devices by class
    pub fn find_devices_by_class(&self, class_code: u8) -> Vec<&UsbDevice> {
        self.device_manager.find_devices_by_class(class_code)
    }

    /// Get device by address
    pub fn get_device(&self, address: u8) -> Option<&UsbDevice> {
        self.device_manager.get_device(address)
    }

    /// Get controller count
    pub fn controller_count(&self) -> usize {
        self.xhci_controllers.len()
    }

    /// Get controller capabilities
    pub fn get_controller_capabilities(&self, index: usize) -> Option<crate::xhci::XhciCapabilities> {
        if let Some(controller) = self.xhci_controllers.get(index) {
            controller.read().get_capabilities().copied()
        } else {
            None
        }
    }

    /// Shutdown the USB subsystem
    pub fn shutdown(&mut self) -> Result<()> {
        for controller in &self.xhci_controllers {
            let _ = controller.write().shutdown();
        }
        self.initialized = false;
        Ok(())
    }
}

impl Default for UsbSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global USB subsystem instance
static USB_SUBSYSTEM: RwLock<UsbSubsystem> = RwLock::new(UsbSubsystem {
    xhci_controllers: Vec::new(),
    device_manager: UsbDeviceManager {
        devices: Vec::new(),
        next_address: 1,
        device_drivers: Vec::new(),
    },
    initialized: false,
});

/// Initialize the USB subsystem
pub fn init() -> Result<()> {
    USB_SUBSYSTEM.write().initialize()
}

/// Get the global USB subsystem
pub fn usb_subsystem() -> &'static RwLock<UsbSubsystem> {
    &USB_SUBSYSTEM
}

/// Legacy function for backward compatibility
pub fn init_xhci() {
    let _ = init();
}

/// USB device information for external use
#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    /// Device address
    pub address: u8,
    /// Port number
    pub port: u8,
    /// Device speed
    pub speed: UsbSpeed,
    /// Device state
    pub state: UsbDeviceState,
    /// Vendor ID
    pub vendor_id: Option<u16>,
    /// Product ID
    pub product_id: Option<u16>,
    /// Device class
    pub device_class: Option<u8>,
    /// Configuration value
    pub configuration: Option<u8>,
}

impl From<&UsbDevice> for UsbDeviceInfo {
    fn from(device: &UsbDevice) -> Self {
        Self {
            address: device.address,
            port: device.port,
            speed: device.speed,
            state: device.state,
            vendor_id: device.device_descriptor.as_ref().map(|d| d.vendor_id),
            product_id: device.device_descriptor.as_ref().map(|d| d.product_id),
            device_class: device.device_descriptor.as_ref().map(|d| d.device_class),
            configuration: device.active_config,
        }
    }
}

/// Get information about all connected USB devices
pub fn get_device_info() -> Vec<UsbDeviceInfo> {
    USB_SUBSYSTEM
        .read()
        .get_devices()
        .iter()
        .map(UsbDeviceInfo::from)
        .collect()
}

/// Find USB devices by class
pub fn find_devices_by_class(class_code: u8) -> Vec<UsbDeviceInfo> {
    USB_SUBSYSTEM
        .read()
        .find_devices_by_class(class_code)
        .iter()
        .map(|&device| UsbDeviceInfo::from(device))
        .collect()
}

/// Get USB controller count
pub fn get_controller_count() -> usize {
    USB_SUBSYSTEM.read().controller_count()
}

/// USB Statistics
#[derive(Debug, Clone, Copy)]
pub struct UsbStats {
    /// Number of controllers
    pub controllers: usize,
    /// Number of connected devices
    pub devices: usize,
    /// Number of HID devices
    pub hid_devices: usize,
    /// Number of mass storage devices
    pub storage_devices: usize,
    /// Number of hubs
    pub hubs: usize,
}

/// Get USB subsystem statistics
pub fn get_usb_stats() -> UsbStats {
    let subsystem = USB_SUBSYSTEM.read();
    let devices = subsystem.get_devices();

    let hid_devices = devices
        .iter()
        .filter(|d| {
            d.device_descriptor
                .as_ref()
                .map(|desc| desc.device_class == class_codes::HID)
                .unwrap_or(false)
        })
        .count();

    let storage_devices = devices
        .iter()
        .filter(|d| {
            d.device_descriptor
                .as_ref()
                .map(|desc| desc.device_class == class_codes::MASS_STORAGE)
                .unwrap_or(false)
        })
        .count();

    let hubs = devices
        .iter()
        .filter(|d| {
            d.device_descriptor
                .as_ref()
                .map(|desc| desc.device_class == class_codes::HUB)
                .unwrap_or(false)
        })
        .count();

    UsbStats {
        controllers: subsystem.controller_count(),
        devices: devices.len(),
        hid_devices,
        storage_devices,
        hubs,
    }
}

/// Submit a control transfer to a USB device
pub fn submit_control_transfer(
    device_address: u8,
    setup_packet: UsbRequest,
    data: Vec<u8>,
    direction: UsbDirection,
) -> Result<u32> {
    let mut subsystem = USB_SUBSYSTEM.write();

    // Find the controller managing this device
    for controller in &mut subsystem.xhci_controllers {
        let mut controller_lock = controller.write();

        // Check if this controller has the device
        if controller_lock.get_devices().iter().any(|d| d.address == device_address) {
            return controller_lock.submit_control_transfer(device_address, setup_packet, data, direction);
        }
    }

    Err(UsbError::DeviceNotFound)
}

/// Submit a bulk transfer to a USB device
pub fn submit_bulk_transfer(
    device_address: u8,
    endpoint: u8,
    data: Vec<u8>,
    direction: UsbDirection,
) -> Result<u32> {
    let mut subsystem = USB_SUBSYSTEM.write();

    // Find the controller managing this device
    for controller in &mut subsystem.xhci_controllers {
        let mut controller_lock = controller.write();

        // Check if this controller has the device
        if controller_lock.get_devices().iter().any(|d| d.address == device_address) {
            return controller_lock.submit_bulk_transfer(device_address, endpoint, data, direction);
        }
    }

    Err(UsbError::DeviceNotFound)
}

/// Submit an interrupt transfer to a USB device
pub fn submit_interrupt_transfer(
    device_address: u8,
    endpoint: u8,
    data: Vec<u8>,
    direction: UsbDirection,
    interval: u16,
) -> Result<u32> {
    let mut subsystem = USB_SUBSYSTEM.write();

    // Find the controller managing this device
    for controller in &mut subsystem.xhci_controllers {
        let mut controller_lock = controller.write();

        // Check if this controller has the device
        if controller_lock.get_devices().iter().any(|d| d.address == device_address) {
            return controller_lock.submit_interrupt_transfer(device_address, endpoint, data, direction, interval);
        }
    }

    Err(UsbError::DeviceNotFound)
}

/// Submit an isochronous transfer to a USB device
pub fn submit_isochronous_transfer(
    device_address: u8,
    endpoint: u8,
    data: Vec<u8>,
    direction: UsbDirection,
    frame_number: u16,
) -> Result<u32> {
    let mut subsystem = USB_SUBSYSTEM.write();

    // Find the controller managing this device
    for controller in &mut subsystem.xhci_controllers {
        let mut controller_lock = controller.write();

        // Check if this controller has the device
        if controller_lock.get_devices().iter().any(|d| d.address == device_address) {
            return controller_lock.submit_isochronous_transfer(device_address, endpoint, data, direction, frame_number);
        }
    }

    Err(UsbError::DeviceNotFound)
}

/// Get the status of a transfer
pub fn get_transfer_status(transfer_id: u32) -> Option<UsbTransferStatus> {
    let subsystem = USB_SUBSYSTEM.read();

    // Check all controllers for the transfer
    for controller in &subsystem.xhci_controllers {
        let controller_lock = controller.read();
        if let Some(status) = controller_lock.get_transfer_status(transfer_id) {
            return Some(status);
        }
    }

    None
}

/// Cancel a transfer
pub fn cancel_transfer(transfer_id: u32) -> Result<()> {
    let mut subsystem = USB_SUBSYSTEM.write();

    // Try to cancel on all controllers
    for controller in &mut subsystem.xhci_controllers {
        let mut controller_lock = controller.write();
        if controller_lock.cancel_transfer(transfer_id).is_ok() {
            return Ok(());
        }
    }

    Err(UsbError::InvalidRequest)
}

/// Get all completed transfers from all controllers
pub fn get_completed_transfers() -> Vec<UsbTransferResult> {
    let mut subsystem = USB_SUBSYSTEM.write();
    let mut all_results = Vec::new();

    for controller in &mut subsystem.xhci_controllers {
        let mut controller_lock = controller.write();
        all_results.extend(controller_lock.get_completed_transfers());
    }

    all_results
}

/// Helper functions for common control transfers
pub mod control_transfers {
    use super::*;

    /// Get device descriptor
    pub fn get_device_descriptor(device_address: u8) -> Result<u32> {
        let request = ControlTransfer::get_descriptor(device_address, descriptor_types::DEVICE, 0, 0, 18);
        submit_control_transfer(device_address, request.setup_packet.unwrap(), request.data, request.direction)
    }

    /// Get configuration descriptor
    pub fn get_configuration_descriptor(device_address: u8, config_index: u8) -> Result<u32> {
        let request = ControlTransfer::get_descriptor(device_address, descriptor_types::CONFIGURATION, config_index, 0, 9);
        submit_control_transfer(device_address, request.setup_packet.unwrap(), request.data, request.direction)
    }

    /// Set device address
    pub fn set_device_address(current_address: u8, new_address: u8) -> Result<u32> {
        let request = ControlTransfer::set_address(current_address, new_address);
        submit_control_transfer(current_address, request.setup_packet.unwrap(), request.data, request.direction)
    }

    /// Set device configuration
    pub fn set_device_configuration(device_address: u8, configuration_value: u8) -> Result<u32> {
        let request = ControlTransfer::set_configuration(device_address, configuration_value);
        submit_control_transfer(device_address, request.setup_packet.unwrap(), request.data, request.direction)
    }
}