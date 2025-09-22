/// Transfer Request Block (TRB) definitions for xHCI
///
/// TRBs are the fundamental data structures used by xHCI to communicate
/// commands, transfers, and events between software and hardware.

use crate::types::*;

/// TRB Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrbType {
    // Transfer TRBs
    Normal = 1,
    SetupStage = 2,
    DataStage = 3,
    StatusStage = 4,
    Isoch = 5,
    Link = 6,
    EventData = 7,
    NoOp = 8,

    // Command TRBs
    EnableSlotCommand = 9,
    DisableSlotCommand = 10,
    AddressDeviceCommand = 11,
    ConfigureEndpointCommand = 12,
    EvaluateContextCommand = 13,
    ResetEndpointCommand = 14,
    StopEndpointCommand = 15,
    SetTrDequeuePointerCommand = 16,
    ResetDeviceCommand = 17,
    ForceEventCommand = 18,
    NegotiateBandwidthCommand = 19,
    SetLatencyToleranceCommand = 20,
    GetPortBandwidthCommand = 21,
    ForceHeaderCommand = 22,
    NoOpCommand = 23,

    // Event TRBs
    TransferEvent = 32,
    CommandCompletionEvent = 33,
    PortStatusChangeEvent = 34,
    BandwidthRequestEvent = 35,
    DoorbellEvent = 36,
    HostControllerEvent = 37,
    DeviceNotificationEvent = 38,
    MfindexWrapEvent = 39,

    // Unknown TRB type
    Unknown = 255,
}

impl From<u8> for TrbType {
    fn from(value: u8) -> Self {
        match value {
            1 => TrbType::Normal,
            2 => TrbType::SetupStage,
            3 => TrbType::DataStage,
            4 => TrbType::StatusStage,
            5 => TrbType::Isoch,
            6 => TrbType::Link,
            7 => TrbType::EventData,
            8 => TrbType::NoOp,
            9 => TrbType::EnableSlotCommand,
            10 => TrbType::DisableSlotCommand,
            11 => TrbType::AddressDeviceCommand,
            12 => TrbType::ConfigureEndpointCommand,
            13 => TrbType::EvaluateContextCommand,
            14 => TrbType::ResetEndpointCommand,
            15 => TrbType::StopEndpointCommand,
            16 => TrbType::SetTrDequeuePointerCommand,
            17 => TrbType::ResetDeviceCommand,
            18 => TrbType::ForceEventCommand,
            19 => TrbType::NegotiateBandwidthCommand,
            20 => TrbType::SetLatencyToleranceCommand,
            21 => TrbType::GetPortBandwidthCommand,
            22 => TrbType::ForceHeaderCommand,
            23 => TrbType::NoOpCommand,
            32 => TrbType::TransferEvent,
            33 => TrbType::CommandCompletionEvent,
            34 => TrbType::PortStatusChangeEvent,
            35 => TrbType::BandwidthRequestEvent,
            36 => TrbType::DoorbellEvent,
            37 => TrbType::HostControllerEvent,
            38 => TrbType::DeviceNotificationEvent,
            39 => TrbType::MfindexWrapEvent,
            _ => TrbType::Unknown,
        }
    }
}

/// Generic TRB structure (16 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Trb {
    /// Parameter or data (64-bit)
    pub parameter: u64,
    /// Status field (32-bit)
    pub status: u32,
    /// Control field (32-bit)
    pub control: u32,
}

impl Trb {
    /// Create a new TRB
    pub fn new() -> Self {
        Self {
            parameter: 0,
            status: 0,
            control: 0,
        }
    }

    /// Get the TRB type
    pub fn trb_type(&self) -> TrbType {
        TrbType::from(((self.control >> 10) & 0x3f) as u8)
    }

    /// Set the TRB type
    pub fn set_trb_type(&mut self, trb_type: TrbType) {
        self.control = (self.control & !0xfc00) | ((trb_type as u32) << 10);
    }

    /// Get the cycle bit
    pub fn cycle_bit(&self) -> bool {
        (self.control & 0x01) != 0
    }

    /// Set the cycle bit
    pub fn set_cycle_bit(&mut self, cycle: bool) {
        if cycle {
            self.control |= 0x01;
        } else {
            self.control &= !0x01;
        }
    }

    /// Get the toggle cycle bit
    pub fn toggle_cycle(&self) -> bool {
        (self.control & 0x02) != 0
    }

