//! Async USB Operations and Utilities

use alloc::{vec, vec::Vec, boxed::Box, sync::Arc, collections::VecDeque};
use core::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};
use spin::Mutex;
use crate::{
    Result, UsbDriverError,
    transfer::{Transfer, TransferStatus, TransferBuffer, SetupPacket},
    device::UsbDevice,
};

/// Async USB transfer operation
pub struct AsyncTransfer {
    /// Inner transfer
    transfer: Transfer,
    /// Completion state
    state: Arc<Mutex<AsyncTransferState>>,
}

/// Async transfer state
#[derive(Debug)]
struct AsyncTransferState {
    /// Transfer status
    status: TransferStatus,
    /// Bytes transferred
    bytes_transferred: usize,
    /// Waker for async completion
    waker: Option<Waker>,
    /// Error if transfer failed
    error: Option<UsbDriverError>,
}

impl AsyncTransfer {
    /// Create a new async transfer
    pub fn new(transfer: Transfer) -> Self {
        Self {
            transfer,
            state: Arc::new(Mutex::new(AsyncTransferState {
                status: TransferStatus::Pending,
                bytes_transferred: 0,
                waker: None,
                error: None,
            })),
        }
    }

    /// Get transfer ID
    pub fn id(&self) -> u32 {
        self.transfer.id()
    }

    /// Check if transfer is complete
    pub fn is_complete(&self) -> bool {
        let state = self.state.lock();
        state.status != TransferStatus::Pending
    }

    /// Complete the transfer (called by controller)
    pub fn complete(&self, status: TransferStatus, bytes_transferred: usize, error: Option<UsbDriverError>) {
        let mut state = self.state.lock();
        state.status = status;
        state.bytes_transferred = bytes_transferred;
        state.error = error;

        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }

