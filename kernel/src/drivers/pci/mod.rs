use crate::kprintln;
use ez_pci::PciAccess;

pub fn init_pci() -> PciAccess {
    let mut pci = unsafe {
        PciAccess::new_pci()
    };
    
    let busses = pci.known_buses();

    for bus in busses {
        kprintln!("Found bus: {}", bus);
        let specific_bus = pci.bus(bus);
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