    /// Set the toggle cycle bit
    pub fn set_toggle_cycle(&mut self, toggle: bool) {
        if toggle {
            self.control |= 0x02;
        } else {
            self.control &= !0x02;
        }
    }

    /// Get the interrupt on completion bit
    pub fn interrupt_on_completion(&self) -> bool {
        (self.control & 0x20) != 0
    }

    /// Set the interrupt on completion bit
    pub fn set_interrupt_on_completion(&mut self, ioc: bool) {
        if ioc {
            self.control |= 0x20;
        } else {
            self.control &= !0x20;
        }
    }

    /// Get the immediate data bit
    pub fn immediate_data(&self) -> bool {
        (self.control & 0x40) != 0
    }

    /// Set the immediate data bit
    pub fn set_immediate_data(&mut self, idt: bool) {
        if idt {
            self.control |= 0x40;
        } else {
            self.control &= !0x40;
        }
    }
}

impl Default for Trb {
    fn default() -> Self {
        Self::new()
    }
}

/// Setup Stage TRB for control transfers
pub struct SetupStageTrb;

impl SetupStageTrb {
    /// Create a setup stage TRB
    pub fn new(setup_packet: &SetupPacket, transfer_length: u32, cycle: bool) -> Trb {
        let mut trb = Trb::new();

        // Setup packet goes in the parameter field
        trb.parameter = u64::from_le_bytes([
            setup_packet.request_type,
            setup_packet.request,
            (setup_packet.value & 0xff) as u8,
            (setup_packet.value >> 8) as u8,
            (setup_packet.index & 0xff) as u8,
            (setup_packet.index >> 8) as u8,
            (setup_packet.length & 0xff) as u8,
            (setup_packet.length >> 8) as u8,
        ]);

        trb.status = transfer_length & 0x1ffff; // Transfer length (17 bits)
        trb.status |= 0 << 22; // TD Size = 0
        trb.status |= 0 << 25; // Interrupter Target = 0

        trb.set_trb_type(TrbType::SetupStage);
        trb.set_cycle_bit(cycle);
        trb.set_interrupt_on_completion(false); // Usually don't interrupt on setup
        trb.set_immediate_data(true); // Setup data is immediate

        // Set transfer type
        let transfer_type = if setup_packet.length == 0 {
            0 // No data stage
        } else if (setup_packet.request_type & 0x80) != 0 {
            3 // IN data stage
        } else {
            2 // OUT data stage
        };
        trb.control |= (transfer_type & 0x03) << 16;

        trb
    }
}

/// Data Stage TRB for control transfers
pub struct DataStageTrb;

impl DataStageTrb {
    /// Create a data stage TRB
    pub fn new(data_buffer: u64, transfer_length: u32, direction: UsbDirection, cycle: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = data_buffer;
        trb.status = transfer_length & 0x1ffff;
        trb.status |= 0 << 22; // TD Size = 0 for now
        trb.status |= 0 << 25; // Interrupter Target = 0

        trb.set_trb_type(TrbType::DataStage);
        trb.set_cycle_bit(cycle);
        trb.set_interrupt_on_completion(false);

        // Set direction bit
        if matches!(direction, UsbDirection::In) {
            trb.control |= 0x10000; // DIR bit
        }

        trb
    }
}

/// Status Stage TRB for control transfers
pub struct StatusStageTrb;

impl StatusStageTrb {
    /// Create a status stage TRB
    pub fn new(direction: UsbDirection, cycle: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = 0;
        trb.status = 0;

        trb.set_trb_type(TrbType::StatusStage);
        trb.set_cycle_bit(cycle);
        trb.set_interrupt_on_completion(true); // Always interrupt on status completion

        // Status stage direction is opposite of data stage
        if matches!(direction, UsbDirection::In) {
            trb.control |= 0x10000; // DIR bit
        }

        trb
    }
}

/// Normal TRB for bulk/interrupt transfers
pub struct NormalTrb;

impl NormalTrb {
    /// Create a normal TRB
    pub fn new(data_buffer: u64, transfer_length: u32, cycle: bool, interrupt_on_completion: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = data_buffer;
        trb.status = transfer_length & 0x1ffff;
        trb.status |= 0 << 22; // TD Size = 0 for now
        trb.status |= 0 << 25; // Interrupter Target = 0

        trb.set_trb_type(TrbType::Normal);
        trb.set_cycle_bit(cycle);
        trb.set_interrupt_on_completion(interrupt_on_completion);

        trb
    }
}