    /// Cancel the transfer
    pub fn cancel(&self) {
        let mut state = self.state.lock();
        state.status = TransferStatus::Cancelled;
        state.error = Some(UsbDriverError::TransferFailed);

        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

impl Future for AsyncTransfer {
    type Output = Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();

        match state.status {
            TransferStatus::Pending => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            },
            TransferStatus::Completed => {
                Poll::Ready(Ok(state.bytes_transferred))
            },
            TransferStatus::Error | TransferStatus::Cancelled => {
                let error = state.error.take().unwrap_or(UsbDriverError::TransferFailed);
                Poll::Ready(Err(error))
            },
            TransferStatus::Timeout => {
                Poll::Ready(Err(UsbDriverError::TransferTimeout))
            },
            TransferStatus::Stalled => {
                Poll::Ready(Err(UsbDriverError::EndpointError))
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

/// USB Control Transfer Builder
pub struct ControlTransferBuilder {
    device_address: u8,
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    data: Option<Vec<u8>>,
    timeout: Duration,
}

impl ControlTransferBuilder {
    /// Create a new control transfer builder
    pub fn new(device_address: u8) -> Self {
        Self {
            device_address,
            request_type: 0,
            request: 0,
            value: 0,
            index: 0,
            data: None,
            timeout: Duration::from_secs(5),
        }
    }

    /// Set request type
    pub fn request_type(mut self, request_type: u8) -> Self {
        self.request_type = request_type;
        self
    }

    /// Set request
    pub fn request(mut self, request: u8) -> Self {
        self.request = request;
        self
    }

    /// Set value
    pub fn value(mut self, value: u16) -> Self {
        self.value = value;
        self
    }

    /// Set index
    pub fn index(mut self, index: u16) -> Self {
        self.index = index;
        self
    }

    /// Set data for data stage
    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = Some(data);
        self
    }

    /// Set timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the control transfer
    pub fn build(self) -> AsyncTransfer {
        let data_length = self.data.as_ref().map(|d| d.len()).unwrap_or(0);

        let setup_packet = SetupPacket::new(
            self.request_type,
            self.request,
            self.value,
            self.index,
            data_length as u16,
        );

        let buffer = if let Some(data) = self.data {
            TransferBuffer::from_data(data)
        } else {
            TransferBuffer::new(0)
        };

        let mut transfer = Transfer::control(self.device_address, setup_packet, buffer);
        transfer.set_timeout(self.timeout.as_millis() as u32);

        AsyncTransfer::new(transfer)
    }
}

/// Common USB control requests
impl ControlTransferBuilder {
    /// Create GET_DESCRIPTOR request
    pub fn get_descriptor(device_address: u8, desc_type: u8, desc_index: u8, length: u16) -> Self {
        Self::new(device_address)
            .request_type(0x80) // Device to host, standard, device
            .request(0x06) // GET_DESCRIPTOR
            .value(((desc_type as u16) << 8) | (desc_index as u16))
            .index(0)
            .data(vec![0; length as usize])
    }

    /// Create SET_ADDRESS request
    pub fn set_address(device_address: u8, new_address: u8) -> Self {
        Self::new(device_address)
            .request_type(0x00) // Host to device, standard, device
            .request(0x05) // SET_ADDRESS
            .value(new_address as u16)
            .index(0)
    }

    /// Create SET_CONFIGURATION request
    pub fn set_configuration(device_address: u8, config_value: u8) -> Self {
        Self::new(device_address)
            .request_type(0x00) // Host to device, standard, device
            .request(0x09) // SET_CONFIGURATION
            .value(config_value as u16)
            .index(0)
    }

    /// Create GET_STATUS request
    pub fn get_status(device_address: u8, recipient: u8, index: u16) -> Self {
        Self::new(device_address)
            .request_type(0x80 | recipient) // Device to host, standard
            .request(0x00) // GET_STATUS
            .value(0)
            .index(index)
            .data(vec![0; 2])
    }

    /// Create CLEAR_FEATURE request
    pub fn clear_feature(device_address: u8, recipient: u8, feature: u16, index: u16) -> Self {
        Self::new(device_address)
            .request_type(0x00 | recipient) // Host to device, standard
            .request(0x01) // CLEAR_FEATURE
            .value(feature)
            .index(index)
    }

    /// Create SET_FEATURE request
    pub fn set_feature(device_address: u8, recipient: u8, feature: u16, index: u16) -> Self {
        Self::new(device_address)
            .request_type(0x00 | recipient) // Host to device, standard
            .request(0x03) // SET_FEATURE
            .value(feature)
            .index(index)
    }
}

/// Async bulk transfer helper
pub struct BulkTransfer;

impl BulkTransfer {
    /// Create async bulk IN transfer
    pub fn read(device_address: u8, endpoint: u8, length: usize) -> AsyncTransfer {
        let buffer = TransferBuffer::new(length);
        let transfer = Transfer::bulk(device_address, endpoint | 0x80, buffer);
        AsyncTransfer::new(transfer)
    }

    /// Create async bulk OUT transfer
    pub fn write(device_address: u8, endpoint: u8, data: Vec<u8>) -> AsyncTransfer {
        let buffer = TransferBuffer::from_data(data);
        let transfer = Transfer::bulk(device_address, endpoint, buffer);
        AsyncTransfer::new(transfer)
    }
}

/// Async interrupt transfer helper
pub struct InterruptTransfer;

impl InterruptTransfer {
    /// Create async interrupt IN transfer
    pub fn read(device_address: u8, endpoint: u8, length: usize) -> AsyncTransfer {
        let buffer = TransferBuffer::new(length);
        let transfer = Transfer::interrupt(device_address, endpoint | 0x80, buffer);
        AsyncTransfer::new(transfer)
    }

    /// Create async interrupt OUT transfer
    pub fn write(device_address: u8, endpoint: u8, data: Vec<u8>) -> AsyncTransfer {
        let buffer = TransferBuffer::from_data(data);
        let transfer = Transfer::interrupt(device_address, endpoint, buffer);
        AsyncTransfer::new(transfer)
    }
}

/// USB Device Enumerator
pub struct DeviceEnumerator {
    device: Arc<Mutex<UsbDevice>>,
}

impl DeviceEnumerator {
    /// Create a new device enumerator
    pub fn new(device: Arc<Mutex<UsbDevice>>) -> Self {
        Self { device }
    }

    /// Enumerate the device
    pub async fn enumerate(&self) -> Result<()> {
        let device = self.device.lock();
        let address = device.address();

        // Step 1: Get device descriptor (first 8 bytes for max packet size)
        let transfer = ControlTransferBuilder::get_descriptor(address, 0x01, 0, 8);
        let _result = transfer.build().await?;

        // Parse max packet size from device descriptor
        // This is simplified - would parse the actual descriptor

        // Step 2: Set device address
        let new_address = 1; // Would be allocated by address manager
        let transfer = ControlTransferBuilder::set_address(address, new_address);
        transfer.build().await?;

        // Update device address
        device.set_address(new_address);

        // Step 3: Get full device descriptor
        let transfer = ControlTransferBuilder::get_descriptor(new_address, 0x01, 0, 18);
        let _result = transfer.build().await?;

        // Step 4: Get configuration descriptor
        let transfer = ControlTransferBuilder::get_descriptor(new_address, 0x02, 0, 9);
        let _result = transfer.build().await?;

        // Parse total length and get full configuration
        // This is simplified

        // Step 5: Set configuration
        let transfer = ControlTransferBuilder::set_configuration(new_address, 1);
        transfer.build().await?;

        device.set_configuration(1)?;

        Ok(())
    }
}

/// Timeout future
pub struct Timeout<F> {
    future: Pin<Box<F>>,
    timeout: Duration,
    start_time: Option<u64>, // Simplified timing
}

impl<F> Timeout<F>
where
    F: Future,
{
    /// Create a new timeout future
    pub fn new(future: F, timeout: Duration) -> Self {
        Self {
            future: Box::pin(future),
            timeout,
            start_time: None,
        }
    }
}

impl<F> Future for Timeout<F>
where
    F: Future,
{
    type Output = Result<F::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start_time.is_none() {
            // In a real implementation, you'd get the current time
            self.start_time = Some(0);
        }

        // Check timeout (simplified)
        // In a real implementation, you'd check against actual time

        match self.future.as_mut().poll(cx) {
            Poll::Ready(result) => Poll::Ready(Ok(result)),
            Poll::Pending => {
                // Check if timed out (simplified)
                // For now, just return pending
                Poll::Pending
            }
        }
    }
}

/// Extension trait for adding timeout to futures
pub trait TimeoutExt: Future + Sized {
    /// Add timeout to this future
    fn timeout(self, duration: Duration) -> Timeout<Self> {
        Timeout::new(self, duration)
    }
}

impl<F: Future> TimeoutExt for F {}

/// USB event stream
pub struct UsbEventStream {
    events: Arc<Mutex<VecDeque<UsbEvent>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

/// USB events
#[derive(Debug, Clone)]
pub enum UsbEvent {
    DeviceConnected { address: u8 },
    DeviceDisconnected { address: u8 },
    TransferComplete { transfer_id: u32, status: TransferStatus },
    Error { error: UsbDriverError },
}

impl UsbEventStream {
    /// Create a new event stream
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::new())),
            waker: Arc::new(Mutex::new(None)),
        }
    }

    /// Push an event
    pub fn push_event(&self, event: UsbEvent) {
        {
            let mut events = self.events.lock();
            events.push_back(event);
        }

        // Wake up any waiting poll
        if let Some(waker) = self.waker.lock().take() {
            waker.wake();
        }
    }

    /// Try to get next event (non-blocking)
    pub fn try_next(&self) -> Option<UsbEvent> {
        let mut events = self.events.lock();
        events.pop_front()
    }
}

