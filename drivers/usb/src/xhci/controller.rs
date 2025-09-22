/// xHCI Host Controller Implementation
///
/// This module implements the main xHCI host controller driver that
/// integrates with the lib_kernel driver framework.

use super::{
    registers::*,
    context::*,
    ring::*,
    trb::*,
    transfer::*,
    XhciCapabilities,
    XhciPortStatus,
};
use crate::error::{UsbError, Result};
use crate::types::*;
use lib_kernel::api::commands::{inl, outl, inb, outb};
use lib_kernel::drivers::{Driver, DriverError};
use alloc::{vec::Vec, collections::BTreeMap, boxed::Box};
use core::any::Any;
use spin::{Mutex, RwLock};
use volatile::Volatile;

/// xHCI Host Controller Driver
pub struct XhciController {
    /// Controller name
    name: &'static str,
    /// Base address of xHCI registers
    base_address: u64,
    /// Capability registers
    cap_regs: Option<&'static CapabilityRegisters>,
    /// Operational registers
    op_regs: Option<&'static mut OperationalRegisters>,
    /// Runtime registers
    runtime_regs: Option<&'static mut RuntimeRegisters>,
    /// Port register sets
    port_regs: Vec<&'static mut PortRegisterSet>,
    /// Doorbell registers
    doorbell_regs: Vec<&'static mut DoorbellRegister>,
    /// Capabilities
    capabilities: Option<XhciCapabilities>,
    /// Command ring
    command_ring: Option<CommandRing>,
    /// Event ring
    event_ring: Option<EventRing>,
    /// Device context base address array
    dcbaa: Option<DeviceContextBaseAddressArray>,
    /// Connected devices
    devices: BTreeMap<u8, UsbDevice>,
    /// Slot allocations
    allocated_slots: [bool; 256],
    /// Transfer rings for endpoints
    transfer_rings: BTreeMap<(u8, u8), TransferRing>, // (slot_id, endpoint_index)
    /// Device contexts
    device_contexts: BTreeMap<u8, Box<DeviceContext>>,
    /// Transfer manager
    transfer_manager: UsbTransferManager,
    /// Controller state
    state: ControllerState,
    /// IRQ line
    irq_line: Option<u8>,
}

/// Controller state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControllerState {
    Uninitialized,
    Initializing,
    Running,
    Halted,
    Error,
}

impl XhciController {
    /// Create a new xHCI controller
    pub fn new(name: &'static str, base_address: u64, irq_line: Option<u8>) -> Self {
        Self {
            name,
            base_address,
            cap_regs: None,
            op_regs: None,
            runtime_regs: None,
            port_regs: Vec::new(),
            doorbell_regs: Vec::new(),
            capabilities: None,
            command_ring: None,
            event_ring: None,
            dcbaa: None,
            devices: BTreeMap::new(),
            allocated_slots: [false; 256],
            transfer_rings: BTreeMap::new(),
            device_contexts: BTreeMap::new(),
            transfer_manager: UsbTransferManager::new(),
            state: ControllerState::Uninitialized,
            irq_line,
        }
    }

    /// Initialize the xHCI controller
    fn initialize_controller(&mut self) -> Result<()> {
        self.state = ControllerState::Initializing;

        // Map registers
        self.map_registers()?;

        // Read capabilities
        self.read_capabilities()?;

        // Reset controller
        self.reset_controller()?;

        // Initialize rings
        self.initialize_rings()?;

        // Initialize DCBAA
        self.initialize_dcbaa()?;

        // Configure controller
        self.configure_controller()?;

        // Start controller
        self.start_controller()?;

        // Initialize ports
        self.initialize_ports()?;

        self.state = ControllerState::Running;
        Ok(())
    }

