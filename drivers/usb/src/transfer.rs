//! USB Transfer Management

use alloc::{vec, vec::Vec, boxed::Box, sync::Arc};
use core::{
    fmt,
    sync::atomic::{AtomicU32, Ordering},
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use spin::Mutex;
use crate::{Result, UsbDriverError, endpoint::EndpointDirection};

/// USB Transfer Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferType {
    Control,
    Bulk,
    Interrupt,
    Isochronous,
}

/// USB Transfer Status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStatus {
    Pending,
    Completed,
    Error,
    Cancelled,
    Stalled,
    Timeout,
    Overflow,
    Underflow,
}

/// USB Setup Packet for control transfers
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SetupPacket {
    /// Request type and direction
    pub request_type: u8,
    /// Specific request
    pub request: u8,
    /// Request-specific parameter
    pub value: u16,
    /// Request-specific parameter
    pub index: u16,
    /// Number of bytes to transfer in data stage
    pub length: u16,
}

impl SetupPacket {
    /// Create a new setup packet
    pub fn new(
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        length: u16,
    ) -> Self {
        Self {
            request_type,
            request,
            value,
            index,
            length,
        }
    }

    /// Create a GET_DESCRIPTOR request
    pub fn get_descriptor(descriptor_type: u8, descriptor_index: u8, language_id: u16, length: u16) -> Self {
        Self::new(
            0x80, // Device to host, standard, device
            0x06, // GET_DESCRIPTOR
            ((descriptor_type as u16) << 8) | (descriptor_index as u16),
            language_id,
            length,
        )
    }

    /// Create a SET_ADDRESS request
    pub fn set_address(address: u8) -> Self {
        Self::new(
            0x00, // Host to device, standard, device
            0x05, // SET_ADDRESS
            address as u16,
            0,
            0,
        )
    }

    /// Create a SET_CONFIGURATION request
    pub fn set_configuration(config: u8) -> Self {
        Self::new(
            0x00, // Host to device, standard, device
            0x09, // SET_CONFIGURATION
            config as u16,
            0,
            0,
        )
    }

    /// Create a CLEAR_FEATURE request
    pub fn clear_feature(feature: u16, index: u16) -> Self {
        Self::new(
            0x02, // Host to device, standard, endpoint
            0x01, // CLEAR_FEATURE
            feature,
            index,
            0,
        )
    }

    /// Check if this is a device-to-host transfer
    pub fn is_device_to_host(&self) -> bool {
        (self.request_type & 0x80) != 0
    }

    /// Get request type bits
    pub fn request_type_bits(&self) -> (u8, u8, u8) {
        let direction = (self.request_type >> 7) & 1;
        let req_type = (self.request_type >> 5) & 3;
        let recipient = self.request_type & 0x1F;
        (direction, req_type, recipient)
    }
}

/// USB Transfer Buffer
pub enum TransferBuffer {
    /// Buffer for single transfer
    Single(Vec<u8>),
    /// Scatter-gather list for large transfers
    ScatterGather(Vec<Vec<u8>>),
    /// External buffer (not owned by transfer)
    External(*mut u8, usize),
}

impl TransferBuffer {
    /// Create a new single buffer
    pub fn new(size: usize) -> Self {
        TransferBuffer::Single(vec![0; size])
    }

    /// Create from existing data
    pub fn from_data(data: Vec<u8>) -> Self {
        TransferBuffer::Single(data)
    }

    /// Create scatter-gather buffer
    pub fn scatter_gather(buffers: Vec<Vec<u8>>) -> Self {
        TransferBuffer::ScatterGather(buffers)
    }

    /// Create external buffer reference
    pub unsafe fn external(ptr: *mut u8, len: usize) -> Self {
        TransferBuffer::External(ptr, len)
    }

    /// Get total length of buffer
    pub fn len(&self) -> usize {
        match self {
            TransferBuffer::Single(buf) => buf.len(),
            TransferBuffer::ScatterGather(buffers) => {
                buffers.iter().map(|b| b.len()).sum()
            },
            TransferBuffer::External(_, len) => *len,
        }
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get mutable slice for single buffer
    pub fn as_mut_slice(&mut self) -> Option<&mut [u8]> {
        match self {
            TransferBuffer::Single(buf) => Some(buf.as_mut_slice()),
            _ => None,
        }
    }

    /// Get slice for single buffer
    pub fn as_slice(&self) -> Option<&[u8]> {
        match self {
            TransferBuffer::Single(buf) => Some(buf.as_slice()),
            _ => None,
        }
    }
}

impl fmt::Debug for TransferBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransferBuffer::Single(buf) => {
                f.debug_struct("Single")
                    .field("len", &buf.len())
                    .finish()
            },
            TransferBuffer::ScatterGather(buffers) => {
                f.debug_struct("ScatterGather")
                    .field("buffers", &buffers.len())
                    .field("total_len", &self.len())
                    .finish()
            },
            TransferBuffer::External(ptr, len) => {
                f.debug_struct("External")
                    .field("ptr", ptr)
                    .field("len", len)
                    .finish()
            },
        }
    }
}

