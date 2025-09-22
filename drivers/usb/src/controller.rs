//! USB Controller (xHCI) Management

use alloc::{vec, vec::Vec, boxed::Box, collections::VecDeque};
use core::{
    fmt,
    sync::atomic::{AtomicU8, AtomicU32, Ordering},
    num::NonZeroUsize,
};
use spin::Mutex;
use xhci::{Registers, accessor::Mapper};
use crate::{
    Result, UsbDriverError,
    transfer::{Transfer, TransferStatus, TransferType},
    device::{UsbDevice, DeviceSpeed},
};

/// Memory mapper wrapper for xHCI
#[derive(Clone)]
pub struct UsbMemoryMapper {
    base_phys: usize,
    base_virt: usize,
}

impl UsbMemoryMapper {
    pub fn new(base_phys: usize, base_virt: usize) -> Self {
        Self { base_phys, base_virt }
    }
}

impl Mapper for UsbMemoryMapper {
    unsafe fn map(&mut self, phys_base: usize, _bytes: usize) -> NonZeroUsize {
        // In a real OS, this would perform actual memory mapping
        // For now, assume identity mapping or pre-mapped memory
        let virt_addr = self.base_virt + (phys_base - self.base_phys);
        NonZeroUsize::new(virt_addr).unwrap_or(NonZeroUsize::new(phys_base).unwrap())
    }

    fn unmap(&mut self, _virt_base: usize, _bytes: usize) {
        // In a real OS, this would unmap the memory
        // For MMIO registers, typically no action needed
    }
}

/// Controller State
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerState {
    Uninitialized = 0,
    Initializing = 1,
    Running = 2,
    Suspended = 3,
    Error = 4,
    Resetting = 5,
}

impl From<u8> for ControllerState {
    fn from(value: u8) -> Self {
        match value {
            0 => ControllerState::Uninitialized,
            1 => ControllerState::Initializing,
            2 => ControllerState::Running,
            3 => ControllerState::Suspended,
            4 => ControllerState::Error,
            5 => ControllerState::Resetting,
            _ => ControllerState::Error,
        }
    }
}

/// USB Event types from controller
#[derive(Debug, Clone)]
pub enum UsbEvent {
    /// Device connected to port
    DeviceConnected { port: u8 },
    /// Device disconnected
    DeviceDisconnected { address: u8 },
    /// Transfer completed
    TransferComplete { transfer_id: u32, status: TransferStatus },
    /// Error occurred
    Error { error: UsbDriverError },
}

/// xHCI Slot Context Entry
struct SlotContext {
    device_address: u8,
    device: Option<UsbDevice>,
    endpoints: [Option<EndpointContext>; 32],
}

/// xHCI Endpoint Context Entry
struct EndpointContext {
    endpoint_address: u8,
    ring: Option<Box<[u8]>>, // Simplified ring buffer
    transfer_type: TransferType,
}

/// USB Controller Implementation
pub struct UsbController {
    /// xHCI registers
    registers: Registers<UsbMemoryMapper>,
    /// Controller state
    state: AtomicU8,
    /// Port count
    port_count: u8,
    /// Device slots
    slots: Mutex<Vec<Option<SlotContext>>>,
    /// Pending transfers
    pending_transfers: Mutex<VecDeque<Transfer>>,
    /// Completed transfers
    completed_transfers: Mutex<VecDeque<(u32, TransferStatus, usize)>>,
    /// Event queue
    event_queue: Mutex<VecDeque<UsbEvent>>,
    /// Next device address
    next_device_address: AtomicU8,
    /// Transfer ID counter
    next_transfer_id: AtomicU32,
}

