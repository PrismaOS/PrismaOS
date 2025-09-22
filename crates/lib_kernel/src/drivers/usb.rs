//! USB driver integration for the kernel driver system
use alloc::sync::Arc;
use spin::RwLock;

use crate::drivers::{Driver, DriverError};

// Import the USB driver from the external driver crate
use usb as usb_driver_crate;

pub struct UsbDriver {
	inner: Arc<RwLock<usb_driver_crate::UsbDriver>>,
}

impl UsbDriver {
	pub fn new() -> Self {
		Self {
			inner: Arc::new(RwLock::new(usb_driver_crate::UsbDriver::new())),
		}
	}
}

impl Driver for UsbDriver {
	fn name(&self) -> &'static str {
		self.inner.read().name()
	}

	fn init(&mut self) -> Result<(), DriverError> {
		self.inner.write().init()
	}

	fn shutdown(&mut self) -> Result<(), DriverError> {
		self.inner.write().shutdown()
	}

	fn interrupt_handler(&mut self, irq: u8) -> bool {
		self.inner.write().interrupt_handler(irq)
	}

	fn as_any(&self) -> &dyn core::any::Any {
		self
	}
}

/// Register the USB driver with the kernel device manager
pub fn register_usb_driver() {
	use crate::drivers::device_manager;
	let driver = Arc::new(RwLock::new(UsbDriver::new()));
	let _ = device_manager().register_driver(driver);
}
