/// xHCI Device Context Structures
///
/// This module defines the device context data structures used by xHCI
/// to maintain state information for USB devices and endpoints.

use crate::types::*;
use crate::error::{UsbError, Result};
use core::mem;

/// Slot Context (32 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct SlotContext {
    /// Context info (route string, speed, MTT, hub, context entries)
    pub context_info: u32,
    /// Port info (max exit latency, root hub port number, number of ports)
    pub port_info: u32,
    /// TT info (TT think time, TT port number, TT hub slot ID)
    pub tt_info: u32,
    /// Device info (device address, device state)
    pub device_info: u32,
    /// Reserved
    reserved: [u32; 4],
}

impl SlotContext {
    /// Create a new slot context
    pub fn new() -> Self {
        Self {
            context_info: 0,
            port_info: 0,
            tt_info: 0,
            device_info: 0,
            reserved: [0; 4],
        }
    }

    /// Set the route string
    pub fn set_route_string(&mut self, route: u32) {
        self.context_info = (self.context_info & !0xfffff) | (route & 0xfffff);
    }

    /// Set the device speed
    pub fn set_speed(&mut self, speed: UsbSpeed) {
        let speed_value = speed.to_xhci_speed() as u32;
        self.context_info = (self.context_info & !0xf00000) | ((speed_value & 0xf) << 20);
    }

    /// Set the multi-TT flag
    pub fn set_multi_tt(&mut self, multi_tt: bool) {
        if multi_tt {
            self.context_info |= 0x2000000;
        } else {
            self.context_info &= !0x2000000;
        }
    }

    /// Set the hub flag
    pub fn set_hub(&mut self, is_hub: bool) {
        if is_hub {
            self.context_info |= 0x4000000;
        } else {
            self.context_info &= !0x4000000;
        }
    }

    /// Set the number of context entries
    pub fn set_context_entries(&mut self, entries: u8) {
        self.context_info = (self.context_info & !0xf8000000) | (((entries & 0x1f) as u32) << 27);
    }

    /// Set the maximum exit latency
    pub fn set_max_exit_latency(&mut self, latency: u16) {
        self.port_info = (self.port_info & !0xffff) | (latency as u32);
    }

    /// Set the root hub port number
    pub fn set_root_hub_port(&mut self, port: u8) {
        self.port_info = (self.port_info & !0xff0000) | (((port & 0xff) as u32) << 16);
    }

    /// Set the number of ports (for hubs)
    pub fn set_num_ports(&mut self, ports: u8) {
        self.port_info = (self.port_info & !0xff000000) | (((ports & 0xff) as u32) << 24);
    }

    /// Set the TT think time
    pub fn set_tt_think_time(&mut self, think_time: u8) {
        self.tt_info = (self.tt_info & !0x3) | ((think_time & 0x3) as u32);
    }

    /// Set the TT port number
    pub fn set_tt_port_number(&mut self, port: u8) {
        self.tt_info = (self.tt_info & !0xff00) | (((port & 0xff) as u32) << 8);
    }

    /// Set the TT hub slot ID
    pub fn set_tt_hub_slot_id(&mut self, slot_id: u8) {
        self.tt_info = (self.tt_info & !0xff0000) | (((slot_id & 0xff) as u32) << 16);
    }

    /// Set the USB device address
    pub fn set_device_address(&mut self, address: u8) {
        self.device_info = (self.device_info & !0xff) | ((address & 0xff) as u32);
    }

    /// Set the device state
    pub fn set_device_state(&mut self, state: u8) {
        self.device_info = (self.device_info & !0xf8000000) | (((state & 0x1f) as u32) << 27);
    }

    /// Get the device address
    pub fn device_address(&self) -> u8 {
        (self.device_info & 0xff) as u8
    }

    /// Get the device state
    pub fn device_state(&self) -> u8 {
        ((self.device_info >> 27) & 0x1f) as u8
    }
}

