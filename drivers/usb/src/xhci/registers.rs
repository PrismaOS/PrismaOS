/// xHCI Register Definitions and Access
///
/// This module defines all xHCI registers and provides safe access methods.

use volatile::Volatile;
use crate::types::UsbSpeed;

/// xHCI Capability Registers
#[repr(C)]
pub struct CapabilityRegisters {
    /// Capability Register Length and Reserved
    pub caplength_reserved: Volatile<u16>,
    /// Interface Version Number
    pub hciversion: Volatile<u16>,
    /// Structural Parameters 1
    pub hcsparams1: Volatile<u32>,
    /// Structural Parameters 2
    pub hcsparams2: Volatile<u32>,
    /// Structural Parameters 3
    pub hcsparams3: Volatile<u32>,
    /// Capability Parameters 1
    pub hccparams1: Volatile<u32>,
    /// Doorbell Offset
    pub dboff: Volatile<u32>,
    /// Runtime Register Space Offset
    pub rtsoff: Volatile<u32>,
    /// Capability Parameters 2
    pub hccparams2: Volatile<u32>,
}

impl CapabilityRegisters {
    /// Get the capability register length
    pub fn cap_length(&self) -> u8 {
        (unsafe { self.caplength_reserved.read() } & 0xff) as u8
    }

    /// Get the HCI version
    pub fn hci_version(&self) -> u16 {
        unsafe { self.hciversion.read() }
    }

    /// Get the maximum number of device slots
    pub fn max_device_slots(&self) -> u8 {
        (unsafe { self.hcsparams1.read() } & 0xff) as u8
    }

    /// Get the maximum number of interrupters
    pub fn max_interrupters(&self) -> u16 {
        ((unsafe { self.hcsparams1.read() } >> 8) & 0x7ff) as u16
    }

    /// Get the maximum number of ports
    pub fn max_ports(&self) -> u8 {
        ((unsafe { self.hcsparams1.read() } >> 24) & 0xff) as u8
    }

    /// Check if 64-bit addressing is supported
    pub fn supports_64bit(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x01) != 0
    }

    /// Check if bandwidth negotiation capability is supported
    pub fn supports_bnc(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x02) != 0
    }

    /// Check if context size is 64 bytes
    pub fn context_size_64(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x04) != 0
    }

    /// Check if port power control is supported
    pub fn supports_ppc(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x08) != 0
    }

    /// Check if port indicators are supported
    pub fn supports_pind(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x10) != 0
    }

    /// Check if light HC reset capability is supported
    pub fn supports_lhrc(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x20) != 0
    }

    /// Check if latency tolerance messaging is supported
    pub fn supports_ltm(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x40) != 0
    }

    /// Check if no secondary SID support
    pub fn no_secondary_sid(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x80) != 0
    }

    /// Get the parse all event data flag
    pub fn parse_all_event_data(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x100) != 0
    }

    /// Get the stopped short packet capability
    pub fn supports_spc(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x200) != 0
    }

    /// Get the stopped EDTLA capability
    pub fn supports_sec(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x400) != 0
    }

    /// Get the contiguous frame ID capability
    pub fn supports_cfc(&self) -> bool {
        (unsafe { self.hccparams1.read() } & 0x800) != 0
    }

    /// Get the maximum primary stream array size
    pub fn max_primary_stream_array_size(&self) -> u8 {
        ((unsafe { self.hccparams1.read() } >> 12) & 0x0f) as u8
    }

    /// Get the xHCI extended capabilities pointer
    pub fn xecp(&self) -> u16 {
        ((unsafe { self.hccparams1.read() } >> 16) & 0xffff) as u16
    }

    /// Get the doorbell offset
    pub fn doorbell_offset(&self) -> u32 {
        unsafe { self.dboff.read() } & !0x03
    }

    /// Get the runtime register space offset
    pub fn runtime_offset(&self) -> u32 {
        unsafe { self.rtsoff.read() } & !0x1f
    }
}

