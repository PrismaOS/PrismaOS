//! USB Endpoint Management

use core::sync::atomic::{AtomicU16, AtomicU8, Ordering};
use usb_device::{UsbDirection, endpoint::EndpointType as UsbEndpointType};

/// USB Endpoint Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

impl From<UsbEndpointType> for EndpointType {
    fn from(ep_type: UsbEndpointType) -> Self {
        match ep_type {
            UsbEndpointType::Control => EndpointType::Control,
            UsbEndpointType::Isochronous { .. } => EndpointType::Isochronous,
            UsbEndpointType::Bulk => EndpointType::Bulk,
            UsbEndpointType::Interrupt => EndpointType::Interrupt,
        }
    }
}

impl Into<u8> for EndpointType {
    fn into(self) -> u8 {
        match self {
            EndpointType::Control => 0,
            EndpointType::Isochronous => 1,
            EndpointType::Bulk => 2,
            EndpointType::Interrupt => 3,
        }
    }
}

/// USB Endpoint Direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointDirection {
    Out,
    In,
}

impl From<UsbDirection> for EndpointDirection {
    fn from(direction: UsbDirection) -> Self {
        match direction {
            UsbDirection::Out => EndpointDirection::Out,
            UsbDirection::In => EndpointDirection::In,
        }
    }
}

/// USB Endpoint
#[derive(Debug)]
pub struct Endpoint {
    /// Endpoint address (including direction bit)
    address: u8,
    /// Endpoint type
    endpoint_type: EndpointType,
    /// Maximum packet size
    max_packet_size: AtomicU16,
    /// Interval for polling (for interrupt/isochronous endpoints)
    interval: u8,
    /// Current toggle state
    toggle: AtomicU8,
    /// Endpoint is stalled
    stalled: AtomicU8,
}

impl Endpoint {
    /// Create a new endpoint
    pub fn new(
        number: u8,
        direction: EndpointDirection,
        endpoint_type: EndpointType,
        max_packet_size: u16,
        interval: u8,
    ) -> Self {
        let address = number | match direction {
            EndpointDirection::Out => 0x00,
            EndpointDirection::In => 0x80,
        };

        Self {
            address,
            endpoint_type,
            max_packet_size: AtomicU16::new(max_packet_size),
            interval,
            toggle: AtomicU8::new(0),
            stalled: AtomicU8::new(0),
        }
    }

    /// Get endpoint address
    pub fn address(&self) -> u8 {
        self.address
    }

    /// Get endpoint number
    pub fn number(&self) -> u8 {
        self.address & 0x7F
    }

    /// Get endpoint direction
    pub fn direction(&self) -> EndpointDirection {
        if self.address & 0x80 != 0 {
            EndpointDirection::In
        } else {
            EndpointDirection::Out
        }
    }

    /// Get endpoint type
    pub fn endpoint_type(&self) -> EndpointType {
        self.endpoint_type
    }

    /// Get maximum packet size
    pub fn max_packet_size(&self) -> u16 {
        self.max_packet_size.load(Ordering::Acquire)
    }

    /// Set maximum packet size
    pub fn set_max_packet_size(&self, size: u16) {
        self.max_packet_size.store(size, Ordering::Release);
    }

    /// Get polling interval
    pub fn interval(&self) -> u8 {
        self.interval
    }

    /// Get current toggle state
    pub fn toggle(&self) -> bool {
        self.toggle.load(Ordering::Acquire) != 0
    }

    /// Set toggle state
    pub fn set_toggle(&self, toggle: bool) {
        self.toggle.store(if toggle { 1 } else { 0 }, Ordering::Release);
    }

    /// Toggle the data toggle
    pub fn flip_toggle(&self) -> bool {
        let old = self.toggle.fetch_xor(1, Ordering::AcqRel);
        old == 0 // Return new state
    }

    /// Check if endpoint is stalled
    pub fn is_stalled(&self) -> bool {
        self.stalled.load(Ordering::Acquire) != 0
    }

    /// Set stall state
    pub fn set_stalled(&self, stalled: bool) {
        self.stalled.store(if stalled { 1 } else { 0 }, Ordering::Release);
    }

    /// Clear stall and reset toggle
    pub fn clear_stall(&self) {
        self.stalled.store(0, Ordering::Release);
        self.toggle.store(0, Ordering::Release);
    }

    /// Check if this is a control endpoint
    pub fn is_control(&self) -> bool {
        self.endpoint_type == EndpointType::Control
    }

    /// Check if this is an input endpoint
    pub fn is_in(&self) -> bool {
        self.direction() == EndpointDirection::In
    }