/// USB Transfer completion callback
type TransferCallback = Box<dyn FnOnce(TransferStatus, usize) + Send>;

/// USB Transfer
pub struct Transfer {
    /// Unique transfer ID
    id: u32,
    /// Device address
    device_address: u8,
    /// Endpoint address
    endpoint_address: u8,
    /// Transfer type
    transfer_type: TransferType,
    /// Transfer buffer
    buffer: TransferBuffer,
    /// Setup packet for control transfers
    setup_packet: Option<SetupPacket>,
    /// Transfer status
    status: Arc<Mutex<TransferStatus>>,
    /// Bytes transferred
    bytes_transferred: Arc<AtomicU32>,
    /// Completion waker
    waker: Arc<Mutex<Option<Waker>>>,
    /// Completion callback
    callback: Option<TransferCallback>,
    /// Transfer timeout in milliseconds
    timeout_ms: u32,
    /// Transfer flags
    flags: u32,
}

/// Transfer flags
pub mod transfer_flags {
    pub const NONE: u32 = 0;
    pub const SHORT_NOT_OK: u32 = 1 << 0;
    pub const ZERO_PACKET: u32 = 1 << 1;
    pub const NO_INTERRUPT: u32 = 1 << 2;
}

static TRANSFER_ID_COUNTER: AtomicU32 = AtomicU32::new(1);

impl Transfer {
    /// Create a new transfer
    pub fn new(
        device_address: u8,
        endpoint_address: u8,
        transfer_type: TransferType,
        buffer: TransferBuffer,
    ) -> Self {
        Self {
            id: TRANSFER_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            device_address,
            endpoint_address,
            transfer_type,
            buffer,
            setup_packet: None,
            status: Arc::new(Mutex::new(TransferStatus::Pending)),
            bytes_transferred: Arc::new(AtomicU32::new(0)),
            waker: Arc::new(Mutex::new(None)),
            callback: None,
            timeout_ms: 5000, // 5 second default timeout
            flags: transfer_flags::NONE,
        }
    }

    /// Create a control transfer
    pub fn control(
        device_address: u8,
        setup_packet: SetupPacket,
        buffer: TransferBuffer,
    ) -> Self {
        let mut transfer = Self::new(
            device_address,
            0, // Control endpoint
            TransferType::Control,
            buffer,
        );
        transfer.setup_packet = Some(setup_packet);
        transfer
    }

    /// Create a bulk transfer
    pub fn bulk(
        device_address: u8,
        endpoint_address: u8,
        buffer: TransferBuffer,
    ) -> Self {
        Self::new(device_address, endpoint_address, TransferType::Bulk, buffer)
    }

    /// Create an interrupt transfer
    pub fn interrupt(
        device_address: u8,
        endpoint_address: u8,
        buffer: TransferBuffer,
    ) -> Self {
        Self::new(device_address, endpoint_address, TransferType::Interrupt, buffer)
    }

    /// Create an isochronous transfer
    pub fn isochronous(
        device_address: u8,
        endpoint_address: u8,
        buffer: TransferBuffer,
    ) -> Self {
        Self::new(device_address, endpoint_address, TransferType::Isochronous, buffer)
    }

    /// Get transfer ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get device address
    pub fn device_address(&self) -> u8 {
        self.device_address
    }

    /// Get endpoint address
    pub fn endpoint_address(&self) -> u8 {
        self.endpoint_address
    }

    /// Get transfer type
    pub fn transfer_type(&self) -> TransferType {
        self.transfer_type
    }

    /// Get buffer reference
    pub fn buffer(&self) -> &TransferBuffer {
        &self.buffer
    }

    /// Get mutable buffer reference
    pub fn buffer_mut(&mut self) -> &mut TransferBuffer {
        &mut self.buffer
    }

    /// Get setup packet
    pub fn setup_packet(&self) -> Option<&SetupPacket> {
        self.setup_packet.as_ref()
    }

    /// Get transfer status
    pub fn status(&self) -> TransferStatus {
        *self.status.lock()
    }

    /// Set transfer status
    pub fn set_status(&self, status: TransferStatus) {
        *self.status.lock() = status;

        // Wake up any waiting futures
        if let Some(waker) = self.waker.lock().take() {
            waker.wake();
        }

        // Call completion callback if available
        // Note: This is simplified - in practice, you'd need to handle this more carefully
    }

    /// Get bytes transferred
    pub fn bytes_transferred(&self) -> u32 {
        self.bytes_transferred.load(Ordering::Acquire)
    }

    /// Set bytes transferred
    pub fn set_bytes_transferred(&self, bytes: u32) {
        self.bytes_transferred.store(bytes, Ordering::Release);
    }

