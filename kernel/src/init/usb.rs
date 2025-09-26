use lib_kernel::{consts::BASE_REVISION, kprintln, scrolling_text};

/// Initialize USB controllers found via PCI enumeration
pub fn init_usb() {
    use ez_pci::PciAccess;

    kprintln!("Scanning for USB controllers...");

    let mut pci = unsafe { PciAccess::new_pci() };
    let busses = pci.known_buses();
    let mut usb_controllers_found = 0;

    if usb_controllers_found == 0 {
        kprintln!("No supported USB controllers found");
    } else {
        kprintln!("Initialized {} USB controller(s)", usb_controllers_found);
    }

    for bus in busses {
        let mut specific_bus = pci.bus(bus);
        for device_num in 0..32 {
            if let Some(mut device) = specific_bus.device(device_num) {
                let functions = device.possible_functions();

                for function in functions {
                    if let Some(mut pci_fn) = device.function(function) {
                        let class_code = pci_fn.class_code();
                        let subclass = pci_fn.sub_class();
                        let prog_if = pci_fn.prog_if();

                        // USB controllers have class code 0x0C (Serial Bus Controller)
                        // and subclass 0x03 (USB controller)
                        if class_code == 0x0C && subclass == 0x03 {
                            let vendor_id = pci_fn.vendor_id();
                            let device_id = pci_fn.device_id();

                            kprintln!("Found USB controller: Bus {}, Function {}", bus, function);
                            kprintln!("  Vendor: {:#06x}, Device: {:#06x}", vendor_id, device_id);
                            kprintln!(
                                "  Class: {:#04x}, Subclass: {:#04x}, Prog IF: {:#04x}",
                                class_code,
                                subclass,
                                prog_if
                            );

                            // Check if this is an xHCI controller (prog_if 0x30)
                            if prog_if == 0x30 {
                                kprintln!("  Type: xHCI (USB 3.0) controller");

                                // Get BAR0 for MMIO base address
                                // Note: BAR access method depends on ez_pci crate API
                                // For now, we'll use a placeholder address
                                //let _mmio_base = 0xFE000000; // Placeholder MMIO base
                                // For now, we'll just log the USB controller discovery
                                // Full USB initialization requires async runtime setup
                                kprintln!("  USB controller found - initialization deferred");
                                kprintln!("  Note: USB initialization will be added in future kernel updates");
                                usb_controllers_found += 1;
                            } else if prog_if == 0x20 {
                                kprintln!("  Type: EHCI (USB 2.0) controller - not supported yet");
                            } else if prog_if == 0x10 {
                                kprintln!("  Type: OHCI (USB 1.1) controller - not supported yet");
                            } else if prog_if == 0x00 {
                                kprintln!("  Type: UHCI (USB 1.1) controller - not supported yet");
                            } else {
                                kprintln!(
                                    "  Type: Unknown USB controller (prog_if: {:#04x})",
                                    prog_if
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
