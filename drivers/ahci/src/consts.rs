// ahci_defs.rs
// Low-level AHCI / SATA layout definitions (repr(C)).
// Based on AHCI 1.3 and commonly-used OSDev references.
// This file contains only layout and constants. Do NOT use these
// structs directly for safe multi-threaded access; use volatile
// accesses and safe wrappers in driver code.

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::marker::PhantomData;

pub type u8_  = u8;
pub type u16_ = u16;
pub type u32_ = u32;
pub type u64_ = u64;

/// Maximums & sizes
pub const HBA_MAX_PORTS: usize = 32;
pub const HBA_PORT_SIZE: usize = 0x80;
pub const HBA_CMD_HEADER_SIZE: usize = 32;
pub const HBA_PRDT_ENTRY_SIZE: usize = 16;
pub const HBA_CMD_TBL_HEADER: usize = 128; // cfis(64) + acmd(16) + rsv(48)
pub const HBA_FIS_SIZE: usize = 256;

/// ---------------------------------------------------------------------------
/// HBA Memory (ABAR) layout: Generic Host Control + ports
/// ---------------------------------------------------------------------------
#[repr(C)]
pub struct HbaMem {
    // 0x00 - 0x2B Generic Host Control
    pub cap:  u32_,      // 0x00 Host capability
    pub ghc:  u32_,      // 0x04 Global host control
    pub is:   u32_,      // 0x08 Interrupt status
    pub pi:   u32_,      // 0x0C Ports implemented (bitmask)
    pub vs:   u32_,      // 0x10 Version register
    pub ccc_ctl: u32_,   // 0x14 Command completion coalescing control
    pub ccc_pts: u32_,   // 0x18 Command completion coalescing ports
    pub em_loc: u32_,    // 0x1C Enclosure management location
    pub em_ctl: u32_,    // 0x20 Enclosure management control
    pub cap2: u32_,      // 0x24 Host capabilities extended
    pub bohc: u32_,      // 0x28 BIOS/OS handoff control and status
    pub rsv: [u8; 0x74], // 0x2C - 0x9F reserved
    pub vendor: [u8; 0x60], // 0xA0 - 0xFF vendor specific
    // Ports begin at offset 0x100. Do not include ports array here because
    // number of ports is dynamic. Use `port_at`.
    _phantom: PhantomData<u8>,
}

/// Per-port registers (0x80 bytes)
#[repr(C)]
pub struct HbaPort {
    pub clb:  u32_,   // 0x00 Command list base address (1K aligned)
    pub clbu: u32_,   // 0x04 Command list base address upper 32 bits
    pub fb:   u32_,   // 0x08 FIS base address (256 byte aligned)
    pub fbu:  u32_,   // 0x0C FIS base address upper 32 bits
    pub is:   u32_,   // 0x10 Interrupt status
    pub ie:   u32_,   // 0x14 Interrupt enable
    pub cmd:  u32_,   // 0x18 Command and status
    pub rsv0: u32_,   // 0x1C Reserved
    pub tfd:  u32_,   // 0x20 Task file data
    pub sig:  u32_,   // 0x24 Signature
    pub ssts: u32_,   // 0x28 SATA status (SStatus)
    pub sctl: u32_,   // 0x2C SATA control (SControl)
    pub serr: u32_,   // 0x30 SATA error (SError)
    pub sact: u32_,   // 0x34 SATA active (SActive)
    pub ci:   u32_,   // 0x38 Command issue
    pub sntf: u32_,   // 0x3C SATA notification (SNotification)
    pub fbs:  u32_,   // 0x40 FIS-based switching control
    pub rsv1: [u8; 0x3C], // 0x44 - 0x7F reserved
}

/// ---------------------------------------------------------------------------
/// Command List / Command Header / PRDT / Command Table
/// ---------------------------------------------------------------------------

