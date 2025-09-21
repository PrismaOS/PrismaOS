use crate::kprintln;
use ez_pci::PciAccess;

pub fn init_pci() -> PciAccess {
    let mut pci = unsafe {
        PciAccess::new_pci()
    };
    
    let busses = pci.known_buses();

    for bus in busses {
        kprintln!("Found bus: {}", bus);
        let mut specific_bus = pci.bus(bus);
        let mut device = specific_bus.device(bus).expect("Failed te fetch PCI device");
        let functions = device.possible_functions();

        // Try to get vendor/device ID and class from function 0 (common for all functions)
        let fn0 = device.function(0);
        let (vendor_id, device_id, class_code) = if let Some(mut fn0) = fn0 {
            let vendor_id = fn0.vendor_id();
            let device_id = fn0.device_id();
            let class_code = fn0.class_code();
            kprintln!("  Vendor ID: {:#06x}, Device ID: {:#06x}", vendor_id, device_id);
            (vendor_id, device_id, class_code)
        } else {
            (0, 0, 0)
        };

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

        // Guess hardware type from class code
        let hw_type = match class_code {
            0x01 => "Mass Storage Controller",
            0x02 => "Network Controller",
            0x03 => "Display Controller",
            0x04 => "Multimedia Controller",
            0x05 => "Memory Controller",
            0x06 => "Bridge Device",
            0x07 => "Simple Communication Controller",
            0x08 => "Base System Peripheral",
            0x09 => "Input Device Controller",
            0x0A => "Docking Station",
            0x0B => "Processor",
            0x0C => "Serial Bus Controller",
            _ => "Unknown/Other Device",
        };
        kprintln!("  Hardware Type: {} (class: {:#04x})", hw_type, class_code);

        // Optionally, print all functions and their capabilities count
        for function in functions {
            kprintln!("    function: {}", function);
            if let Some(mut pci_fn) = device.function(function) { // Stop being afk you bum
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

        // Print summary for this device
        kprintln!("  === Device Summary ===");
        kprintln!("    Vendor: {:#06x}, Device: {:#06x}", vendor_id, device_id);
        kprintln!("    Type: {}", pci_type);
        kprintln!("    Hardware: {} (class: {:#04x})", hw_type, class_code);
        kprintln!("  =====================");
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