    /// Set timeout
    pub fn set_timeout(&mut self, timeout_ms: u32) {
        self.timeout_ms = timeout_ms;
    }

    /// Get timeout
    pub fn timeout(&self) -> u32 {
        self.timeout_ms
    }

    /// Set flags
    pub fn set_flags(&mut self, flags: u32) {
        self.flags = flags;
    }

    /// Get flags
    pub fn flags(&self) -> u32 {
        self.flags
    }

    /// Set completion callback
    pub fn set_callback(&mut self, callback: TransferCallback) {
        self.callback = Some(callback);
    }

    /// Check if transfer is complete
    pub fn is_complete(&self) -> bool {
        match self.status() {
            TransferStatus::Pending => false,
            _ => true,
        }
    }

    /// Cancel the transfer
    pub fn cancel(&self) {
        self.set_status(TransferStatus::Cancelled);
    }

    /// Get endpoint direction
    pub fn direction(&self) -> EndpointDirection {
        if self.endpoint_address & 0x80 != 0 {
            EndpointDirection::In
        } else {
            EndpointDirection::Out
        }
    }

    /// Check if this is an IN transfer
    pub fn is_in(&self) -> bool {
        self.direction() == EndpointDirection::In
    }

    /// Check if this is an OUT transfer
    pub fn is_out(&self) -> bool {
        self.direction() == EndpointDirection::Out
    }
}

impl Future for Transfer {
    type Output = Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.status() {
            TransferStatus::Pending => {
                // Store waker for when transfer completes
                *self.waker.lock() = Some(cx.waker().clone());
                Poll::Pending
            },
            TransferStatus::Completed => {
                Poll::Ready(Ok(self.bytes_transferred() as usize))
            },
            TransferStatus::Error => {
                Poll::Ready(Err(UsbDriverError::TransferFailed))
            },
            TransferStatus::Cancelled => {
                Poll::Ready(Err(UsbDriverError::TransferFailed))
            },
            TransferStatus::Stalled => {
                Poll::Ready(Err(UsbDriverError::EndpointError))
            },
            TransferStatus::Timeout => {
                Poll::Ready(Err(UsbDriverError::TransferTimeout))
            },
            TransferStatus::Overflow => {
                Poll::Ready(Err(UsbDriverError::BufferOverflow))
            },
            TransferStatus::Underflow => {
                Poll::Ready(Err(UsbDriverError::TransferFailed))
            },
        }
    }
}

impl fmt::Debug for Transfer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transfer")
            .field("id", &self.id)
            .field("device_address", &self.device_address)
            .field("endpoint_address", &format_args!("{:#04x}", self.endpoint_address))
            .field("transfer_type", &self.transfer_type)
            .field("buffer", &self.buffer)
            .field("status", &self.status())
            .field("bytes_transferred", &self.bytes_transferred())
            .finish()
    }
}

unsafe impl Send for Transfer {}
unsafe impl Sync for Transfer {}

/// Transfer builder for easier construction
pub struct TransferBuilder {
    device_address: u8,
    endpoint_address: u8,
    transfer_type: TransferType,
    buffer_size: Option<usize>,
    buffer_data: Option<Vec<u8>>,
    setup_packet: Option<SetupPacket>,
    timeout_ms: u32,
    flags: u32,
}

impl TransferBuilder {
    /// Create a new transfer builder
    pub fn new(device_address: u8, endpoint_address: u8, transfer_type: TransferType) -> Self {
        Self {
            device_address,
            endpoint_address,
            transfer_type,
            buffer_size: None,
            buffer_data: None,
            setup_packet: None,
            timeout_ms: 5000,
            flags: transfer_flags::NONE,
        }
    }

    /// Set buffer size (creates empty buffer)
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = Some(size);
        self
    }

    /// Set buffer data
    pub fn buffer_data(mut self, data: Vec<u8>) -> Self {
        self.buffer_data = Some(data);
        self
    }

    /// Set setup packet (for control transfers)
    pub fn setup_packet(mut self, packet: SetupPacket) -> Self {
        self.setup_packet = Some(packet);
        self
    }

    /// Set timeout
    pub fn timeout(mut self, timeout_ms: u32) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set flags
    pub fn flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    /// Build the transfer
    pub fn build(self) -> Transfer {
        let buffer = if let Some(data) = self.buffer_data {
            TransferBuffer::from_data(data)
        } else if let Some(size) = self.buffer_size {
            TransferBuffer::new(size)
        } else {
            TransferBuffer::new(0)
        };

        let mut transfer = Transfer::new(
            self.device_address,
            self.endpoint_address,
            self.transfer_type,
            buffer,
        );

        transfer.setup_packet = self.setup_packet;
        transfer.timeout_ms = self.timeout_ms;
        transfer.flags = self.flags;

        transfer
    }
}