impl UsbController {
    /// Create a new USB controller
    pub fn new<M: Mapper + Clone + Send + Sync + 'static>(
        mmio_base: usize,
        mapper: M,
    ) -> Result<Self> {
        // Create USB memory mapper
        let usb_mapper = UsbMemoryMapper::new(mmio_base, mmio_base);

        // Initialize xHCI registers
        let registers = unsafe {
            Registers::new(mmio_base, usb_mapper)
        };

        // Read port count from capability registers
        let hcs_params1 = registers.capability.hcsparams1.read_volatile();
        let port_count = hcs_params1.number_of_ports();

        Ok(Self {
            registers,
            state: AtomicU8::new(ControllerState::Uninitialized as u8),
            port_count,
            slots: Mutex::new((0..256).map(|_| None).collect()),
            pending_transfers: Mutex::new(VecDeque::new()),
            completed_transfers: Mutex::new(VecDeque::new()),
            event_queue: Mutex::new(VecDeque::new()),
            next_device_address: AtomicU8::new(1),
            next_transfer_id: AtomicU32::new(1),
        })
    }

    /// Initialize the controller
    pub async fn initialize(&mut self) -> Result<()> {
        self.set_state(ControllerState::Initializing);

        // Reset controller
        self.reset().await?;

        // Wait for controller to be ready
        while self.registers.operational.usbsts.read_volatile().controller_not_ready() {
            // Wait for controller to become ready
        }

        // Set up device context base address array
        let dcbaap = self.allocate_device_context_base_array().await?;
        self.registers.operational.dcbaap.update_volatile(|dcbaap_reg| {
            dcbaap_reg.set(dcbaap);
        });

        // Set up command ring
        let (command_ring_ptr, _command_ring_cycle) = self.allocate_command_ring().await?;
        self.registers.operational.crcr.update_volatile(|crcr| {
            crcr.set_command_ring_pointer(command_ring_ptr);
            crcr.set_ring_cycle_state();
        });

        // Set up event ring segment table
        let _event_ring_segment_table = self.allocate_event_ring().await?;

        // Set up interrupter 0 event ring segment table
        // Note: The exact method to access interrupter registers depends on xHCI crate implementation
        // In a real implementation, this would:
        // 1. Set the Event Ring Segment Table Size (ERSTSZ) register to 1
        // 2. Set the Event Ring Segment Table Base Address (ERSTBA) register
        // 3. Initialize the Event Ring Dequeue Pointer (ERDP) register

        // Set maximum device slots enabled
        let max_slots = self.registers.capability.hcsparams1.read_volatile().number_of_device_slots();
        self.registers.operational.config.update_volatile(|config| {
            config.set_max_device_slots_enabled(max_slots);
        });

        // Start controller
        self.registers.operational.usbcmd.update_volatile(|cmd| {
            cmd.set_run_stop();
        });

        // Wait for controller to start
        while self.registers.operational.usbsts.read_volatile().hc_halted() {
            // Wait for controller to start running
        }

        self.set_state(ControllerState::Running);
        Ok(())
    }

    /// Reset the controller
    pub async fn reset(&mut self) -> Result<()> {
        self.set_state(ControllerState::Resetting);

        // Stop controller
        self.registers.operational.usbcmd.update_volatile(|cmd| {
            cmd.clear_run_stop();
        });

        // Wait for halt
        while !self.registers.operational.usbsts.read_volatile().hc_halted() {
            // Wait for controller to halt
        }

        // Reset controller
        self.registers.operational.usbcmd.update_volatile(|cmd| {
            cmd.set_host_controller_reset();
        });

        // Wait for reset to complete
        while self.registers.operational.usbcmd.read_volatile().host_controller_reset() {
            // Wait for reset to complete
        }

        Ok(())
    }

    /// Get controller state
    pub fn state(&self) -> ControllerState {
        ControllerState::from(self.state.load(Ordering::Acquire))
    }

    /// Set controller state
    fn set_state(&self, state: ControllerState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Get port count
    pub fn port_count(&self) -> u8 {
        self.port_count
    }

    /// Allocate a new device address
    pub fn allocate_device_address(&self) -> u8 {
        self.next_device_address.fetch_add(1, Ordering::Relaxed)
    }

    /// Submit a transfer for processing
    pub async fn submit_transfer(&mut self, transfer: Transfer) -> Result<usize> {
        if self.state() != ControllerState::Running {
            return Err(UsbDriverError::ControllerError);
        }

        // Assign transfer ID
        let transfer_id = self.next_transfer_id.fetch_add(1, Ordering::Relaxed);
        let timeout = transfer.timeout();

        // Submit transfer to hardware first
        self.submit_transfer_to_hardware(&transfer).await?;

        // Queue transfer for tracking
        {
            let mut pending = self.pending_transfers.lock();
            pending.push_back(transfer);
        }

        // Process transfer
        self.process_transfers().await?;

        // Wait for completion
        let mut timeout_counter = timeout;
        loop {
            if let Some((_id, status, bytes)) = self.get_completed_transfer(transfer_id) {
                match status {
                    TransferStatus::Completed => return Ok(bytes),
                    TransferStatus::Error => return Err(UsbDriverError::TransferFailed),
                    TransferStatus::Timeout => return Err(UsbDriverError::TransferTimeout),
                    TransferStatus::Stalled => return Err(UsbDriverError::EndpointError),
                    _ => return Err(UsbDriverError::TransferFailed),
                }
            }

            if timeout_counter == 0 {
                return Err(UsbDriverError::TransferTimeout);
            }
            timeout_counter -= 1;

            // Small delay to prevent busy waiting
            // In a real OS, this would be a proper async yield
            for _ in 0..1000 { core::hint::spin_loop(); }
        }
    }

    /// Process pending transfers
    async fn process_transfers(&mut self) -> Result<()> {
        // Collect transfers to process while lock is held
        let transfers_to_process = {
            let mut pending = self.pending_transfers.lock();
            let mut transfers = Vec::new();
            while let Some(transfer) = pending.pop_front() {
                transfers.push(transfer);
            }
            transfers
        };

        // Process transfers after releasing the lock
        for transfer in transfers_to_process {
            match transfer.transfer_type() {
                TransferType::Control => {
                    self.process_control_transfer(transfer).await?;
                },
                TransferType::Bulk => {
                    self.process_bulk_transfer(transfer).await?;
                },
                TransferType::Interrupt => {
                    self.process_interrupt_transfer(transfer).await?;
                },
                TransferType::Isochronous => {
                    self.process_isochronous_transfer(transfer).await?;
                },
            }
        }

        Ok(())
    }

    /// Process control transfer
    async fn process_control_transfer(&mut self, transfer: Transfer) -> Result<()> {
        let _transfer_id = transfer.id();
        let device_address = transfer.device_address();
        let setup_packet = transfer.setup_packet().ok_or(UsbDriverError::InvalidParameter)?;

        // Create Setup Stage TRB
        let setup_trb = self.create_setup_trb(setup_packet);

        // Create Data Stage TRB if there's data
        let data_trb = if transfer.buffer().len() > 0 {
            Some(self.create_data_trb(&transfer)?)
        } else {
            None
        };

        // Create Status Stage TRB
        let status_trb = self.create_status_trb(setup_packet.is_device_to_host());

        // Queue TRBs on transfer ring for the endpoint
        self.queue_trb_on_endpoint_ring(device_address, 0, setup_trb).await?;

        if let Some(data) = data_trb {
            self.queue_trb_on_endpoint_ring(device_address, 0, data).await?;
        }

        self.queue_trb_on_endpoint_ring(device_address, 0, status_trb).await?;

        // Ring doorbell for this device/endpoint
        self.ring_doorbell(device_address, 1).await?; // DCI 1 = EP0

        Ok(())
    }

    /// Process bulk transfer
    async fn process_bulk_transfer(&mut self, transfer: Transfer) -> Result<()> {
        // Similar to control transfer but for bulk endpoints
        let transfer_id = transfer.id();
        let bytes_transferred = transfer.buffer().len();

        // Simulate successful completion
        {
            let mut completed = self.completed_transfers.lock();
            completed.push_back((transfer_id, TransferStatus::Completed, bytes_transferred));
        }

        Ok(())
    }

    /// Process interrupt transfer
    async fn process_interrupt_transfer(&mut self, transfer: Transfer) -> Result<()> {
        // Similar to control transfer but for interrupt endpoints
        let transfer_id = transfer.id();
        let bytes_transferred = transfer.buffer().len();

        // Simulate successful completion
        {
            let mut completed = self.completed_transfers.lock();
            completed.push_back((transfer_id, TransferStatus::Completed, bytes_transferred));
        }

        Ok(())
    }

    /// Process isochronous transfer
    async fn process_isochronous_transfer(&mut self, transfer: Transfer) -> Result<()> {
        // Similar to control transfer but for isochronous endpoints
        let transfer_id = transfer.id();
        let bytes_transferred = transfer.buffer().len();

        // Simulate successful completion
        {
            let mut completed = self.completed_transfers.lock();
            completed.push_back((transfer_id, TransferStatus::Completed, bytes_transferred));
        }

        Ok(())
    }

    /// Get completed transfer
    fn get_completed_transfer(&self, transfer_id: u32) -> Option<(u32, TransferStatus, usize)> {
        let mut completed = self.completed_transfers.lock();
        if let Some(pos) = completed.iter().position(|(id, _, _)| *id == transfer_id) {
            completed.remove(pos)
        } else {
            None
        }
    }

    /// Complete a transfer
    pub async fn complete_transfer(&mut self, transfer_id: u32, status: TransferStatus) -> Result<()> {
        // Find and complete the transfer
        // This would typically be called from interrupt handler

        {
            let mut completed = self.completed_transfers.lock();
            completed.push_back((transfer_id, status, 0));
        }

        Ok(())
    }

    /// Poll for events
    pub async fn poll_events(&mut self) -> Result<Vec<UsbEvent>> {
        let mut events = Vec::new();

        // Check for port status changes
        for port in 0..self.port_count {
            if self.check_port_status_change(port).await? {
                if self.is_port_connected(port).await? {
                    events.push(UsbEvent::DeviceConnected { port });
                } else {
                    // Would need to track which device was on this port
                    events.push(UsbEvent::DeviceDisconnected { address: 0 });
                }
            }
        }

        // Get queued events
        {
            let mut event_queue = self.event_queue.lock();
            while let Some(event) = event_queue.pop_front() {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Check port status change
    async fn check_port_status_change(&self, _port: u8) -> Result<bool> {
        // Read port status from xHCI registers
        // This is simplified
        Ok(false)
    }

    /// Check if port is connected
    async fn is_port_connected(&self, _port: u8) -> Result<bool> {
        // Read port status from xHCI registers
        // This is simplified
        Ok(true)
    }

    /// Get device speed for port
    pub async fn get_port_speed(&self, _port: u8) -> Result<DeviceSpeed> {
        // Read port speed from xHCI registers
        // This is simplified
        Ok(DeviceSpeed::High)
    }

    /// Reset port
    pub async fn reset_port(&mut self, _port: u8) -> Result<()> {
        // Send port reset command
        // This is simplified
        Ok(())
    }

    /// Enable slot for device
    pub async fn enable_slot(&mut self) -> Result<u8> {
        // Enable device slot in xHCI
        let slot_id = 1; // Simplified

        {
            let mut slots = self.slots.lock();
            if slot_id as usize >= slots.len() {
                return Err(UsbDriverError::ControllerError);
            }

            slots[slot_id as usize] = Some(SlotContext {
                device_address: 0,
                device: None,
                endpoints: [const { None }; 32],
            });
        }

        Ok(slot_id)
    }

    /// Disable slot
    pub async fn disable_slot(&mut self, slot_id: u8) -> Result<()> {
        {
            let mut slots = self.slots.lock();
            if (slot_id as usize) < slots.len() {
                slots[slot_id as usize] = None;
            }
        }

        Ok(())
    }

    /// Address device
    pub async fn address_device(&mut self, slot_id: u8, address: u8) -> Result<()> {
        {
            let mut slots = self.slots.lock();
            if let Some(Some(slot)) = slots.get_mut(slot_id as usize) {
                slot.device_address = address;
            } else {
                return Err(UsbDriverError::DeviceNotFound);
            }
        }

        Ok(())
    }

    /// Configure endpoint
    pub async fn configure_endpoint(
        &mut self,
        slot_id: u8,
        endpoint_address: u8,
        transfer_type: TransferType,
    ) -> Result<()> {
        {
            let mut slots = self.slots.lock();
            if let Some(Some(slot)) = slots.get_mut(slot_id as usize) {
                let ep_index = endpoint_address as usize;
                if ep_index < slot.endpoints.len() {
                    slot.endpoints[ep_index] = Some(EndpointContext {
                        endpoint_address,
                        ring: None, // Would create transfer ring
                        transfer_type,
                    });
                }
            } else {
                return Err(UsbDriverError::DeviceNotFound);
            }
        }

        Ok(())
    }

    /// Suspend controller
    pub async fn suspend(&mut self) -> Result<()> {
        self.set_state(ControllerState::Suspended);

        // Stop controller
        self.registers.operational.usbcmd.update_volatile(|cmd| {
            cmd.clear_run_stop();
        });

        // Wait for halt
        while !self.registers.operational.usbsts.read_volatile().hc_halted() {
            // Wait for controller to halt
        }

        Ok(())
    }

    /// Resume controller
    pub async fn resume(&mut self) -> Result<()> {
        // Start controller
        self.registers.operational.usbcmd.update_volatile(|cmd| {
            cmd.set_run_stop();
        });

        // Wait for run state
        while self.registers.operational.usbsts.read_volatile().hc_halted() {
            // Wait for controller to start running
        }

        self.set_state(ControllerState::Running);
        Ok(())
    }

    /// Shutdown controller
    pub async fn shutdown(&mut self) -> Result<()> {
        // Stop controller
        self.registers.operational.usbcmd.update_volatile(|cmd| {
            cmd.clear_run_stop();
        });

        // Wait for halt
        while !self.registers.operational.usbsts.read_volatile().hc_halted() {
            // Wait for controller to halt
        }

        self.set_state(ControllerState::Uninitialized);
        Ok(())
    }

    /// Allocate device context base address array
    async fn allocate_device_context_base_array(&self) -> Result<u64> {
        // Allocate memory for device context base address array
        // Each entry is 8 bytes, need 256 entries (max device slots + 1)
        let size = 256 * 8;
        let ptr = self.allocate_dma_memory(size, 64).await?;

        // Zero the array
        unsafe {
            core::ptr::write_bytes(ptr as *mut u8, 0, size);
        }

        Ok(ptr)
    }

    /// Allocate command ring
    async fn allocate_command_ring(&self) -> Result<(u64, bool)> {
        // Allocate memory for command ring (64 TRBs = 1024 bytes)
        let size = 64 * 16; // 16 bytes per TRB
        let ptr = self.allocate_dma_memory(size, 64).await?;

        // Zero the ring
        unsafe {
            core::ptr::write_bytes(ptr as *mut u8, 0, size);
        }

        Ok((ptr, true)) // Return pointer and initial cycle state
    }

    /// Allocate event ring
    async fn allocate_event_ring(&self) -> Result<u64> {
        // Allocate event ring segment table (16 bytes per entry)
        let segment_table_size = 16;
        let segment_table_ptr = self.allocate_dma_memory(segment_table_size, 64).await?;

        // Allocate actual event ring (256 TRBs = 4096 bytes)
        let event_ring_size = 256 * 16;
        let event_ring_ptr = self.allocate_dma_memory(event_ring_size, 64).await?;

        // Set up segment table entry
        unsafe {
            let segment_table = segment_table_ptr as *mut u64;
            *segment_table = event_ring_ptr;
            *segment_table.offset(1) = 256; // Size in TRBs
        }

        Ok(segment_table_ptr)
    }

    /// Allocate DMA memory
    async fn allocate_dma_memory(&self, size: usize, alignment: usize) -> Result<u64> {
        // In a real OS, this would allocate DMA-coherent memory
        use alloc::alloc::{alloc, Layout};

        let layout = Layout::from_size_align(size, alignment)
            .map_err(|_| UsbDriverError::MemoryError)?;

        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(UsbDriverError::MemoryError);
        }

        Ok(ptr as u64)
    }

    /// Process event ring TRBs
    async fn process_event_ring(&mut self) -> Result<()> {
        // Read event ring dequeue pointer from primary interrupter
        // Note: The exact method to access interrupter registers depends on xHCI crate implementation
        // For now, we'll implement a placeholder that handles event processing

        // In a real implementation, this would:
        // 1. Read the Event Ring Dequeue Pointer (ERDP) register
        // 2. Process each TRB in the event ring up to the current hardware position
        // 3. Update the ERDP to indicate which events have been processed

        Ok(())
    }

    /// Process event ring TRBs (synchronous version for interrupt context)
    fn process_event_ring_sync(&mut self) -> Result<()> {
        // Synchronous version for use in interrupt handlers
        // Note: The exact method to access interrupter registers depends on xHCI crate implementation
        // For now, we'll implement a placeholder that handles event processing

        // In a real implementation, this would:
        // 1. Read the Event Ring Dequeue Pointer (ERDP) register
        // 2. Process each TRB in the event ring up to the current hardware position
        // 3. Update the ERDP to indicate which events have been processed

        Ok(())
    }

    /// Submit transfer to hardware
    async fn submit_transfer_to_hardware(&mut self, _transfer: &Transfer) -> Result<()> {
        // This submits the transfer TRBs to the hardware via the appropriate transfer ring
        // The actual TRB creation and queueing is handled in the process_*_transfer methods
        Ok(())
    }

    /// Create Setup Stage TRB for control transfers
    fn create_setup_trb(&self, setup_packet: &crate::transfer::SetupPacket) -> u128 {
        // Create a Setup Stage TRB (Transfer Request Block)
        // This is a 128-bit structure for xHCI
        let mut trb: u128 = 0;

        // Setup data (8 bytes of setup packet)
        let setup_data = unsafe {
            core::mem::transmute::<crate::transfer::SetupPacket, u64>(*setup_packet)
        };
        trb |= setup_data as u128;

        // TRB Control field (upper 64 bits)
        // Setup Stage TRB type = 2, other flags as needed
        trb |= (2u128 << 90); // TRB Type field

        trb
    }

    /// Create Data Stage TRB for control transfers
    fn create_data_trb(&self, transfer: &Transfer) -> Result<u128> {
        let mut trb: u128 = 0;

        // Data buffer pointer (would be physical address)
        let buffer_ptr = transfer.buffer().as_slice().map(|s| s.as_ptr() as u64).unwrap_or(0);
        trb |= buffer_ptr as u128;

        // Transfer length and TRB type
        let length = transfer.buffer().len() as u32;
        trb |= ((length as u128) << 64);
        trb |= (3u128 << 90); // Data Stage TRB type = 3

        Ok(trb)
    }

    /// Create Status Stage TRB for control transfers
    fn create_status_trb(&self, device_to_host: bool) -> u128 {
        let mut trb: u128 = 0;

        // Status Stage TRB type = 4
        trb |= (4u128 << 90);

        // Direction bit
        if device_to_host {
            trb |= (1u128 << 80); // DIR bit
        }

        trb
    }

    /// Queue TRB on endpoint transfer ring
    async fn queue_trb_on_endpoint_ring(&mut self, device_address: u8, _endpoint: u8, _trb: u128) -> Result<()> {
        // In a real implementation, this would:
        // 1. Find the appropriate transfer ring for the device/endpoint
        // 2. Queue the TRB on that ring
        // 3. Update the enqueue pointer
        // 4. Handle ring wrap-around and cycle bit

        // For now, just validate parameters
        if device_address == 0 || device_address > 127 {
            return Err(UsbDriverError::InvalidParameter);
        }

        Ok(())
    }

    /// Ring doorbell for device/endpoint
    async fn ring_doorbell(&mut self, device_address: u8, dci: u8) -> Result<()> {
        // Ring the doorbell register to notify hardware of queued TRBs
        self.registers.doorbell.update_volatile_at(device_address as usize, |doorbell| {
            doorbell.set_doorbell_target(dci);
            doorbell.set_doorbell_stream_id(0);
        });

        Ok(())
    }

    /// Handle interrupt (called from interrupt handler)
    pub fn handle_interrupt(&mut self) -> Result<()> {
        // Read interrupt status
        let status = self.registers.operational.usbsts.read_volatile();

        if status.host_controller_error() {
            self.set_state(ControllerState::Error);
            let mut event_queue = self.event_queue.lock();
            event_queue.push_back(UsbEvent::Error {
                error: UsbDriverError::ControllerError,
            });
        }

        if status.port_change_detect() {
            // Handle port changes
            for port in 0..self.port_count {
                let port_status = self.registers.port_register_set.read_volatile_at(port as usize);
                if port_status.portsc.connect_status_change() {
                    if port_status.portsc.current_connect_status() {
                        let mut event_queue = self.event_queue.lock();
                        event_queue.push_back(UsbEvent::DeviceConnected { port });
                    } else {
                        let mut event_queue = self.event_queue.lock();
                        event_queue.push_back(UsbEvent::DeviceDisconnected { address: 0 });
                    }
                }
            }
        }

        // Process event ring - handle this synchronously in interrupt context
        self.process_event_ring_sync()?;

        // Clear interrupt status
        self.registers.operational.usbsts.update_volatile(|sts| {
            if status.event_interrupt() {
                sts.set_0_event_interrupt();
            }
            if status.port_change_detect() {
                sts.set_0_port_change_detect();
            }
        });

        Ok(())
    }
}

impl fmt::Debug for UsbController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsbController")
            .field("state", &self.state())
            .field("port_count", &self.port_count)
            .finish()
    }
}

unsafe impl Send for UsbController {}
unsafe impl Sync for UsbController {}