    /// Map xHCI registers
    fn map_registers(&mut self) -> Result<()> {
        // In a real implementation, we would use proper memory mapping
        // For now, we'll use direct memory access (unsafe!)

        unsafe {
            // Map capability registers
            self.cap_regs = Some(&*(self.base_address as *const CapabilityRegisters));

            let cap_regs = self.cap_regs.unwrap();
            let cap_length = cap_regs.cap_length() as u64;

            // Map operational registers
            let op_base = self.base_address + cap_length;
            self.op_regs = Some(&mut *(op_base as *mut OperationalRegisters));

            // Map runtime registers
            let runtime_offset = cap_regs.runtime_offset() as u64;
            let runtime_base = self.base_address + runtime_offset;
            self.runtime_regs = Some(&mut *(runtime_base as *mut RuntimeRegisters));

            // Map port registers
            let max_ports = cap_regs.max_ports();
            let port_base = op_base + 0x400; // Port register sets start at offset 0x400
            for i in 0..max_ports {
                let port_addr = port_base + (i as u64 * 0x10);
                self.port_regs.push(&mut *(port_addr as *mut PortRegisterSet));
            }

            // Map doorbell registers
            let doorbell_offset = cap_regs.doorbell_offset() as u64;
            let doorbell_base = self.base_address + doorbell_offset;
            for i in 0..=max_ports {
                let doorbell_addr = doorbell_base + (i as u64 * 4);
                self.doorbell_regs.push(&mut *(doorbell_addr as *mut DoorbellRegister));
            }
        }

        Ok(())
    }

    /// Read controller capabilities
    fn read_capabilities(&mut self) -> Result<()> {
        let cap_regs = self.cap_regs.ok_or(UsbError::InitializationFailed)?;

        self.capabilities = Some(XhciCapabilities {
            cap_length: cap_regs.cap_length(),
            hci_version: cap_regs.hci_version(),
            hcsparams1: cap_regs.hcsparams1.read(),
            hcsparams2: cap_regs.hcsparams2.read(),
            hcsparams3: cap_regs.hcsparams3.read(),
            hccparams1: cap_regs.hccparams1.read(),
            doorbell_offset: cap_regs.doorbell_offset(),
            runtime_offset: cap_regs.runtime_offset(),
            hccparams2: cap_regs.hccparams2.read(),
        });

        Ok(())
    }

    /// Reset the controller
    fn reset_controller(&mut self) -> Result<()> {
        let op_regs = self.op_regs.as_mut().ok_or(UsbError::InitializationFailed)?;

        // Stop the controller first
        op_regs.stop();

        // Wait for halt
        let mut timeout = 1000;
        while !op_regs.is_halted() && timeout > 0 {
            // In a real implementation, we would use proper timing
            for _ in 0..1000 { core::hint::spin_loop(); }
            timeout -= 1;
        }

        if !op_regs.is_halted() {
            return Err(UsbError::InitializationFailed);
        }

        // Reset the controller
        op_regs.reset();

        // Wait for reset to complete
        timeout = 1000;
        while op_regs.is_controller_not_ready() && timeout > 0 {
            for _ in 0..1000 { core::hint::spin_loop(); }
            timeout -= 1;
        }

        if op_regs.is_controller_not_ready() {
            return Err(UsbError::InitializationFailed);
        }

        Ok(())
    }

    /// Initialize command and event rings
    fn initialize_rings(&mut self) -> Result<()> {
        // Create command ring
        self.command_ring = Some(CommandRing::new()?);

        // Create event ring segments
        let event_segments = vec![RingSegment::new(256)?]; // Single segment with 256 TRBs
        self.event_ring = Some(EventRing::new(event_segments)?);

        Ok(())
    }

    /// Initialize Device Context Base Address Array
    fn initialize_dcbaa(&mut self) -> Result<()> {
        self.dcbaa = Some(DeviceContextBaseAddressArray::new());
        Ok(())
    }

    /// Configure the controller
    fn configure_controller(&mut self) -> Result<()> {
        let op_regs = self.op_regs.as_mut().ok_or(UsbError::InitializationFailed)?;
        let runtime_regs = self.runtime_regs.as_mut().ok_or(UsbError::InitializationFailed)?;
        let capabilities = self.capabilities.as_ref().ok_or(UsbError::InitializationFailed)?;

        // Set page size (should be 4KB)
        let page_size = op_regs.get_page_size();
        if page_size != 4096 {
            return Err(UsbError::InitializationFailed);
        }

        // Set maximum device slots
        let max_slots = capabilities.max_device_slots();
        op_regs.set_max_device_slots(max_slots);

        // Set DCBAA pointer
        if let Some(dcbaa) = &self.dcbaa {
            op_regs.set_dcbaap(dcbaa.physical_address());
        }

        // Set command ring
        if let Some(cmd_ring) = &self.command_ring {
            op_regs.set_command_ring(cmd_ring.physical_address(), cmd_ring.cycle_state());
        }

        // Configure primary interrupter
        if let Some(event_ring) = &self.event_ring {
            let interrupter = &mut runtime_regs.interrupters[0];
            interrupter.set_event_ring_segment_table(
                event_ring.segment_table_address(),
                event_ring.segment_count(),
            );
            interrupter.set_event_ring_dequeue_pointer(event_ring.dequeue_pointer());
            interrupter.enable_interrupts();
        }

        // Enable controller interrupts
        op_regs.enable_interrupts();

        Ok(())
    }