/// Command header (one per command slot) - 32 bytes (DW0..DW7)
/// Note: fields like cfl/a/w/p are packed into a u16 'flags' for simplicity.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct HbaCmdHeader {
    pub flags: u16,    // DW0 low: cfl(5) | a(1) | w(1) | p(1) | ... for convenience
    pub prdtl: u16,    // DW0 high: PRDT entry count
    pub prdbc: u32_,   // DW1: PRD byte count transferred
    pub ctba: u32_,    // DW2: Command table base address
    pub ctbau: u32_,   // DW3: Command table base address upper 32 bits
    pub rsv: [u32_; 4],// DW4-7 reserved (16 bytes)
}
impl HbaCmdHeader {
    // helpers to interpret flags bits can be added in safe wrapper code
}

/// PRDT entry - 16 bytes
#[repr(C)]
#[derive(Copy, Clone)]
pub struct HbaPrdtEntry {
    pub dba:  u32_, // Data base address
    pub dbau: u32_, // Data base address upper 32 bits
    pub rsv:  u32_, // Reserved
    pub dbc:  u32_, // Data byte count (22 bits) + interrupt on completion (1 bit)
}

/// Command Table (Command FIS + ATAPI + reserved + PRDT entries)
/// The PRDT array length is variable (0..65535 entries). The struct below
/// represents the header; allocate (128 + prdt_count*16) bytes for a full table.
#[repr(C)]
pub struct HbaCmdTbl {
    pub cfis: [u8; 64],  // command FIS
    pub acmd: [u8; 16],  // ATAPI command, 12/16 bytes
    pub rsv:  [u8; 48],  // reserved
    // followed by PRDT entries: HbaPrdtEntry prdt_entry[]
    // NOTE: can't express flexible array in Rust; allocate manually.
}

/// ---------------------------------------------------------------------------
/// FIS (Frame Information Structure) layouts
/// (packed to ensure exact in-memory layout)
/// ---------------------------------------------------------------------------

/// FIS types (values)
pub const FIS_TYPE_REG_H2D: u8 = 0x27; // Register – host to device
pub const FIS_TYPE_REG_D2H: u8 = 0x34; // Register – device to host
pub const FIS_TYPE_DMA_ACT: u8 = 0x39; // DMA activate – device to host
pub const FIS_TYPE_DMA_SETUP: u8 = 0x41; // DMA setup – bidirectional
pub const FIS_TYPE_DATA: u8 = 0x46;    // Data – bidirectional
pub const FIS_TYPE_BIST: u8 = 0x58;    // BIST activate – bidirectional
pub const FIS_TYPE_PIO_SETUP: u8 = 0x5F; // PIO setup – device to host
pub const FIS_TYPE_DEV_BITS: u8 = 0xA1;  // Set device bits FIS – device to host

/// Register - Host to Device FIS (command)
#[repr(C, packed)]
pub struct FisRegH2D {
    pub fis_type: u8,   // FIS_TYPE_REG_H2D
    pub pmport_c: u8,   // pmport:4 | rsv0:3 | c:1
    pub command: u8,    // ATA command
    pub featurel: u8,

    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,

    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub featureh: u8,

    pub countl: u8,
    pub counth: u8,
    pub icc: u8,
    pub control: u8,

    pub rsv: [u8; 4],
}
impl FisRegH2D {
    /// helper: set c bit in pmport_c to indicate command (c=1)
    pub fn set_cmd(&mut self) { self.pmport_c |= 0x01 << 7; }
}

/// Register - Device to Host FIS (status)
#[repr(C, packed)]
pub struct FisRegD2H {
    pub fis_type: u8,   // FIS_TYPE_REG_D2H
    pub pmport_i: u8,   // pmport:4 | rsv0:2 | i:1 | rsv1:1
    pub status: u8,
    pub error: u8,

    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,

    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub rsv2: u8,

    pub countl: u8,
    pub counth: u8,
    pub rsv3: [u8; 2],

    pub rsv4: [u8; 4],
}

/// Data FIS (payload)
#[repr(C, packed)]
pub struct FisData {
    pub fis_type: u8, // FIS_TYPE_DATA
    pub pmport_rsv: u8, // pmport:4 | rsv:4
    pub rsv1: [u8; 2],
    // data follows here (variable)
    // stored as bytes in the PRDT target buffer
}

