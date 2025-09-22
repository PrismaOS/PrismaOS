/// xHCI Host Controller Driver
///
/// This module implements a complete xHCI (eXtensible Host Controller Interface) driver
/// for USB 3.0+ host controllers. It supports all USB speeds and transfer types.

pub mod controller;
pub mod context;
pub mod ring;
pub mod trb;
pub mod registers;
pub mod transfer;

pub use controller::XhciController;
pub use context::*;
pub use ring::*;
pub use trb::*;
pub use registers::*;
pub use transfer::*;

use crate::error::{UsbError, Result};
use crate::types::*;

/// xHCI controller capability parameters
#[derive(Debug, Clone, Copy)]
pub struct XhciCapabilities {
    /// Capability register length
    pub cap_length: u8,
    /// HCI version
    pub hci_version: u16,
    /// Structural parameters 1
    pub hcsparams1: u32,
    /// Structural parameters 2
    pub hcsparams2: u32,
    /// Structural parameters 3
    pub hcsparams3: u32,
    /// Capability parameters 1
    pub hccparams1: u32,
    /// Doorbell offset
    pub doorbell_offset: u32,
    /// Runtime register space offset
    pub runtime_offset: u32,
    /// Capability parameters 2
    pub hccparams2: u32,
}

impl XhciCapabilities {
    /// Get the number of device slots
    pub fn max_device_slots(&self) -> u8 {
        (self.hcsparams1 & 0xff) as u8
    }

    /// Get the number of interrupters
    pub fn max_interrupters(&self) -> u16 {
        ((self.hcsparams1 >> 8) & 0x7ff) as u16
    }

    /// Get the number of ports
    pub fn max_ports(&self) -> u8 {
        ((self.hcsparams1 >> 24) & 0xff) as u8
    }

    /// Check if 64-bit addressing is supported
    pub fn supports_64bit(&self) -> bool {
        (self.hccparams1 & 0x01) != 0
    }

    /// Check if bandwidth negotiation is supported
    pub fn supports_bandwidth_negotiation(&self) -> bool {
        (self.hccparams1 & 0x02) != 0
    }

    /// Check if context size is 64 bytes (vs 32 bytes)
    pub fn context_size_64(&self) -> bool {
        (self.hccparams1 & 0x04) != 0
    }

    /// Get the page size
    pub fn page_size(&self, pagesize_reg: u32) -> u32 {
        4096 << (pagesize_reg.trailing_zeros())
    }
}

/// xHCI port status and control
#[derive(Debug, Clone, Copy)]
pub struct XhciPortStatus {
    /// Raw PORTSC register value
    pub portsc: u32,
}

impl XhciPortStatus {
    pub fn new(portsc: u32) -> Self {
        Self { portsc }
    }

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        (self.portsc & 0x01) != 0
    }

    /// Check if port is enabled
    pub fn is_enabled(&self) -> bool {
        (self.portsc & 0x02) != 0
    }

    /// Check if over-current is active
    pub fn is_over_current(&self) -> bool {
        (self.portsc & 0x08) != 0
    }

    /// Check if port is reset
    pub fn is_reset(&self) -> bool {
        (self.portsc & 0x10) != 0
    }

    /// Get the port power state
    pub fn power_state(&self) -> u8 {
        ((self.portsc >> 5) & 0x0f) as u8
    }

    /// Get the device speed
    pub fn speed(&self) -> Option<UsbSpeed> {
        match (self.portsc >> 10) & 0x0f {
            0 => None, // No device
            1 => Some(UsbSpeed::Full),
            2 => Some(UsbSpeed::Low),
            3 => Some(UsbSpeed::High),
            4 => Some(UsbSpeed::Super),
            5 => Some(UsbSpeed::SuperPlus),
            _ => None,
        }
    }

    /// Check if connect status changed
    pub fn connect_status_changed(&self) -> bool {
        (self.portsc & 0x20000) != 0
    }

    /// Check if port enabled/disabled changed
    pub fn port_enabled_changed(&self) -> bool {
        (self.portsc & 0x40000) != 0
    }

    /// Check if warm reset changed
    pub fn warm_reset_changed(&self) -> bool {
        (self.portsc & 0x80000) != 0
    }

    /// Check if over-current changed
    pub fn over_current_changed(&self) -> bool {
        (self.portsc & 0x100000) != 0
    }

    /// Check if port reset changed
    pub fn port_reset_changed(&self) -> bool {
        (self.portsc & 0x200000) != 0
    }

    /// Check if port link state changed
    pub fn link_state_changed(&self) -> bool {
        (self.portsc & 0x400000) != 0
    }

    /// Check if port config error changed
    pub fn config_error_changed(&self) -> bool {
        (self.portsc & 0x800000) != 0
    }

    /// Get all change bits
    pub fn change_bits(&self) -> u32 {
        self.portsc & 0xfe0000
    }

    /// Create PORTSC value to clear change bits
    pub fn clear_change_bits(&self) -> u32 {
        (self.portsc & !0xfe0000) | self.change_bits()
    }

    /// Create PORTSC value to enable the port
    pub fn enable_port(&self) -> u32 {
        self.portsc | 0x02
    }

    /// Create PORTSC value to reset the port
    pub fn reset_port(&self) -> u32 {
        (self.portsc & !0x02) | 0x10
    }

    /// Create PORTSC value to set port power
    pub fn set_port_power(&self, power: bool) -> u32 {
        if power {
            self.portsc | 0x100
        } else {
            self.portsc & !0x100
        }
    }
}