    /// Start the controller
    fn start_controller(&mut self) -> Result<()> {
        let op_regs = self.op_regs.as_mut().ok_or(UsbError::InitializationFailed)?;

        op_regs.start();

        // Wait for controller to start
        let mut timeout = 1000;
        while op_regs.is_halted() && timeout > 0 {
            for _ in 0..1000 { core::hint::spin_loop(); }
            timeout -= 1;
        }

        if op_regs.is_halted() {
            return Err(UsbError::InitializationFailed);
        }

        Ok(())
    }

    /// Initialize ports
    fn initialize_ports(&mut self) -> Result<()> {
        let capabilities = self.capabilities.as_ref().ok_or(UsbError::InitializationFailed)?;
        let max_ports = capabilities.max_ports();

        for port_num in 0..max_ports {
            if let Some(port_regs) = self.port_regs.get_mut(port_num as usize) {
                // Power on the port if power control is supported
                if capabilities.supports_64bit() { // Using this as a placeholder check
                    port_regs.set_power(true);
                }

                // Clear any change events
                port_regs.clear_changes();
            }
        }

        Ok(())
    }

    /// Handle port status change
    fn handle_port_status_change(&mut self, port_id: u8) -> Result<()> {
        if let Some(port_regs) = self.port_regs.get_mut((port_id - 1) as usize) {
            let changes = port_regs.get_changes();

            if (changes & PortRegisterSet::portsc::CSC) != 0 {
                // Connection status changed
                if port_regs.is_connected() {
                    // Device connected
                    self.handle_device_connected(port_id)?;
                } else {
                    // Device disconnected
                    self.handle_device_disconnected(port_id)?;
                }
            }

            if (changes & PortRegisterSet::portsc::PRC) != 0 {
                // Port reset change
                self.handle_port_reset_complete(port_id)?;
            }

            // Clear the change events
            port_regs.clear_changes();
        }

        Ok(())
    }

    /// Handle device connection
    fn handle_device_connected(&mut self, port_id: u8) -> Result<()> {
        if let Some(port_regs) = self.port_regs.get_mut((port_id - 1) as usize) {
            // Reset the port to determine device speed
            port_regs.reset();

            // Wait for reset to complete (this would normally be handled by interrupt)
            // For now, we'll simulate it

            Ok(())
        } else {
            Err(UsbError::InvalidRequest)
        }
    }

    /// Handle port reset completion
    fn handle_port_reset_complete(&mut self, port_id: u8) -> Result<()> {
        if let Some(port_regs) = self.port_regs.get((port_id - 1) as usize) {
            if let Some(speed) = port_regs.get_speed() {
                // Enable a device slot
                let slot_id = self.enable_device_slot()?;

                // Create device
                let mut device = UsbDevice::new(0, port_id, speed); // Address 0 initially

                // Store device temporarily
                self.devices.insert(slot_id, device);

                // Start device enumeration
                self.enumerate_device(slot_id)?;
            }
        }

        Ok(())
    }

    /// Handle device disconnection
    fn handle_device_disconnected(&mut self, port_id: u8) -> Result<()> {
        // Find device by port
        let mut slot_to_remove = None;
        for (&slot_id, device) in &self.devices {
            if device.port == port_id {
                slot_to_remove = Some(slot_id);
                break;
            }
        }

        if let Some(slot_id) = slot_to_remove {
            self.disable_device_slot(slot_id)?;
        }

        Ok(())
    }

    /// Enable a device slot
    fn enable_device_slot(&mut self) -> Result<u8> {
        // Find free slot
        let mut slot_id = 0;
        for (i, &allocated) in self.allocated_slots.iter().enumerate().skip(1) {
            if !allocated {
                slot_id = i as u8;
                break;
            }
        }

        if slot_id == 0 {
            return Err(UsbError::OutOfMemory);
        }

        // Send enable slot command
        if let Some(cmd_ring) = &mut self.command_ring {
            let enable_slot_trb = EnableSlotCommandTrb::new(0, cmd_ring.cycle_state());
            cmd_ring.submit_command(enable_slot_trb)?;

            // Ring command doorbell
            if let Some(doorbell) = self.doorbell_regs.get_mut(0) {
                doorbell.ring_command();
            }

            // Wait for completion (simplified)
            let _completion = cmd_ring.wait_for_completion(1000)?;

            self.allocated_slots[slot_id as usize] = true;
        }

        Ok(slot_id)
    }