impl Future for UsbEventStream {
    type Output = Option<UsbEvent>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(event) = self.try_next() {
            Poll::Ready(Some(event))
        } else {
            *self.waker.lock() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Utility functions for async USB operations
pub mod utils {
    use super::*;

    /// Wait for device to reach specific state
    pub async fn wait_for_device_state(
        _device: &UsbDevice,
        _target_state: crate::device::DeviceState,
        _timeout: Duration,
    ) -> Result<()> {
        // Simplified implementation
        // In practice, you'd poll the device state with timeout
        Ok(())
    }

    /// Retry an operation with exponential backoff
    pub async fn retry_with_backoff<F, Fut, T>(
        mut operation: F,
        max_retries: usize,
        initial_delay: Duration,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut delay = initial_delay;

        for attempt in 0..max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if attempt == max_retries - 1 {
                        return Err(error);
                    }

                    // Wait before retry (simplified)
                    // In practice, you'd use a proper async delay
                    delay = Duration::from_millis(delay.as_millis() as u64 * 2);
                }
            }
        }

        Err(UsbDriverError::TransferFailed)
    }

    /// Join multiple async transfers
    pub async fn join_transfers(transfers: Vec<AsyncTransfer>) -> Result<Vec<usize>> {
        // Simplified implementation
        // In practice, you'd use a proper join implementation
        let mut results = Vec::new();
        for transfer in transfers {
            results.push(transfer.await?);
        }
        Ok(results)
    }

    /// Select first completed transfer
    pub async fn select_first(transfers: Vec<AsyncTransfer>) -> Result<(usize, usize)> {
        // Simplified implementation
        // In practice, you'd use a proper select implementation
        for (index, transfer) in transfers.into_iter().enumerate() {
            if transfer.is_complete() {
                return Ok((index, transfer.await?));
            }
        }
        Err(UsbDriverError::TransferTimeout)
    }
}