    /// Check if this is an output endpoint
    pub fn is_out(&self) -> bool {
        self.direction() == EndpointDirection::Out
    }

    /// Calculate the actual interval in microframes for high-speed devices
    pub fn microframe_interval(&self) -> u16 {
        match self.endpoint_type {
            EndpointType::Isochronous => {
                // For isochronous endpoints, interval is 2^(bInterval-1)
                if self.interval == 0 {
                    1
                } else {
                    1 << (self.interval - 1)
                }
            },
            EndpointType::Interrupt => {
                // For interrupt endpoints, interval is 2^(bInterval-1)
                if self.interval == 0 {
                    1
                } else {
                    1 << (self.interval - 1)
                }
            },
            _ => 0, // Control and bulk endpoints don't use intervals
        }
    }

    /// Get the wMaxPacketSize field value for descriptors
    pub fn descriptor_max_packet_size(&self) -> u16 {
        let base_size = self.max_packet_size();

        // For high-speed isochronous and interrupt endpoints,
        // bits 12-11 indicate additional transactions per microframe
        match self.endpoint_type {
            EndpointType::Isochronous | EndpointType::Interrupt => {
                // This is simplified - in practice, you'd set the multiplier
                // based on your specific requirements
                base_size
            },
            _ => base_size,
        }
    }
}

/// Endpoint Builder for easier construction
pub struct EndpointBuilder {
    number: u8,
    direction: EndpointDirection,
    endpoint_type: EndpointType,
    max_packet_size: u16,
    interval: u8,
}

impl EndpointBuilder {
    /// Create a new endpoint builder
    pub fn new(number: u8, direction: EndpointDirection, endpoint_type: EndpointType) -> Self {
        Self {
            number,
            direction,
            endpoint_type,
            max_packet_size: match endpoint_type {
                EndpointType::Control => 64,
                EndpointType::Bulk => 512,
                EndpointType::Interrupt => 64,
                EndpointType::Isochronous => 1024,
            },
            interval: match endpoint_type {
                EndpointType::Interrupt => 1,
                EndpointType::Isochronous => 1,
                _ => 0,
            },
        }
    }

    /// Set maximum packet size
    pub fn max_packet_size(mut self, size: u16) -> Self {
        self.max_packet_size = size;
        self
    }

    /// Set polling interval
    pub fn interval(mut self, interval: u8) -> Self {
        self.interval = interval;
        self
    }

    /// Build the endpoint
    pub fn build(self) -> Endpoint {
        Endpoint::new(
            self.number,
            self.direction,
            self.endpoint_type,
            self.max_packet_size,
            self.interval,
        )
    }
}

/// Helper functions for common endpoint types
impl Endpoint {
    /// Create a control endpoint (always bidirectional, endpoint 0)
    pub fn control(max_packet_size: u16) -> Self {
        Self::new(0, EndpointDirection::Out, EndpointType::Control, max_packet_size, 0)
    }

    /// Create a bulk IN endpoint
    pub fn bulk_in(number: u8, max_packet_size: u16) -> Self {
        Self::new(number, EndpointDirection::In, EndpointType::Bulk, max_packet_size, 0)
    }

    /// Create a bulk OUT endpoint
    pub fn bulk_out(number: u8, max_packet_size: u16) -> Self {
        Self::new(number, EndpointDirection::Out, EndpointType::Bulk, max_packet_size, 0)
    }

    /// Create an interrupt IN endpoint
    pub fn interrupt_in(number: u8, max_packet_size: u16, interval: u8) -> Self {
        Self::new(number, EndpointDirection::In, EndpointType::Interrupt, max_packet_size, interval)
    }

    /// Create an interrupt OUT endpoint
    pub fn interrupt_out(number: u8, max_packet_size: u16, interval: u8) -> Self {
        Self::new(number, EndpointDirection::Out, EndpointType::Interrupt, max_packet_size, interval)
    }

    /// Create an isochronous IN endpoint
    pub fn isochronous_in(number: u8, max_packet_size: u16, interval: u8) -> Self {
        Self::new(number, EndpointDirection::In, EndpointType::Isochronous, max_packet_size, interval)
    }

    /// Create an isochronous OUT endpoint
    pub fn isochronous_out(number: u8, max_packet_size: u16, interval: u8) -> Self {
        Self::new(number, EndpointDirection::Out, EndpointType::Isochronous, max_packet_size, interval)
    }
}

unsafe impl Send for Endpoint {}
unsafe impl Sync for Endpoint {}