/// PIO Setup FIS (device -> host)
#[repr(C, packed)]
pub struct FisPioSetup {
    pub fis_type: u8, // FIS_TYPE_PIO_SETUP
    pub pmport_d_i: u8, // pmport:4 | rsv0:1 | d:1 | i:1 | rsv1:1
    pub status: u8,
    pub error: u8,

    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,

    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub rsv2: u8,

    pub countl: u8,
    pub counth: u8,
    pub rsv3: u8,
    pub e_status: u8,

    pub tc: u16,
    pub rsv4: [u8; 2],
}

/// DMA Setup FIS
/// Note: DMA setup FIS contained DMA buffer descriptor and etc.
/// Layout below is minimal; the AHCI spec treats the DMA buffer ID as host-specific.
#[repr(C, packed)]
pub struct FisDmaSetup {
    pub fis_type: u8, // FIS_TYPE_DMA_SETUP
    pub pmport_d_i_a: u8, // pmport:4 | rsv0:1 | d:1 | i:1 | a:1
    pub rsv1: [u8; 2],
    pub dma_buffer_id_low: u32,
    pub dma_buffer_id_high: u32,
    pub rsv2: u32,
    pub dma_buffer_offset: u32, // bits 31:2, lower two bits must be zero
    pub transfer_count: u32,    // bits 31:0 (bit0 must be zero)
    pub resvd: u32,
}

/// Set Device Bits FIS
#[repr(C, packed)]
pub struct FisDevBits {
    pub fis_type: u8, // FIS_TYPE_DEV_BITS
    pub pmport_rsv: u8,
    pub rsv: [u8; 2],
    pub status: u8,
    pub error: u8,
    pub rsv2: [u8; 10],
}

/// The Received FIS area (HBA will write FISes into this area)
/// Size: 256 bytes per port.
#[repr(C)]
pub struct HbaFis {
    pub dsfis: FisDmaSetup,     // 0x00 DMA setup FIS
    pub pad0: [u8; 4],
    pub psfis: FisPioSetup,     // 0x20 PIO setup FIS
    pub pad1: [u8; 12],
    pub rfis: FisRegD2H,        // 0x40 Reg Device->Host FIS
    pub pad2: [u8; 4],
    pub sdbfis: FisDevBits,     // 0x58 Set Device Bits FIS
    pub ufis: [u8; 64],         // 0x60 - 0x9F Unknown FIS area (UFIS)
    pub rsv: [u8; 0x60],        // 0xA0 - 0xFF reserved
}

/// ---------------------------------------------------------------------------
/// AHCI / ATA constants & flags
/// ---------------------------------------------------------------------------

/* HBA Global Host Control (GHC) bits */
pub const HBA_GHC_HR: u32 = 1 << 0;   // HBA Reset
pub const HBA_GHC_IE: u32 = 1 << 1;   // Interrupt Enable
pub const HBA_GHC_AE: u32 = 1 << 31;  // AHCI Enable

/* Port command (PxCMD) bits */
pub const PORT_CMD_ST: u32 = 1 << 0;    // Start
pub const PORT_CMD_SUD: u32 = 1 << 1;   // Spin-Up Device
pub const PORT_CMD_POD: u32 = 1 << 2;   // Power On Device
pub const PORT_CMD_CLO: u32 = 1 << 3;   // Command List Override
pub const PORT_CMD_FRE: u32 = 1 << 4;   // FIS Receive Enable
pub const PORT_CMD_FR: u32  = 1 << 14;  // FIS Receive Running
pub const PORT_CMD_CR: u32  = 1 << 15;  // Command List Running

/* PxSSTS fields */
pub const SSTS_DET_MASK: u32 = 0xF;
pub const SSTS_DET_NO_DEV: u32 = 0x0;
pub const SSTS_DET_PRESENT: u32 = 0x3;

