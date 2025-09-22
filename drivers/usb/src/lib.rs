//! Comprehensive USB Host Controller Driver for PrismaOS
//!
//! This driver provides a complete USB 3.0/2.0/1.1 host controller implementation
//! using modular architecture with xHCI backend support. It handles device
//! enumeration, configuration, transfer management, hub support, and USB class drivers.

#![no_std]
#![allow(dead_code)]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec, collections::BTreeMap, sync::Arc};
use core::{
    fmt,
    sync::atomic::{AtomicU8, Ordering},
};
use spin::{Mutex, RwLock};
use xhci::accessor::Mapper;

pub mod error;
pub mod device;
pub mod endpoint;
pub mod transfer;
pub mod hub;
pub mod class;
pub mod descriptor;
pub mod controller;
pub mod memory;
pub mod async_ops;

pub use error::UsbDriverError;
pub use device::{UsbDevice, DeviceState, DeviceClass};
pub use endpoint::{EndpointType, EndpointDirection, Endpoint};
pub use transfer::{TransferType, Transfer, TransferBuffer};
pub use hub::{UsbHub, PortStatus};
pub use class::{UsbClassDriver, ClassType};
pub use controller::{UsbController, ControllerState};

/// USB Driver result type
pub type Result<T> = core::result::Result<T, UsbDriverError>;

/// USB Host Controller Driver
pub struct UsbHostDriver {
    /// xHCI controller instance
    controller: Arc<Mutex<UsbController>>,
    /// Connected devices
    devices: Arc<RwLock<BTreeMap<u8, Arc<Mutex<UsbDevice>>>>>,
    /// Root hub
    root_hub: Arc<Mutex<UsbHub>>,
    /// Class drivers
    class_drivers: Arc<RwLock<Vec<Box<dyn UsbClassDriver + Send + Sync>>>>,
    /// Memory allocator for USB operations
    memory_allocator: Arc<Mutex<memory::UsbMemoryAllocator>>,
    /// Driver state
    state: Arc<AtomicU8>,
}

/// USB driver state
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    Uninitialized = 0,
    Initializing = 1,
    Running = 2,
    Suspended = 3,
    Error = 4,
}

impl From<u8> for DriverState {
    fn from(value: u8) -> Self {
        match value {
            0 => DriverState::Uninitialized,
            1 => DriverState::Initializing,
            2 => DriverState::Running,
            3 => DriverState::Suspended,
            _ => DriverState::Error,
        }
    }
}