    /// Disable a device slot
    fn disable_device_slot(&mut self, slot_id: u8) -> Result<()> {
        if slot_id == 0 || slot_id > 255 || !self.allocated_slots[slot_id as usize] {
            return Err(UsbError::InvalidRequest);
        }

        // Send disable slot command
        if let Some(cmd_ring) = &mut self.command_ring {
            let disable_slot_trb = DisableSlotCommandTrb::new(slot_id, cmd_ring.cycle_state());
            cmd_ring.submit_command(disable_slot_trb)?;

            // Ring command doorbell
            if let Some(doorbell) = self.doorbell_regs.get_mut(0) {
                doorbell.ring_command();
            }

            // Wait for completion
            let _completion = cmd_ring.wait_for_completion(1000)?;
        }

        // Clean up
        self.allocated_slots[slot_id as usize] = false;
        self.devices.remove(&slot_id);
        self.device_contexts.remove(&slot_id);

        // Remove transfer rings for this slot
        self.transfer_rings.retain(|(sid, _), _| *sid != slot_id);

        Ok(())
    }

    /// Enumerate a device
    fn enumerate_device(&mut self, slot_id: u8) -> Result<()> {
        // Create device context
        let mut device_context = Box::new(DeviceContext::new());

        // Create control endpoint transfer ring
        let control_ring = TransferRing::new(1, 64)?;
        let control_ring_addr = control_ring.enqueue_pointer();

        if let Some(device) = self.devices.get(&slot_id) {
            device_context.configure_device(device, control_ring_addr);
        }

        // Store transfer ring
        self.transfer_rings.insert((slot_id, 1), control_ring);

        // Set up device context in DCBAA
        if let Some(dcbaa) = &mut self.dcbaa {
            dcbaa.set_device_context_base_address(slot_id, device_context.as_ref() as *const _ as u64);
        }

        // Store device context
        self.device_contexts.insert(slot_id, device_context);

        // Address the device
        self.address_device(slot_id)?;

        // Get device descriptor
        self.get_device_descriptor(slot_id)?;

        Ok(())
    }

    /// Address a device
    fn address_device(&mut self, slot_id: u8) -> Result<()> {
        // Create input context
        let mut input_context = InputContext::new();
        input_context.input_control_context.set_add_context(0); // Slot context
        input_context.input_control_context.set_add_context(1); // Endpoint 1 (control)

        // Copy device context to input context
        if let Some(device_context) = self.device_contexts.get(&slot_id) {
            input_context.device_context = **device_context;
        }

        // Send address device command
        if let Some(cmd_ring) = &mut self.command_ring {
            let address_device_trb = AddressDeviceCommandTrb::new(
                &input_context as *const _ as u64,
                slot_id,
                false, // Don't block SET_ADDRESS
                cmd_ring.cycle_state(),
            );
            cmd_ring.submit_command(address_device_trb)?;

            // Ring command doorbell
            if let Some(doorbell) = self.doorbell_regs.get_mut(0) {
                doorbell.ring_command();
            }

            // Wait for completion
            let _completion = cmd_ring.wait_for_completion(1000)?;
        }

        Ok(())
    }

    /// Get device descriptor
    fn get_device_descriptor(&mut self, slot_id: u8) -> Result<()> {
        // Create GET_DESCRIPTOR setup packet
        let setup_packet = SetupPacket::get_descriptor(
            descriptor_types::DEVICE,
            0,
            core::mem::size_of::<DeviceDescriptor>() as u16,
        );

        // Submit control transfer
        self.control_transfer(slot_id, setup_packet, None)?;

        Ok(())
    }

