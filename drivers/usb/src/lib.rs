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
use xhci::{XhciController, UsbTransferManager, UsbTransferRequest, UsbTransferType, UsbTransferStatus, UsbTransferResult, UsbTransferStats, ControlTransfer};
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