impl Default for SlotContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Endpoint Context (32 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct EndpointContext {
    /// Endpoint state info
    pub endpoint_info: u32,
    /// Endpoint characteristics
    pub endpoint_info2: u32,
    /// Transfer ring dequeue pointer (64-bit)
    pub tr_dequeue_pointer: u64,
    /// Transfer characteristics
    pub transfer_info: u32,
    /// Reserved
    reserved: [u32; 3],
}

impl EndpointContext {
    /// Create a new endpoint context
    pub fn new() -> Self {
        Self {
            endpoint_info: 0,
            endpoint_info2: 0,
            tr_dequeue_pointer: 0,
            transfer_info: 0,
            reserved: [0; 3],
        }
    }

    /// Set the endpoint state
    pub fn set_endpoint_state(&mut self, state: u8) {
        self.endpoint_info = (self.endpoint_info & !0x7) | ((state & 0x7) as u32);
    }

    /// Set the multiplier
    pub fn set_multiplier(&mut self, mult: u8) {
        self.endpoint_info = (self.endpoint_info & !0x18) | (((mult & 0x3) as u32) << 3);
    }

    /// Set the maximum primary streams
    pub fn set_max_primary_streams(&mut self, streams: u8) {
        self.endpoint_info = (self.endpoint_info & !0x3e0) | (((streams & 0x1f) as u32) << 5);
    }

    /// Set the linear stream array
    pub fn set_linear_stream_array(&mut self, lsa: bool) {
        if lsa {
            self.endpoint_info |= 0x8000;
        } else {
            self.endpoint_info &= !0x8000;
        }
    }

    /// Set the interval
    pub fn set_interval(&mut self, interval: u8) {
        self.endpoint_info = (self.endpoint_info & !0xff0000) | (((interval & 0xff) as u32) << 16);
    }

    /// Set the max endpoint service time interval payload high
    pub fn set_max_esit_payload_hi(&mut self, payload: u8) {
        self.endpoint_info = (self.endpoint_info & !0xff000000) | (((payload & 0xff) as u32) << 24);
    }

    /// Set the error count
    pub fn set_error_count(&mut self, count: u8) {
        self.endpoint_info2 = (self.endpoint_info2 & !0x6) | (((count & 0x3) as u32) << 1);
    }

    /// Set the endpoint type
    pub fn set_endpoint_type(&mut self, ep_type: UsbEndpointType) {
        let type_value = match ep_type {
            UsbEndpointType::Control => 4,
            UsbEndpointType::Isochronous => 1, // OUT
            UsbEndpointType::Bulk => 2,       // OUT
            UsbEndpointType::Interrupt => 3,  // OUT
        };
        self.endpoint_info2 = (self.endpoint_info2 & !0x38) | ((type_value as u32) << 3);
    }

    /// Set the endpoint type with direction
    pub fn set_endpoint_type_with_direction(&mut self, ep_type: UsbEndpointType, direction: UsbDirection) {
        let type_value = match (ep_type, direction) {
            (UsbEndpointType::Control, _) => 4,
            (UsbEndpointType::Isochronous, UsbDirection::Out) => 1,
            (UsbEndpointType::Isochronous, UsbDirection::In) => 5,
            (UsbEndpointType::Bulk, UsbDirection::Out) => 2,
            (UsbEndpointType::Bulk, UsbDirection::In) => 6,
            (UsbEndpointType::Interrupt, UsbDirection::Out) => 3,
            (UsbEndpointType::Interrupt, UsbDirection::In) => 7,
        };
        self.endpoint_info2 = (self.endpoint_info2 & !0x38) | ((type_value as u32) << 3);
    }

    /// Set the host initiate disable
    pub fn set_host_initiate_disable(&mut self, disable: bool) {
        if disable {
            self.endpoint_info2 |= 0x80;
        } else {
            self.endpoint_info2 &= !0x80;
        }
    }

    /// Set the maximum burst size
    pub fn set_max_burst_size(&mut self, burst: u8) {
        self.endpoint_info2 = (self.endpoint_info2 & !0xff00) | (((burst & 0xff) as u32) << 8);
    }

    /// Set the maximum packet size
    pub fn set_max_packet_size(&mut self, size: u16) {
        self.endpoint_info2 = (self.endpoint_info2 & !0xffff0000) | (((size & 0xffff) as u32) << 16);
    }

    /// Set the transfer ring dequeue pointer
    pub fn set_tr_dequeue_pointer(&mut self, pointer: u64, cycle_state: bool) {
        self.tr_dequeue_pointer = (pointer & !0xf) | if cycle_state { 1 } else { 0 };
    }

    /// Set the average transfer length
    pub fn set_average_trb_length(&mut self, length: u16) {
        self.transfer_info = (self.transfer_info & !0xffff) | ((length & 0xffff) as u32);
    }

    /// Set the max endpoint service time interval payload low
    pub fn set_max_esit_payload_lo(&mut self, payload: u16) {
        self.transfer_info = (self.transfer_info & !0xffff0000) | (((payload & 0xffff) as u32) << 16);
    }

    /// Get the endpoint state
    pub fn endpoint_state(&self) -> u8 {
        (self.endpoint_info & 0x7) as u8
    }

    /// Get the maximum packet size
    pub fn max_packet_size(&self) -> u16 {
        ((self.endpoint_info2 >> 16) & 0xffff) as u16
    }

    /// Configure for control endpoint
    pub fn configure_control(&mut self, max_packet_size: u16, ring_address: u64) {
        self.set_endpoint_state(0); // Disabled initially
        self.set_endpoint_type(UsbEndpointType::Control);
        self.set_max_packet_size(max_packet_size);
        self.set_error_count(3);
        self.set_tr_dequeue_pointer(ring_address, true);
        self.set_average_trb_length(8); // Typical for control transfers
    }

    /// Configure for bulk endpoint
    pub fn configure_bulk(&mut self, direction: UsbDirection, max_packet_size: u16, ring_address: u64) {
        self.set_endpoint_state(0); // Disabled initially
        self.set_endpoint_type_with_direction(UsbEndpointType::Bulk, direction);
        self.set_max_packet_size(max_packet_size);
        self.set_error_count(3);
        self.set_tr_dequeue_pointer(ring_address, true);
        self.set_average_trb_length(1024); // Typical for bulk transfers
    }

    /// Configure for interrupt endpoint
    pub fn configure_interrupt(&mut self, direction: UsbDirection, max_packet_size: u16, interval: u8, ring_address: u64) {
        self.set_endpoint_state(0); // Disabled initially
        self.set_endpoint_type_with_direction(UsbEndpointType::Interrupt, direction);
        self.set_max_packet_size(max_packet_size);
        self.set_interval(interval);
        self.set_error_count(3);
        self.set_tr_dequeue_pointer(ring_address, true);
        self.set_average_trb_length(max_packet_size);
    }
}

impl Default for EndpointContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Device Context (contains slot context and endpoint contexts)
#[derive(Debug, Clone)]
pub struct DeviceContext {
    /// Slot context
    pub slot_context: SlotContext,
    /// Endpoint contexts (up to 31)
    pub endpoint_contexts: [EndpointContext; 31],
}

impl DeviceContext {
    /// Create a new device context
    pub fn new() -> Self {
        Self {
            slot_context: SlotContext::new(),
            endpoint_contexts: [EndpointContext::new(); 31],
        }
    }

    /// Get the size of the device context in bytes
    pub fn size() -> usize {
        mem::size_of::<SlotContext>() + 31 * mem::size_of::<EndpointContext>()
    }

    /// Get the endpoint context for a specific endpoint
    pub fn endpoint_context(&self, endpoint_index: u8) -> Option<&EndpointContext> {
        if endpoint_index == 0 || endpoint_index > 31 {
            return None;
        }
        Some(&self.endpoint_contexts[(endpoint_index - 1) as usize])
    }

    /// Get mutable endpoint context for a specific endpoint
    pub fn endpoint_context_mut(&mut self, endpoint_index: u8) -> Option<&mut EndpointContext> {
        if endpoint_index == 0 || endpoint_index > 31 {
            return None;
        }
        Some(&mut self.endpoint_contexts[(endpoint_index - 1) as usize])
    }

    /// Configure the device context for a USB device
    pub fn configure_device(&mut self, device: &UsbDevice, control_ring_address: u64) {
        // Configure slot context
        self.slot_context.set_speed(device.speed);
        self.slot_context.set_route_string(0); // Direct connection to root hub
        self.slot_context.set_root_hub_port(device.port);
        self.slot_context.set_context_entries(1); // Only control endpoint initially
        self.slot_context.set_device_address(device.address);

        // Configure control endpoint (endpoint 1)
        if let Some(control_ep) = self.endpoint_context_mut(1) {
            let control_endpoint = device.control_endpoint();
            control_ep.configure_control(control_endpoint.max_packet_size, control_ring_address);
        }
    }

    /// Add an endpoint to the device context
    pub fn add_endpoint(&mut self, endpoint: &UsbEndpoint, ring_address: u64) -> Result<()> {
        let endpoint_index = endpoint.xhci_index();
        if endpoint_index == 0 || endpoint_index > 31 {
            return Err(UsbError::InvalidEndpoint);
        }

        if let Some(ep_context) = self.endpoint_context_mut(endpoint_index) {
            match endpoint.endpoint_type {
                UsbEndpointType::Control => {
                    ep_context.configure_control(endpoint.max_packet_size, ring_address);
                }
                UsbEndpointType::Bulk => {
                    ep_context.configure_bulk(endpoint.direction, endpoint.max_packet_size, ring_address);
                }
                UsbEndpointType::Interrupt => {
                    ep_context.configure_interrupt(endpoint.direction, endpoint.max_packet_size, endpoint.interval, ring_address);
                }
                UsbEndpointType::Isochronous => {
                    // Isochronous endpoints require more complex configuration
                    ep_context.set_endpoint_type_with_direction(UsbEndpointType::Isochronous, endpoint.direction);
                    ep_context.set_max_packet_size(endpoint.max_packet_size);
                    ep_context.set_interval(endpoint.interval);
                    ep_context.set_tr_dequeue_pointer(ring_address, true);
                }
            }

            // Update context entries in slot context
            let current_entries = ((self.slot_context.context_info >> 27) & 0x1f) as u8;
            if endpoint_index > current_entries {
                self.slot_context.set_context_entries(endpoint_index);
            }

            Ok(())
        } else {
            Err(UsbError::InvalidEndpoint)
        }
    }
}

impl Default for DeviceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Input Context for xHCI commands
#[derive(Debug)]
pub struct InputContext {
    /// Input control context
    pub input_control_context: InputControlContext,
    /// Device context
    pub device_context: DeviceContext,
}

impl InputContext {
    /// Create a new input context
    pub fn new() -> Self {
        Self {
            input_control_context: InputControlContext::new(),
            device_context: DeviceContext::new(),
        }
    }

    /// Get the size of the input context in bytes
    pub fn size() -> usize {
        mem::size_of::<InputControlContext>() + DeviceContext::size()
    }
}

impl Default for InputContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Input Control Context
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct InputControlContext {
    /// Drop context flags
    pub drop_context_flags: u32,
    /// Add context flags
    pub add_context_flags: u32,
    /// Reserved
    reserved: [u32; 5],
    /// Configuration value
    pub configuration_value: u8,
    /// Interface number
    pub interface_number: u8,
    /// Alternate setting
    pub alternate_setting: u8,
    /// Reserved
    reserved2: u8,
}

impl InputControlContext {
    /// Create a new input control context
    pub fn new() -> Self {
        Self {
            drop_context_flags: 0,
            add_context_flags: 0,
            reserved: [0; 5],
            configuration_value: 0,
            interface_number: 0,
            alternate_setting: 0,
            reserved2: 0,
        }
    }

    /// Set the add context flag for a specific context
    pub fn set_add_context(&mut self, context_index: u8) {
        if context_index <= 31 {
            self.add_context_flags |= 1 << context_index;
        }
    }

    /// Set the drop context flag for a specific context
    pub fn set_drop_context(&mut self, context_index: u8) {
        if context_index <= 31 {
            self.drop_context_flags |= 1 << context_index;
        }
    }

    /// Clear all flags
    pub fn clear_flags(&mut self) {
        self.drop_context_flags = 0;
        self.add_context_flags = 0;
    }
}

impl Default for InputControlContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Device Context Base Address Array (DCBAA)
pub struct DeviceContextBaseAddressArray {
    /// Array of device context pointers (up to 256 slots)
    entries: [u64; 256],
}

impl DeviceContextBaseAddressArray {
    /// Create a new DCBAA
    pub fn new() -> Self {
        Self {
            entries: [0; 256],
        }
    }

    /// Set the device context base address for a slot
    pub fn set_device_context_base_address(&mut self, slot_id: u8, address: u64) {
        if slot_id > 0 && slot_id <= 255 {
            self.entries[slot_id as usize] = address & !0x3f; // Must be 64-byte aligned
        }
    }

    /// Get the device context base address for a slot
    pub fn get_device_context_base_address(&self, slot_id: u8) -> u64 {
        if slot_id > 0 && slot_id <= 255 {
            self.entries[slot_id as usize]
        } else {
            0
        }
    }

    /// Get the physical address of the DCBAA
    pub fn physical_address(&self) -> u64 {
        self.entries.as_ptr() as u64
    }

    /// Clear a slot entry
    pub fn clear_slot(&mut self, slot_id: u8) {
        if slot_id > 0 && slot_id <= 255 {
            self.entries[slot_id as usize] = 0;
        }
    }
}

impl Default for DeviceContextBaseAddressArray {
    fn default() -> Self {
        Self::new()
    }
}