    /// Submit a control transfer
    fn control_transfer(&mut self, slot_id: u8, setup_packet: SetupPacket, data_buffer: Option<&[u8]>) -> Result<()> {
        if let Some(transfer_ring) = self.transfer_rings.get_mut(&(slot_id, 1)) {
            let cycle = transfer_ring.cycle_state();

            // Setup stage
            let setup_trb = SetupStageTrb::new(&setup_packet, setup_packet.length as u32, cycle);
            transfer_ring.enqueue_trb(setup_trb)?;

            // Data stage (if any)
            if setup_packet.length > 0 {
                let direction = if (setup_packet.request_type & 0x80) != 0 {
                    UsbDirection::In
                } else {
                    UsbDirection::Out
                };

                let data_buffer_addr = data_buffer.map(|buf| buf.as_ptr() as u64).unwrap_or(0);
                let data_trb = DataStageTrb::new(
                    data_buffer_addr,
                    setup_packet.length as u32,
                    direction,
                    transfer_ring.cycle_state(),
                );
                transfer_ring.enqueue_trb(data_trb)?;
            }

            // Status stage
            let status_direction = if setup_packet.length == 0 || (setup_packet.request_type & 0x80) == 0 {
                UsbDirection::In
            } else {
                UsbDirection::Out
            };
            let status_trb = StatusStageTrb::new(status_direction, transfer_ring.cycle_state());
            transfer_ring.enqueue_trb(status_trb)?;

            // Ring doorbell
            if let Some(doorbell) = self.doorbell_regs.get_mut(slot_id as usize) {
                doorbell.ring(1, 0); // Endpoint 1, stream 0
            }
        }

        Ok(())
    }

    /// Get controller capabilities
    pub fn get_capabilities(&self) -> Option<&XhciCapabilities> {
        self.capabilities.as_ref()
    }

    /// Get port status
    pub fn get_port_status(&self, port_id: u8) -> Option<XhciPortStatus> {
        if let Some(port_regs) = self.port_regs.get((port_id - 1) as usize) {
            Some(XhciPortStatus::new(port_regs.portsc.read()))
        } else {
            None
        }
    }

    /// Get connected devices
    pub fn get_devices(&self) -> Vec<&UsbDevice> {
        self.devices.values().collect()
    }

    /// Shutdown the controller
    fn shutdown_controller(&mut self) -> Result<()> {
        if let Some(op_regs) = &mut self.op_regs {
            op_regs.stop();

            // Wait for halt
            let mut timeout = 1000;
            while !op_regs.is_halted() && timeout > 0 {
                for _ in 0..1000 { core::hint::spin_loop(); }
                timeout -= 1;
            }
        }

        self.state = ControllerState::Halted;
        Ok(())
    }
}

impl Driver for XhciController {
    fn name(&self) -> &'static str {
        self.name
    }

    fn init(&mut self) -> Result<(), DriverError> {
        self.initialize_controller()
            .map_err(|_| DriverError::InitializationFailed)
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
        self.shutdown_controller()
            .map_err(|_| DriverError::HardwareError)
    }

    fn interrupt_handler(&mut self, irq: u8) -> bool {
        if Some(irq) != self.irq_line {
            return false;
        }

        // Check if this is our interrupt
        if let Some(op_regs) = &self.op_regs {
            let status = op_regs.usbsts.read();
            if (status & OperationalRegisters::usbsts::EINT) == 0 {
                return false; // Not our interrupt
            }

            // Clear interrupt
            op_regs.clear_status(OperationalRegisters::usbsts::EINT);
        }

        // Process events
        if let Some(event_ring) = &mut self.event_ring {
            let mut events_processed = 0;

            while let Some(event_trb) = event_ring.dequeue_event() {
                match event_trb.trb_type() {
                    TrbType::PortStatusChangeEvent => {
                        let port_event = PortStatusChangeEvent::from_trb(event_trb);
                        let _ = self.handle_port_status_change(port_event.port_id());
                    }
                    TrbType::TransferEvent => {
                        // Handle transfer completion
                        let transfer_event = TransferEvent::from_trb(event_trb);
                        // Process transfer completion...
                    }
                    TrbType::CommandCompletionEvent => {
                        // Handle command completion
                        if let Some(cmd_ring) = &mut self.command_ring {
                            cmd_ring.process_completion(event_trb);
                        }
                    }
                    _ => {
                        // Handle other event types
                    }
                }

                events_processed += 1;
            }

            // Update event ring dequeue pointer
            if events_processed > 0 {
                event_ring.update_dequeue_pointer(events_processed);

                // Update ERDP register
                if let Some(runtime_regs) = &mut self.runtime_regs {
                    runtime_regs.interrupters[0].update_erdp(event_ring.dequeue_pointer(), true);
                }
            }
        }

        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}