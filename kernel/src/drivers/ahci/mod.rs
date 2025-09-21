pub mod consts;
use consts::*;

use crate::kprintln;

pub unsafe fn probe_port(abar: *mut HbaMem) {
    let pi: u32 = (*abar).pi;

    for i in 0..32 {
        // Check if port i exists
        if (pi & (1 << i)) != 0 {
            // Get pointer to port i
            let port_ptr = HbaMem::port_at(abar, i);
            if port_ptr.is_null() {
                continue; // skip if port pointer is null
            }

            // Dereference the port pointer
            let port: &HbaPort = &*port_ptr;

            // Read the SATA status
            let ssts = port.ssts;
            let det = ssts & SSTS_DET_MASK;
            let ipm = ssts & SSTS_IPM_MASK;

            // Check if a device is present and active
            if det == SSTS_DET_PRESENT && ipm == SSTS_IPM_ACTIVE {
                let sig = port.sig;
                match sig {
                    SATA_SIG_ATA => {
                        kprintln!("  SATA drive detected at port {}", i);
                    }
                    SATA_SIG_ATAPI => {
                        kprintln!("  SATAPI drive detected at port {}", i);
                    }
                    SATA_SIG_SEMB => {
                        kprintln!("  Enclosure management bridge detected at port {}", i);
                    }
                    SATA_SIG_PM => {
                        kprintln!("  Port multiplier detected at port {}", i);
                    }
                    _ => {}
                }
            }
        }
    }
}