/// USB Command Register bits
pub mod usbcmd {
    pub const RUN_STOP: u32 = 1 << 0;
    pub const HCRST: u32 = 1 << 1;
    pub const INTE: u32 = 1 << 2;
    pub const HSEE: u32 = 1 << 3;
    pub const LHCRST: u32 = 1 << 7;
    pub const CSS: u32 = 1 << 8;
    pub const CRS: u32 = 1 << 9;
    pub const EWE: u32 = 1 << 10;
    pub const EU3S: u32 = 1 << 11;
}

/// USB Status Register bits
pub mod usbsts {
    pub const HCH: u32 = 1 << 0;
    pub const HSE: u32 = 1 << 2;
    pub const EINT: u32 = 1 << 3;
    pub const PCD: u32 = 1 << 4;
    pub const SSS: u32 = 1 << 8;
    pub const RSS: u32 = 1 << 9;
    pub const SRE: u32 = 1 << 10;
    pub const CNR: u32 = 1 << 11;
    pub const HCE: u32 = 1 << 12;
}

/// Port Status and Control Register bits
pub mod portsc {
    pub const CCS: u32 = 1 << 0;    // Current Connect Status
    pub const PED: u32 = 1 << 1;    // Port Enabled/Disabled
    pub const OCA: u32 = 1 << 3;    // Over-current Active
    pub const PR: u32 = 1 << 4;     // Port Reset
    pub const PP: u32 = 1 << 9;     // Port Power
    pub const LWS: u32 = 1 << 16;   // Port Link State Write Strobe
    pub const CSC: u32 = 1 << 17;   // Connect Status Change
    pub const PEC: u32 = 1 << 18;   // Port Enabled/Disabled Change
    pub const WRC: u32 = 1 << 19;   // Warm Port Reset Change
    pub const OCC: u32 = 1 << 20;   // Over-current Change
    pub const PRC: u32 = 1 << 21;   // Port Reset Change
    pub const PLC: u32 = 1 << 22;   // Port Link State Change
    pub const CEC: u32 = 1 << 23;   // Port Config Error Change

    pub const CHANGE_BITS: u32 = CSC | PEC | WRC | OCC | PRC | PLC | CEC;
}

/// Interrupter Management Register bits
pub mod iman {
    pub const IP: u32 = 1 << 0;  // Interrupt Pending
    pub const IE: u32 = 1 << 1;  // Interrupt Enable
}

/// xHCI Operational Registers
#[repr(C)]
pub struct OperationalRegisters {
    /// USB Command Register
    pub usbcmd: Volatile<u32>,
    /// USB Status Register
    pub usbsts: Volatile<u32>,
    /// Page Size Register
    pub pagesize: Volatile<u32>,
    /// Reserved
    _reserved1: [u32; 2],
    /// Device Notification Control Register
    pub dnctrl: Volatile<u32>,
    /// Command Ring Control Register (64-bit)
    pub crcr: Volatile<u64>,
    /// Reserved
    _reserved2: [u32; 4],
    /// Device Context Base Address Array Pointer (64-bit)
    pub dcbaap: Volatile<u64>,
    /// Configure Register
    pub config: Volatile<u32>,
}

impl OperationalRegisters {

    /// Start the host controller
    pub fn start(&mut self) {
        let mut cmd = unsafe { self.usbcmd.read() };
        cmd |= super::usbcmd::RUN_STOP;
        unsafe { self.usbcmd.write(cmd) };
    }

    /// Stop the host controller
    pub fn stop(&mut self) {
        let mut cmd = unsafe { self.usbcmd.read() };
        cmd &= !super::usbcmd::RUN_STOP;
        unsafe { self.usbcmd.write(cmd) };
    }

    /// Reset the host controller
    pub fn reset(&mut self) {
        let mut cmd = unsafe { self.usbcmd.read() };
        cmd |= super::usbcmd::HCRST;
        unsafe { self.usbcmd.write(cmd) };
    }

    /// Enable interrupts
    pub fn enable_interrupts(&mut self) {
        let mut cmd = unsafe { self.usbcmd.read() };
        cmd |= super::usbcmd::INTE;
        unsafe { self.usbcmd.write(cmd) };
    }