/// Link TRB for ring segments
pub struct LinkTrb;

impl LinkTrb {
    /// Create a link TRB
    pub fn new(ring_segment: u64, cycle: bool, toggle_cycle: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = ring_segment & !0x0f; // Must be 16-byte aligned
        trb.status = 0;

        trb.set_trb_type(TrbType::Link);
        trb.set_cycle_bit(cycle);
        trb.set_toggle_cycle(toggle_cycle);

        trb
    }
}

/// Enable Slot Command TRB
pub struct EnableSlotCommandTrb;

impl EnableSlotCommandTrb {
    /// Create an enable slot command TRB
    pub fn new(slot_type: u8, cycle: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = 0;
        trb.status = 0;
        trb.control = (slot_type as u32) << 16;

        trb.set_trb_type(TrbType::EnableSlotCommand);
        trb.set_cycle_bit(cycle);

        trb
    }
}

/// Disable Slot Command TRB
pub struct DisableSlotCommandTrb;

impl DisableSlotCommandTrb {
    /// Create a disable slot command TRB
    pub fn new(slot_id: u8, cycle: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = 0;
        trb.status = 0;
        trb.control = (slot_id as u32) << 24;

        trb.set_trb_type(TrbType::DisableSlotCommand);
        trb.set_cycle_bit(cycle);

        trb
    }
}

/// Address Device Command TRB
pub struct AddressDeviceCommandTrb;

impl AddressDeviceCommandTrb {
    /// Create an address device command TRB
    pub fn new(input_context: u64, slot_id: u8, block_set_address_request: bool, cycle: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = input_context & !0x0f; // Must be 16-byte aligned
        trb.status = 0;
        trb.control = (slot_id as u32) << 24;

        if block_set_address_request {
            trb.control |= 0x200; // BSR bit
        }

        trb.set_trb_type(TrbType::AddressDeviceCommand);
        trb.set_cycle_bit(cycle);

        trb
    }
}

/// Configure Endpoint Command TRB
pub struct ConfigureEndpointCommandTrb;

impl ConfigureEndpointCommandTrb {
    /// Create a configure endpoint command TRB
    pub fn new(input_context: u64, slot_id: u8, deconfigure: bool, cycle: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = input_context & !0x0f; // Must be 16-byte aligned
        trb.status = 0;
        trb.control = (slot_id as u32) << 24;

        if deconfigure {
            trb.control |= 0x200; // DC bit
        }

        trb.set_trb_type(TrbType::ConfigureEndpointCommand);
        trb.set_cycle_bit(cycle);

        trb
    }
}

/// No-Op Command TRB
pub struct NoOpCommandTrb;

impl NoOpCommandTrb {
    /// Create a no-op command TRB
    pub fn new(cycle: bool) -> Trb {
        let mut trb = Trb::new();

        trb.parameter = 0;
        trb.status = 0;
        trb.control = 0;

        trb.set_trb_type(TrbType::NoOpCommand);
        trb.set_cycle_bit(cycle);

        trb
    }
}

/// Transfer Event TRB
#[derive(Debug, Clone, Copy)]
pub struct TransferEvent {
    trb: Trb,
}

impl TransferEvent {
    /// Create from raw TRB
    pub fn from_trb(trb: Trb) -> Self {
        Self { trb }
    }

    /// Get the TRB pointer
    pub fn trb_pointer(&self) -> u64 {
        self.trb.parameter
    }

    /// Get the transfer length
    pub fn transfer_length(&self) -> u32 {
        self.trb.status & 0xffffff
    }

    /// Get the completion code
    pub fn completion_code(&self) -> u8 {
        ((self.trb.status >> 24) & 0xff) as u8
    }

    /// Get the endpoint ID
    pub fn endpoint_id(&self) -> u8 {
        ((self.trb.control >> 16) & 0x1f) as u8
    }

    /// Get the slot ID
    pub fn slot_id(&self) -> u8 {
        ((self.trb.control >> 24) & 0xff) as u8
    }

    /// Check if this is a successful completion
    pub fn is_success(&self) -> bool {
        self.completion_code() == 1 // Success completion code
    }

    /// Check if this is a short packet
    pub fn is_short_packet(&self) -> bool {
        self.completion_code() == 13 // Short packet completion code
    }
}

