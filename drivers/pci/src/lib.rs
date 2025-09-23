#![no_std]

extern crate alloc;

use lib_kernel::kprintln;
use alloc::{string::{String, ToString}, vec::Vec, format};
use ez_pci::PciAccess;
use ids_rs::{parser::VendorBuilder, DeviceId, PciDatabase, VendorId};

/// USB Controller information
#[derive(Debug, Clone)]
pub struct UsbController {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub controller_type: UsbControllerType,
    pub vendor_name: String,
    pub device_name: String,
    pub bar0_addr: Option<u64>,
    pub bar0_size: Option<usize>,
}

/// Types of USB controllers
#[derive(Debug, Clone)]
pub enum UsbControllerType {
    UHCI,  // USB 1.1 (prog_if 0x00)
    OHCI,  // USB 1.1 (prog_if 0x10)
    EHCI,  // USB 2.0 (prog_if 0x20)
    XHCI,  // USB 3.0 (prog_if 0x30)
    Unknown(u8),
}

/// Get USB controllers from PCI bus
pub fn get_usb_controllers() -> Vec<UsbController> {
    let db = PciDatabase::get();
    let mut pci = unsafe { PciAccess::new_pci() };
    let mut usb_controllers = Vec::new();

    let busses = pci.known_buses();

    for bus in busses {
        let mut specific_bus = pci.bus(bus);

        // Enumerate through all possible device slots (0-31) on this bus
        for device_num in 0..32u8 {
            if let Some(mut device) = specific_bus.device(device_num) {
                let functions = device.possible_functions();

                for function in functions {
                    if let Some(mut pci_fn) = device.function(function) {
                        let class_code = pci_fn.class_code();
                        let subclass = pci_fn.sub_class();
                        let prog_if = pci_fn.prog_if();
                        let vendor_id = pci_fn.vendor_id();
                        let device_id = pci_fn.device_id();

                        // Skip invalid devices
                        if vendor_id == 0xFFFF || vendor_id == 0x0000 {
                            continue;
                        }

                        // USB controllers have class code 0x0C (Serial Bus Controller)
                        // and subclass 0x03 (USB controller)
                        if class_code == 0x0C && subclass == 0x03 {
                            // Look up vendor and device names
                            let (vendor_name, device_name) = if let Some(vendor) = db.find_vendor(VendorId::new(vendor_id)) {
                                let vendor_name = vendor.name().to_string();
                                let device_name = if let Some(device) = vendor.find_device(DeviceId::new(device_id)) {
                                    device.name().to_string()
                                } else {
                                    format!("Unknown Device {:#06x}", device_id)
                                };
                                (vendor_name, device_name)
                            } else {
                                (format!("Unknown Vendor {:#06x}", vendor_id), format!("Unknown Device {:#06x}", device_id))
                            };

                            let controller_type = match prog_if {
                                0x00 => UsbControllerType::UHCI,
                                0x10 => UsbControllerType::OHCI,
                                0x20 => UsbControllerType::EHCI,
                                0x30 => UsbControllerType::XHCI,
                                other => UsbControllerType::Unknown(other),
                            };

                            // Read PCI BAR0 for MMIO base address
                            let (bar0_addr, bar0_size) = read_pci_bar0(&mut pci_fn);

                            usb_controllers.push(UsbController {
                                bus,
                                device: device_num,
                                function,
                                vendor_id,
                                device_id,
                                class_code,
                                subclass,
                                prog_if,
                                controller_type,
                                vendor_name,
                                device_name,
                                bar0_addr,
                                bar0_size,
                            });
                        }
                    }
                }
            }
        }
    }

    usb_controllers
}

/// Read PCI BAR0 to get the MMIO base address and size
fn read_pci_bar0(pci_fn: &mut ez_pci::PciFunction) -> (Option<u64>, Option<usize>) {
    // Read BAR0 register (offset 0x10)
    let bar0_value = pci_fn.read_u32(0x10);

    if bar0_value == 0 || bar0_value == 0xFFFFFFFF {
        return (None, None);
    }

    // Check if this is a memory BAR (bit 0 = 0)
    if (bar0_value & 0x1) != 0 {
        // This is an I/O BAR, not memory
        return (None, None);
    }

    // Extract memory type (bits 2-1)
    let mem_type = (bar0_value >> 1) & 0x3;

    let (base_addr, bar_size) = match mem_type {
        0x0 => {
            // 32-bit BAR
            let base = (bar0_value & 0xFFFFFFF0) as u64;

            // Calculate size by writing all 1s and reading back
            pci_fn.write_u32(0x10, 0xFFFFFFFF);
            let size_mask = pci_fn.read_u32(0x10);
            pci_fn.write_u32(0x10, bar0_value); // Restore original value

            let size = if size_mask != 0 {
                let masked = size_mask & 0xFFFFFFF0;
                (!masked + 1) as usize
            } else {
                0
            };

            (base, size)
        },
        0x2 => {
            // 64-bit BAR
            let bar1_value = pci_fn.read_u32(0x14);
            let base = ((bar0_value & 0xFFFFFFF0) as u64) | ((bar1_value as u64) << 32);

            // Calculate size for 64-bit BAR
            pci_fn.write_u32(0x10, 0xFFFFFFFF);
            pci_fn.write_u32(0x14, 0xFFFFFFFF);
            let size_mask_low = pci_fn.read_u32(0x10);
            let size_mask_high = pci_fn.read_u32(0x14);
            pci_fn.write_u32(0x10, bar0_value); // Restore original values
            pci_fn.write_u32(0x14, bar1_value);

            let size_mask = ((size_mask_low & 0xFFFFFFF0) as u64) | ((size_mask_high as u64) << 32);
            let size = if size_mask != 0 {
                (!size_mask + 1) as usize
            } else {
                0
            };

            (base, size)
        },
        _ => {
            // Reserved or invalid type
            return (None, None);
        }
    };

    if base_addr == 0 || bar_size == 0 {
        (None, None)
    } else {
        (Some(base_addr), Some(bar_size))
    }
}