    /// Check if controller is halted
    pub fn is_halted(&self) -> bool {
        (unsafe { self.usbsts.read() } & super::usbsts::HCH) != 0
    }

    /// Check if controller is running
    pub fn is_running(&self) -> bool {
        !self.is_halted()
    }

    /// Check if controller not ready
    pub fn is_controller_not_ready(&self) -> bool {
        (unsafe { self.usbsts.read() } & super::usbsts::CNR) != 0
    }

    /// Clear status bits
    pub fn clear_status(&mut self, bits: u32) {
        unsafe { self.usbsts.write(bits) };
    }

    /// Set the command ring control register
    pub fn set_command_ring(&mut self, address: u64, ring_cycle_state: bool) {
        let mut crcr = address & !0x3f; // Clear lower 6 bits
        if ring_cycle_state {
            crcr |= 1; // Ring Cycle State
        }
        unsafe { self.crcr.write(crcr) };
    }

    /// Set the device context base address array pointer
    pub fn set_dcbaap(&mut self, address: u64) {
        unsafe { self.dcbaap.write(address & !0x3f) }; // Must be 64-byte aligned
    }

    /// Set the maximum device slots enabled
    pub fn set_max_device_slots(&mut self, slots: u8) {
        let mut config = unsafe { self.config.read() };
        config = (config & !0xff) | (slots as u32);
        unsafe { self.config.write(config) };
    }

    /// Get the page size
    pub fn get_page_size(&self) -> u32 {
        let pagesize = unsafe { self.pagesize.read() };
        4096 << pagesize.trailing_zeros()
    }
}

/// xHCI Port Register Set
#[repr(C)]
pub struct PortRegisterSet {
    /// Port Status and Control Register
    pub portsc: Volatile<u32>,
    /// Port Power Management Status and Control Register
    pub portpmsc: Volatile<u32>,
    /// Port Link Info Register
    pub portli: Volatile<u32>,
    /// Port Hardware LPM Control Register
    pub porthlpmc: Volatile<u32>,
}

impl PortRegisterSet {

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        (unsafe { self.portsc.read() } & super::portsc::CCS) != 0
    }

    /// Check if port is enabled
    pub fn is_enabled(&self) -> bool {
        (unsafe { self.portsc.read() } & super::portsc::PED) != 0
    }

    /// Check if port is powered
    pub fn is_powered(&self) -> bool {
        (unsafe { self.portsc.read() } & super::portsc::PP) != 0
    }

    /// Get the port speed
    pub fn get_speed(&self) -> Option<UsbSpeed> {
        let portsc = unsafe { self.portsc.read() };
        match (portsc >> 10) & 0x0f {
            0 => None,
            1 => Some(UsbSpeed::Full),
            2 => Some(UsbSpeed::Low),
            3 => Some(UsbSpeed::High),
            4 => Some(UsbSpeed::Super),
            5 => Some(UsbSpeed::SuperPlus),
            _ => None,
        }
    }

    /// Get port link state
    pub fn get_link_state(&self) -> u8 {
        ((unsafe { self.portsc.read() } >> 5) & 0x0f) as u8
    }

    /// Check for any change events
    pub fn has_changes(&self) -> bool {
        (unsafe { self.portsc.read() } & super::portsc::CHANGE_BITS) != 0
    }

    /// Get all change bits
    pub fn get_changes(&self) -> u32 {
        unsafe { self.portsc.read() } & super::portsc::CHANGE_BITS
    }

    /// Clear change bits
    pub fn clear_changes(&mut self) {
        let portsc = unsafe { self.portsc.read() };
        let clear_value = (portsc & !super::portsc::CHANGE_BITS) | (portsc & super::portsc::CHANGE_BITS);
        unsafe { self.portsc.write(clear_value) };
    }

    /// Set port power
    pub fn set_power(&mut self, power: bool) {
        let mut portsc = unsafe { self.portsc.read() };
        portsc &= !(super::portsc::CHANGE_BITS | super::portsc::PED); // Preserve RW1C bits
        if power {
            portsc |= super::portsc::PP;
        } else {
            portsc &= !super::portsc::PP;
        }
        unsafe { self.portsc.write(portsc) };
    }

    /// Reset the port
    pub fn reset(&mut self) {
        let mut portsc = unsafe { self.portsc.read() };
        portsc &= !(super::portsc::CHANGE_BITS | super::portsc::PED); // Preserve RW1C bits
        portsc |= super::portsc::PR;
        unsafe { self.portsc.write(portsc) };
    }

    /// Enable the port
    pub fn enable(&mut self) {
        let mut portsc = unsafe { self.portsc.read() };
        portsc &= !super::portsc::CHANGE_BITS; // Preserve RW1C bits
        portsc |= super::portsc::PED;
        unsafe { self.portsc.write(portsc) };
    }
}