impl UsbHostDriver {
    /// Create a new USB host driver instance
    pub fn new<M: Mapper + Clone + Send + Sync + 'static>(
        mmio_base: usize,
        mapper: M,
    ) -> Result<Self> {
        let controller = UsbController::new(mmio_base, mapper)?;
        let memory_allocator = memory::UsbMemoryAllocator::new()?;
        let root_hub = UsbHub::new_root_hub()?;

        Ok(Self {
            controller: Arc::new(Mutex::new(controller)),
            devices: Arc::new(RwLock::new(BTreeMap::new())),
            root_hub: Arc::new(Mutex::new(root_hub)),
            class_drivers: Arc::new(RwLock::new(Vec::new())),
            memory_allocator: Arc::new(Mutex::new(memory_allocator)),
            state: Arc::new(AtomicU8::new(DriverState::Uninitialized as u8)),
        })
    }

    /// Initialize the USB host driver
    pub async fn initialize(&self) -> Result<()> {
        self.state.store(DriverState::Initializing as u8, Ordering::SeqCst);

        // Initialize controller
        {
            let mut controller = self.controller.lock();
            controller.initialize().await?;
        }

        // Initialize root hub
        {
            let mut root_hub = self.root_hub.lock();
            root_hub.initialize().await?;
        }

        // Start device enumeration
        self.enumerate_devices().await?;

        self.state.store(DriverState::Running as u8, Ordering::SeqCst);
        Ok(())
    }

    /// Get current driver state
    pub fn state(&self) -> DriverState {
        DriverState::from(self.state.load(Ordering::Acquire))
    }

    /// Register a USB class driver
    pub fn register_class_driver(&self, driver: Box<dyn UsbClassDriver + Send + Sync>) {
        let mut drivers = self.class_drivers.write();
        drivers.push(driver);
    }

    /// Enumerate all connected USB devices
    pub async fn enumerate_devices(&self) -> Result<()> {
        let mut root_hub = self.root_hub.lock();
        let port_count = root_hub.port_count();

        for port in 0..port_count {
            if let Some(device) = root_hub.probe_port(port).await? {
                self.add_device(device).await?;
            }
        }

        Ok(())
    }

    /// Add a newly discovered device
    async fn add_device(&self, mut device: UsbDevice) -> Result<()> {
        // Configure device
        device.configure().await?;

        // Find appropriate class driver
        let device_address = device.address();
        let class_drivers = self.class_drivers.read();
        for driver in class_drivers.iter() {
            if driver.supports_device(&device) {
                driver.attach_device(device_address).await?;
                break;
            }
        }

        // Store device
        let mut devices = self.devices.write();
        devices.insert(device_address, Arc::new(Mutex::new(device)));

        Ok(())
    }

    /// Remove a disconnected device
    pub async fn remove_device(&self, address: u8) -> Result<()> {
        let mut devices = self.devices.write();
        if let Some(device_arc) = devices.remove(&address) {
            let device = device_arc.lock();

            // Notify class drivers
            let class_drivers = self.class_drivers.read();
            for driver in class_drivers.iter() {
                if driver.supports_device(&device) {
                    driver.detach_device(address).await?;
                    break;
                }
            }
        }

        Ok(())
    }

    /// Get device by address
    pub fn get_device(&self, address: u8) -> Option<Arc<Mutex<UsbDevice>>> {
        let devices = self.devices.read();
        devices.get(&address).cloned()
    }

    /// Submit a USB transfer
    pub async fn submit_transfer(&self, transfer: Transfer) -> Result<usize> {
        let mut controller = self.controller.lock();
        controller.submit_transfer(transfer).await
    }

    /// Handle USB events (called from interrupt handler)
    pub async fn handle_events(&self) -> Result<()> {
        let mut controller = self.controller.lock();
        let events = controller.poll_events().await?;

        for event in events {
            match event {
                controller::UsbEvent::DeviceConnected { port } => {
                    // Handle device connection
                    let mut root_hub = self.root_hub.lock();
                    if let Some(device) = root_hub.probe_port(port).await? {
                        self.add_device(device).await?;
                    }
                }
                controller::UsbEvent::DeviceDisconnected { address } => {
                    // Handle device disconnection
                    self.remove_device(address).await?;
                }
                controller::UsbEvent::TransferComplete { transfer_id, status } => {
                    // Handle transfer completion
                    controller.complete_transfer(transfer_id, status).await?;
                }
                controller::UsbEvent::Error { error } => {
                    // Handle error
                    log::error!("USB Error: {:?}", error);
                }
            }
        }

        Ok(())
    }

    /// Suspend the USB driver
    pub async fn suspend(&self) -> Result<()> {
        self.state.store(DriverState::Suspended as u8, Ordering::SeqCst);

        let mut controller = self.controller.lock();
        controller.suspend().await?;

        Ok(())
    }

    /// Resume the USB driver
    pub async fn resume(&self) -> Result<()> {
        let mut controller = self.controller.lock();
        controller.resume().await?;

        self.state.store(DriverState::Running as u8, Ordering::SeqCst);
        Ok(())
    }

    /// Shutdown the USB driver
    pub async fn shutdown(&self) -> Result<()> {
        // Remove all devices
        let mut devices = self.devices.write();
        let addresses: Vec<u8> = devices.keys().cloned().collect();
        for address in addresses {
            if let Some(_device) = devices.remove(&address) {
                // Notify class drivers about device removal
                let class_drivers = self.class_drivers.read();
                for driver in class_drivers.iter() {
                    // Attempt to detach without handling errors during shutdown
                    let _ = driver.detach_device(address).await;
                }
            }
        }

        // Shutdown controller
        let mut controller = self.controller.lock();
        controller.shutdown().await?;

        self.state.store(DriverState::Uninitialized as u8, Ordering::SeqCst);
        Ok(())
    }
}

unsafe impl Send for UsbHostDriver {}
unsafe impl Sync for UsbHostDriver {}

impl fmt::Debug for UsbHostDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsbHostDriver")
            .field("state", &self.state())
            .finish()
    }
}

/// Initialize the USB subsystem
pub async fn init_usb_subsystem<M: Mapper + Clone + Send + Sync + 'static>(
    mmio_base: usize,
    mapper: M,
) -> Result<Arc<UsbHostDriver>> {
    let driver = Arc::new(UsbHostDriver::new(mmio_base, mapper)?);
    driver.initialize().await?;
    Ok(driver)
}