pub fn init_pci() -> PciAccess {
    let db = PciDatabase::get();

    let mut pci = unsafe {
        PciAccess::new_pci()
    };
    
    let busses = pci.known_buses();

    for bus in busses {
        kprintln!("Found bus: {}", bus);
        let mut specific_bus = pci.bus(bus);

        // Enumerate through all possible device slots (0-31) on this bus
        for device_num in 0..32u8 {
            if let Some(mut device) = specific_bus.device(device_num) {
                let functions: core::ops::RangeInclusive<u8> = device.possible_functions();

                // Try to get vendor/device ID and class from function 0 (common for all functions)
                let fn0 = device.function(0);
                let (vendor_id, device_id, class_code) = if let Some(mut fn0) = fn0 {
                    let vendor_id = fn0.vendor_id();
                    let device_id = fn0.device_id();
                    let class_code = fn0.class_code();
                    kprintln!("  Vendor ID: {:#06x}, Device ID: {:#06x}", vendor_id, device_id);
                    (vendor_id, device_id, class_code)
                } else {
                    continue; // Skip invalid devices
                };

                // Skip invalid devices
                if vendor_id == 0xFFFF || vendor_id == 0x0000 {
                    continue;
                }

                // Guess PCI type
                let mut is_pcie = false;
                let mut has_capabilities = false;
                for function in functions.clone() {
                    if let Some(mut pci_fn) = device.function(function) {
                        if let Some(mut caps) = pci_fn.capabilities() {
                            let mut found_cap = false;
                            while let Some(cap) = caps.next() {
                                found_cap = true;
                                if cap.id == 0x10 {
                                    is_pcie = true;
                                }
                            }
                            if found_cap {
                                has_capabilities = true;
                            }
                        }
                    }
                }

                let pci_type = if is_pcie {
                    "PCI Express"
                } else if has_capabilities {
                    "PCI"
                } else {
                    "Legacy/Non-PCI"
                };
                kprintln!("  Type: {}", pci_type);

                // Optionally, print all functions and their capabilities count
                for function in functions {
                    kprintln!("    function: {}", function);
                    if let Some(mut pci_fn) = device.function(function) {
                        if let Some(caps) = pci_fn.capabilities() {
                            let cap_count = caps.count();
                            kprintln!("      Number of capabilities: {}", cap_count);
                        } else {
                            kprintln!("      Could not fetch capabilities");
                        }
                    } else {
                        kprintln!("      Could not fetch function");
                    }
                }

                let vendor_id_struct = VendorId::new(vendor_id);
                let device_id_struct = DeviceId::new(device_id);
                let mut vendor_name: &str = "Unknown Vendor";
                let mut device_name: &str = "Unknown Device";

                if let Some(vendor) = db.find_vendor(vendor_id_struct) {
                    vendor_name = vendor.name();
                    if let Some(device) = vendor.find_device(device_id_struct) {
                        device_name = device.name();
                    }
                }

                // Print summary for this device
                kprintln!("  === Device Summary ===");
                kprintln!("    Vendor: {}, Device: {}", vendor_name.to_string(), device_name.to_string());
                kprintln!("    Type: {}", pci_type);
                kprintln!("    Hardware: (class: {:#04x})", class_code);
                kprintln!("  =====================");
            }
        }
    }

    pci
}

// To use PCI, use [`PciAccess::new_pci`].
// To use PCIe, use [`PciAccess::new_pcie`].
//
// Then you can scan buses.
// For each bus, you can scan devices.
// For each device, you can scan functions.
// For each function, you can scan BARs, capabilities, and general info.
//
// You can also find and configure MSI (Message Signaled Interrupts)