/// xHCI Runtime Registers
#[repr(C)]
pub struct RuntimeRegisters {
    /// Microframe Index Register
    pub mfindex: Volatile<u32>,
    /// Reserved
    _reserved: [u32; 7],
    /// Interrupter Register Sets (up to 1024)
    pub interrupters: [InterrupterRegisterSet; 1024],
}

/// xHCI Interrupter Register Set
#[repr(C)]
pub struct InterrupterRegisterSet {
    /// Interrupter Management Register
    pub iman: Volatile<u32>,
    /// Interrupter Moderation Register
    pub imod: Volatile<u32>,
    /// Event Ring Segment Table Size Register
    pub erstsz: Volatile<u32>,
    /// Reserved
    _reserved: u32,
    /// Event Ring Segment Table Base Address Register (64-bit)
    pub erstba: Volatile<u64>,
    /// Event Ring Dequeue Pointer Register (64-bit)
    pub erdp: Volatile<u64>,
}

impl InterrupterRegisterSet {

    /// Enable interrupts for this interrupter
    pub fn enable_interrupts(&mut self) {
        let mut iman = unsafe { self.iman.read() };
        iman |= super::iman::IE;
        unsafe { self.iman.write(iman) };
    }

    /// Disable interrupts for this interrupter
    pub fn disable_interrupts(&mut self) {
        let mut iman = unsafe { self.iman.read() };
        iman &= !super::iman::IE;
        unsafe { self.iman.write(iman) };
    }

    /// Check if interrupt is pending
    pub fn is_interrupt_pending(&self) -> bool {
        (unsafe { self.iman.read() } & super::iman::IP) != 0
    }

    /// Clear interrupt pending
    pub fn clear_interrupt_pending(&mut self) {
        let mut iman = unsafe { self.iman.read() };
        iman |= super::iman::IP; // Write 1 to clear
        unsafe { self.iman.write(iman) };
    }

    /// Set the event ring segment table
    pub fn set_event_ring_segment_table(&mut self, base_address: u64, size: u16) {
        unsafe { self.erstba.write(base_address & !0x3f) }; // Must be 64-byte aligned
        unsafe { self.erstsz.write(size as u32) };
    }

    /// Set the event ring dequeue pointer
    pub fn set_event_ring_dequeue_pointer(&mut self, address: u64) {
        unsafe { self.erdp.write(address & !0x0f) }; // Must be 16-byte aligned
    }

    /// Update the event ring dequeue pointer
    pub fn update_erdp(&mut self, address: u64, clear_ehb: bool) {
        let mut erdp = address & !0x0f;
        if clear_ehb {
            erdp |= 0x08; // Event Handler Busy bit
        }
        unsafe { self.erdp.write(erdp) };
    }
}

/// xHCI Doorbell Register
#[repr(C)]
pub struct DoorbellRegister {
    /// Doorbell value
    pub doorbell: Volatile<u32>,
}

impl DoorbellRegister {
    /// Ring a doorbell for a specific endpoint
    pub fn ring(&mut self, endpoint: u8, stream_id: u16) {
        let value = (endpoint as u32) | ((stream_id as u32) << 16);
        unsafe { self.doorbell.write(value) };
    }

    /// Ring the command doorbell
    pub fn ring_command(&mut self) {
        unsafe { self.doorbell.write(0) };
    }
}