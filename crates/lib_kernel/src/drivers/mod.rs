use alloc::{boxed::Box, collections::BTreeMap, sync::Arc, vec::Vec};
use core::any::Any;
use spin::RwLock;

/// Device driver trait that all drivers must implement
pub trait Driver: Send + Sync {
    fn name(&self) -> &'static str;
    fn init(&mut self) -> Result<(), DriverError>;
    fn shutdown(&mut self) -> Result<(), DriverError>;
    fn interrupt_handler(&mut self, irq: u8) -> bool;
    fn as_any(&self) -> &dyn Any;
}

/// Device manager for registering and managing device drivers
pub struct DeviceManager {
    drivers: RwLock<BTreeMap<&'static str, Arc<RwLock<dyn Driver>>>>,
    irq_handlers: RwLock<BTreeMap<u8, Vec<Arc<RwLock<dyn Driver>>>>>,
}

impl DeviceManager {
    pub const fn new() -> Self {
        DeviceManager {
            drivers: RwLock::new(BTreeMap::new()),
            irq_handlers: RwLock::new(BTreeMap::new()),
        }
    }

    /// Register a device driver
    pub fn register_driver(&self, driver: Arc<RwLock<dyn Driver>>) -> Result<(), DriverError> {
        let name = {
            let d = driver.read();
            d.name()
        };

        let mut drivers = self.drivers.write();
        if drivers.contains_key(name) {
            return Err(DriverError::AlreadyRegistered);
        }

        drivers.insert(name, driver.clone());
        
        // Initialize the driver
        driver.write().init()?;
        
        crate::println!("Registered driver: {}", name);
        Ok(())
    }

    /// Unregister a device driver
    pub fn unregister_driver(&self, name: &str) -> Result<(), DriverError> {
        let mut drivers = self.drivers.write();
        if let Some(driver) = drivers.remove(name) {
            driver.write().shutdown()?;
            
            // Remove from IRQ handlers
            let mut handlers = self.irq_handlers.write();
            for (_, handler_list) in handlers.iter_mut() {
                handler_list.retain(|d| {
                    let d_name = d.read().name();
                    d_name != name
                });
            }
            
            crate::println!("Unregistered driver: {}", name);
            Ok(())
        } else {
            Err(DriverError::NotFound)
        }
    }

    /// Register driver for IRQ handling
    pub fn register_irq_handler(&self, irq: u8, driver: Arc<RwLock<dyn Driver>>) {
        let mut handlers = self.irq_handlers.write();
        handlers.entry(irq).or_insert_with(Vec::new).push(driver);
    }

    /// Handle hardware interrupt
    pub fn handle_interrupt(&self, irq: u8) -> bool {
        let handlers = self.irq_handlers.read();
        if let Some(driver_list) = handlers.get(&irq) {
            let mut handled = false;
            for driver in driver_list {
                if driver.write().interrupt_handler(irq) {
                    handled = true;
                }
            }
            handled
        } else {
            false
        }
    }

    /// Get driver by name
    pub fn get_driver(&self, name: &str) -> Option<Arc<RwLock<dyn Driver>>> {
        self.drivers.read().get(name).cloned()
    }

    /// List all registered drivers
    pub fn list_drivers(&self) -> Vec<&'static str> {
        self.drivers.read().keys().copied().collect()
    }

    /// Get driver count
    pub fn driver_count(&self) -> usize {
        self.drivers.read().len()
    }
}

/// Global device manager instance
static DEVICE_MANAGER: DeviceManager = DeviceManager::new();

pub fn device_manager() -> &'static DeviceManager {
    &DEVICE_MANAGER
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverError {
    InitializationFailed,
    AlreadyRegistered,
    NotFound,
    HardwareError,
    InvalidParameter,
    ResourceBusy,
    InsufficientMemory,
}

/// Device information structure
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
    pub base_addresses: Vec<u64>,
    pub irq_line: Option<u8>,
    pub name: &'static str,
}

/// PCI device enumeration and management
pub struct PciManager {
    devices: RwLock<Vec<DeviceInfo>>,
}

impl PciManager {
    pub const fn new() -> Self {
        PciManager {
            devices: RwLock::new(Vec::new()),
        }
    }

    /// Enumerate PCI devices
    pub fn enumerate_devices(&self) {
        // In a real implementation, this would scan the PCI bus
        // and populate the devices list
        let mut devices = self.devices.write();
        
        // Mock some common devices for demo
        devices.push(DeviceInfo {
            vendor_id: 0x8086,
            device_id: 0x100E,
            class_code: 0x02,
            subclass: 0x00,
            prog_if: 0x00,
            revision: 0x02,
            base_addresses: alloc::vec![0xC0000000],
            irq_line: Some(11),
            name: "Intel 82540EM Ethernet",
        });

        devices.push(DeviceInfo {
            vendor_id: 0x1234,
            device_id: 0x1111,
            class_code: 0x03,
            subclass: 0x00,
            prog_if: 0x00,
            revision: 0x02,
            base_addresses: alloc::vec![0xD0000000],
            irq_line: Some(10),
            name: "QEMU VGA",
        });
        
        crate::println!("Enumerated {} PCI devices", devices.len());
    }

    /// Get all PCI devices
    pub fn get_devices(&self) -> Vec<DeviceInfo> {
        self.devices.read().clone()
    }

    /// Find device by vendor/device ID
    pub fn find_device(&self, vendor_id: u16, device_id: u16) -> Option<DeviceInfo> {
        let devices = self.devices.read();
        devices.iter()
            .find(|dev| dev.vendor_id == vendor_id && dev.device_id == device_id)
            .cloned()
    }

    /// Find devices by class code
    pub fn find_devices_by_class(&self, class_code: u8) -> Vec<DeviceInfo> {
        let devices = self.devices.read();
        devices.iter()
            .filter(|dev| dev.class_code == class_code)
            .cloned()
            .collect()
    }
}

static PCI_MANAGER: PciManager = PciManager::new();

pub fn pci_manager() -> &'static PciManager {
    &PCI_MANAGER
}