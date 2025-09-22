//! # PrismaOS USB Driver (Modular)
//!
//! This crate provides a modular, production-ready USB driver for PrismaOS, designed for both
//! real-world use and as a teaching resource for OSDev learners. It demonstrates:
//!
//! - Modular bus/controller support (see `bus/`)
//! - Device/class management (see below)
//! - Kernel driver integration
//! - Comprehensive documentation for education and production
//!
//! ## Directory Structure
//! - `bus/`   : Hardware controller implementations (e.g., xHCI)
//! - `class/` : (Future) USB class implementations
//! - `lib.rs` : Driver entry point, device/class glue, kernel integration
//!
//! ## How to Extend
//! - Implement a real hardware bus in `bus/` (see `bus/xhci.rs`)
//! - Add USB classes in `class/`
//! - Integrate with kernel events/interrupts
//! - Use as a template for other drivers


extern crate alloc;
use alloc::sync::Arc;
use spin::RwLock;

use lib_kernel::drivers::{Driver, DriverError};

// Import USB stack types (use correct crate name: usb_device)
use usb_device::{bus::UsbBusAllocator, device::{UsbDevice, UsbDeviceBuilder, UsbVidPid}, class::UsbClass};
mod bus;

/// Example USB class for demonstration (replace with real implementation as needed)
pub struct KernelUsbClass<'a, B: usb_device::bus::UsbBus> {
	_iface: usb_device::bus::InterfaceNumber,
	_ep_in: usb_device::endpoint::EndpointIn<'a, B>,
	_ep_out: usb_device::endpoint::EndpointOut<'a, B>,
}

impl<'a, B: usb_device::bus::UsbBus> KernelUsbClass<'a, B> {
	pub fn new(alloc: &UsbBusAllocator<B>) -> Self {
		Self {
			_iface: alloc.interface(),
			_ep_in: alloc.bulk(64),
			_ep_out: alloc.bulk(64),
		}
	}
}

impl<'a, B: usb_device::bus::UsbBus> UsbClass<B> for KernelUsbClass<'a, B> {}

/// The main USB driver struct
///
/// This struct manages the USB bus, allocator, device, and class.
/// For production, replace the xHCI stub with a real implementation.
use xhci::accessor::Mapper;

/// The main USB driver struct
///
/// This struct manages the USB bus, allocator, device, and class.
/// For production, replace the xHCI stub with a real implementation.
pub struct UsbDriver<M: Mapper + Clone> {
	/// USB bus allocator (for endpoint/class allocation)
	bus_allocator: Option<UsbBusAllocator<bus::xhci::XhciBus<M>>>,
	/// USB device instance
	usb_device: Option<UsbDevice<'static, bus::xhci::XhciBus<M>>>,
	/// USB class instance
	usb_class: Option<KernelUsbClass<'static, bus::xhci::XhciBus<M>>>,
}

impl<M: Mapper + Clone> UsbDriver<M> {
	/// Create a new USB driver (does not initialize hardware yet)
	pub fn new() -> Self {
		Self {
			bus_allocator: None,
			usb_device: None,
			usb_class: None,
		}
	}
}

impl<M: Mapper + Clone + 'static> Driver for UsbDriver<M> {
	fn name(&self) -> &'static str {
		"usb"
	}

	fn init(&mut self) -> Result<(), DriverError> {
		// --- USB Hardware Initialization ---
		// 1. Create the bus (replace with real MMIO/IRQ params for your hardware)
		// SAFETY: The caller must ensure exclusive access to the xHCI controller and provide correct MMIO base and mapper.
		let mmio_base = 0xfee00000; // TODO: Replace with real MMIO base address
		let mapper = unsafe { core::mem::zeroed() }; // TODO: Replace with real Mapper implementation
		let bus = unsafe { bus::xhci::XhciBus::new(mmio_base, mapper) };
		// 2. Create the allocator
		let mut allocator = UsbBusAllocator::new(bus);
		// 3. Create the USB class (replace with your own class as needed)
		let class = KernelUsbClass::new(&allocator);
		// 4. Build the USB device
		let device = UsbDeviceBuilder::new(&allocator, UsbVidPid(0x1234, 0x5678))
			.manufacturer("PrismaOS")
			.product("PrismaOS USB Device")
			.serial_number("0001")
			.build();
		// 5. Store in struct for later use
		self.bus_allocator = Some(allocator);
		self.usb_class = Some(class);
		self.usb_device = Some(device);
		Ok(())
	}

	fn shutdown(&mut self) -> Result<(), DriverError> {
		// Clean up USB resources if needed
		Ok(())
	}

	fn interrupt_handler(&mut self, _irq: u8) -> bool {
		// In a real system, this would be called by the kernel when a USB interrupt fires.
		// For now, we simply poll the device for events.
		poll_usb_driver(self);
		// Return true if an event was handled (for now, always true if device is present)
		self.usb_device.is_some()
	}

	fn as_any(&self) -> &dyn core::any::Any {
		self
	}
}



/// Register the USB driver with the kernel device manager
pub fn register_usb_driver<M: Mapper + Clone + 'static>() {
	use lib_kernel::drivers::device_manager;
	let driver = Arc::new(RwLock::new(UsbDriver::<M>::new()));
	let _ = device_manager().register_driver(driver);
}

/// Poll the USB device for events (should be called from the kernel event loop or interrupt handler)
///
/// This function processes USB events, such as transfers and state changes. It should be called
/// regularly (e.g., from a timer, main loop, or hardware interrupt) to keep the USB stack responsive.
pub fn poll_usb_driver<M: Mapper + Clone>(driver: &mut UsbDriver<M>) {
	if let (Some(device), Some(class)) = (driver.usb_device.as_mut(), driver.usb_class.as_mut()) {
		// Poll the USB device. This will call into the bus and class as needed.
		if device.poll(&mut [class]) {
			// Handle any events or completed transfers here if needed
			// (e.g., notify the kernel, update state, etc.)
		}
	}
}