/// Command Completion Event TRB
#[derive(Debug, Clone, Copy)]
pub struct CommandCompletionEvent {
    trb: Trb,
}

impl CommandCompletionEvent {
    /// Create from raw TRB
    pub fn from_trb(trb: Trb) -> Self {
        Self { trb }
    }

    /// Get the command TRB pointer
    pub fn command_trb_pointer(&self) -> u64 {
        self.trb.parameter
    }

    /// Get the command completion parameter
    pub fn parameter(&self) -> u32 {
        self.trb.status & 0xffffff
    }

    /// Get the completion code
    pub fn completion_code(&self) -> u8 {
        ((self.trb.status >> 24) & 0xff) as u8
    }

    /// Get the VF ID
    pub fn vf_id(&self) -> u8 {
        ((self.trb.control >> 16) & 0xff) as u8
    }

    /// Get the slot ID
    pub fn slot_id(&self) -> u8 {
        ((self.trb.control >> 24) & 0xff) as u8
    }

    /// Check if this is a successful completion
    pub fn is_success(&self) -> bool {
        self.completion_code() == 1 // Success completion code
    }
}

/// Port Status Change Event TRB
#[derive(Debug, Clone, Copy)]
pub struct PortStatusChangeEvent {
    trb: Trb,
}

impl PortStatusChangeEvent {
    /// Create from raw TRB
    pub fn from_trb(trb: Trb) -> Self {
        Self { trb }
    }

    /// Get the port ID
    pub fn port_id(&self) -> u8 {
        ((self.trb.parameter >> 24) & 0xff) as u8
    }

    /// Get the completion code
    pub fn completion_code(&self) -> u8 {
        ((self.trb.status >> 24) & 0xff) as u8
    }
}

/// TRB Completion Codes
#[allow(dead_code)]
pub mod completion_codes {
    pub const INVALID: u8 = 0;
    pub const SUCCESS: u8 = 1;
    pub const DATA_BUFFER_ERROR: u8 = 2;
    pub const BABBLE_DETECTED_ERROR: u8 = 3;
    pub const USB_TRANSACTION_ERROR: u8 = 4;
    pub const TRB_ERROR: u8 = 5;
    pub const STALL_ERROR: u8 = 6;
    pub const RESOURCE_ERROR: u8 = 7;
    pub const BANDWIDTH_ERROR: u8 = 8;
    pub const NO_SLOTS_AVAILABLE_ERROR: u8 = 9;
    pub const INVALID_STREAM_TYPE_ERROR: u8 = 10;
    pub const SLOT_NOT_ENABLED_ERROR: u8 = 11;
    pub const ENDPOINT_NOT_ENABLED_ERROR: u8 = 12;
    pub const SHORT_PACKET: u8 = 13;
    pub const RING_UNDERRUN: u8 = 14;
    pub const RING_OVERRUN: u8 = 15;
    pub const VF_EVENT_RING_FULL_ERROR: u8 = 16;
    pub const PARAMETER_ERROR: u8 = 17;
    pub const BANDWIDTH_OVERRUN_ERROR: u8 = 18;
    pub const CONTEXT_STATE_ERROR: u8 = 19;
    pub const NO_PING_RESPONSE_ERROR: u8 = 20;
    pub const EVENT_RING_FULL_ERROR: u8 = 21;
    pub const INCOMPATIBLE_DEVICE_ERROR: u8 = 22;
    pub const MISSED_SERVICE_ERROR: u8 = 23;
    pub const COMMAND_RING_STOPPED: u8 = 24;
    pub const COMMAND_ABORTED: u8 = 25;
    pub const STOPPED: u8 = 26;
    pub const STOPPED_LENGTH_INVALID: u8 = 27;
    pub const STOPPED_SHORT_PACKET: u8 = 28;
    pub const MAX_EXIT_LATENCY_TOO_LARGE_ERROR: u8 = 29;
    pub const ISOCH_BUFFER_OVERRUN: u8 = 31;
    pub const EVENT_LOST_ERROR: u8 = 32;
    pub const UNDEFINED_ERROR: u8 = 33;
    pub const INVALID_STREAM_ID_ERROR: u8 = 34;
    pub const SECONDARY_BANDWIDTH_ERROR: u8 = 35;
    pub const SPLIT_TRANSACTION_ERROR: u8 = 36;
}