pub const SSTS_IPM_MASK: u32 = 0xF0;
pub const SSTS_IPM_NO_DEV: u32 = 0x00;
pub const SSTS_IPM_ACTIVE: u32 = 0x10;

/* Port Signature values */
pub const SATA_SIG_ATA: u32   = 0x0000_0101;
pub const SATA_SIG_ATAPI: u32 = 0xEB14_0101;
pub const SATA_SIG_SEMB: u32  = 0xC33C_0101;
pub const SATA_SIG_PM: u32    = 0x9669_0101;

/* Port Interrupt Status bits (PxIS / HBA IS) — common ones */
pub const PORT_IS_DHRS: u32 = 1 << 0;  // Device to Host Register FIS
pub const PORT_IS_PSS:  u32 = 1 << 1;  // PIO Setup FIS
pub const PORT_IS_DSS:  u32 = 1 << 2;  // DMA Setup FIS
pub const PORT_IS_SDBS: u32 = 1 << 3;  // Set Device Bits FIS
pub const PORT_IS_UFS:  u32 = 1 << 4;  // Unknown FIS
pub const PORT_IS_DPS:  u32 = 1 << 5;  // Descriptor Processed
pub const PORT_IS_PCS:  u32 = 1 << 6;  // Port Connect Change
pub const PORT_IS_DMPS: u32 = 1 << 7;  // Device Mechanical Presence
pub const PORT_IS_PRCS: u32 = 1 << 22; // PhyRdy Change Status
pub const PORT_IS_IPMS: u32 = 1 << 23; // Incorrect Port Multiplier Status
pub const PORT_IS_OFS:  u32 = 1 << 24; // Overflow Status
pub const PORT_IS_INFS: u32 = 1 << 26; // Interface Non-fatal Error
pub const PORT_IS_IFS:  u32 = 1 << 27; // Interface Fatal Error
pub const PORT_IS_HBDS: u32 = 1 << 28; // Host Bus Data Error
pub const PORT_IS_HBFS: u32 = 1 << 29; // Host Bus Fatal Error
pub const PORT_IS_TFES: u32 = 1 << 30; // Task File Error
pub const PORT_IS_CPDS: u32 = 1 << 31; // Cold Port Detect

/* ATA opcodes (commonly used) */
pub const ATA_CMD_READ_DMA: u8      = 0xC8;
pub const ATA_CMD_READ_DMA_EXT: u8  = 0x25;
pub const ATA_CMD_WRITE_DMA: u8     = 0xCA;
pub const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
pub const ATA_CMD_IDENTIFY: u8      = 0xEC;
pub const ATA_CMD_PACKET: u8        = 0xA0;
pub const ATA_CMD_ATAPI_IDENT: u8   = 0xA1;

/* ATA device status bits (Task file) */
pub const ATA_DEV_BUSY: u8 = 0x80;
pub const ATA_DEV_DRQ:  u8 = 0x08;

/* FIS types already defined earlier (FIS_TYPE_*) */

/* Misc sizes/limits */
pub const HBA_CMD_SLOT_MAX: usize = 32; // max command slots per port (AHCI supports up to 32)
pub const PRDT_MAX_BYTES: u32 = 0x400000; // PRDT byte count max (4MB) per entry specification-ish

/// ---------------------------------------------------------------------------
/// Small unsafe helper: get pointer to port `index`
///
/// Use volatile reads/writes when accessing the returned pointer.
/// `base` should be a pointer to the mapped HBA MMIO region (ABAR).
/// ---------------------------------------------------------------------------
impl HbaMem {
    /// Get pointer to port at index (0..31). Unsafe because of raw pointer math.
    /// Returns null pointer if index >= HBA_MAX_PORTS.
    pub unsafe fn port_at(base: *mut HbaMem, index: usize) -> *mut HbaPort {
        if index >= HBA_MAX_PORTS {
            core::ptr::null_mut()
        } else {
            let byte_ptr = base as *mut u8;
            byte_ptr.add(0x100 + index * HBA_PORT_SIZE) as *mut HbaPort
        }
    }
}
