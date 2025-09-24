#![no_std]

extern crate alloc;

use lib_kernel::kprintln;
use alloc::string::{String, ToString};
use ez_pci::PciAccess;
use ids_rs::{parser::VendorBuilder, DeviceId, PciDatabase, VendorId};

pub fn init_pci() -> PciAccess {
    let db = PciDatabase::get();

    let mut pci = unsafe { PciAccess::new_pci() };
    let busses = pci.known_buses();

    for bus in busses {
        kprintln!("Found bus: {}", bus);
        let mut specific_bus = pci.bus(bus);

        for device_num in 0..32 {
            if let Some(mut device) = specific_bus.device(device_num) {
                let functions = device.possible_functions();

                for function in functions {
                    if let Some(mut pci_fn) = device.function(function) {
                        let vendor_id = pci_fn.vendor_id();
                        let device_id = pci_fn.device_id();
                        let class_code = pci_fn.class_code();

                        let vendor_id_obj = VendorId::new(vendor_id as u16);
                        let device_id_obj = DeviceId::new(device_id as u16);

                        let vendor_name = db.find_vendor(vendor_id_obj)
                            .map(|v| v.name())
                            .unwrap_or("Unknown Vendor");
                        let device_name = db.find_device(vendor_id_obj, device_id_obj)
                            .map(|d| d.name())
                            .unwrap_or("Unknown Device");

                        // Guess PCI type
                        let mut is_pcie = false;
                        let mut has_capabilities = false;
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

                        let pci_type = if is_pcie {
                            "PCI Express"
                        } else if has_capabilities {
                            "PCI"
                        } else {
                            "Legacy/Non-PCI"
                        };

                        kprintln!("  Bus {:02x}, Device {:02x}, Function {:x}:", bus, device_num, function);
                        kprintln!("    Vendor: {} ({:#06x}), Device: {} ({:#06x})", vendor_name, vendor_id, device_name, device_id);
                        kprintln!("    Type: {}", pci_type);
                        kprintln!("    Class Code: {:#04x}", class_code);

                        if let Some(caps) = pci_fn.capabilities() {
                            let cap_count = caps.count();
                            kprintln!("    Number of capabilities: {}", cap_count);
                        } else {
                            kprintln!("    Could not fetch capabilities");
                        }
                    }
                }
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