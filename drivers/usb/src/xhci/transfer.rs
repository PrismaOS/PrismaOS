/// USB Transfer Management for xHCI
///
/// This module implements the four USB transfer types:
/// - Control transfers (setup/data/status phases)
/// - Bulk transfers (large data transfers)
/// - Interrupt transfers (periodic data)
/// - Isochronous transfers (time-critical data)

use super::trb::{Trb, TrbType, SetupStageTrb, DataStageTrb, StatusStageTrb, NormalTrb};
use super::ring::TransferRing;
use super::context::EndpointContext;
use crate::error::{UsbError, Result};
use crate::types::{UsbDirection, UsbSpeed, UsbEndpoint, UsbRequest};
use alloc::{vec::Vec, boxed::Box, vec};
use core::mem;

/// USB Transfer Request
#[derive(Debug, Clone)]
pub struct UsbTransferRequest {
    /// Device address
    pub device_address: u8,
    /// Endpoint number
    pub endpoint: u8,
    /// Transfer direction
    pub direction: UsbDirection,
    /// Transfer type
    pub transfer_type: UsbTransferType,
    /// Data buffer
    pub data: Vec<u8>,
    /// Setup packet for control transfers
    pub setup_packet: Option<UsbRequest>,
    /// Completion callback (removed for now to ensure Send + Sync)
    /// Transfer ID for tracking
    pub transfer_id: u32,
}

/// USB Transfer Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbTransferType {
    /// Control transfer (setup/data/status)
    Control,
    /// Bulk transfer (large data)
    Bulk,
    /// Interrupt transfer (periodic)
    Interrupt,
    /// Isochronous transfer (time-critical)
    Isochronous,
}

/// USB Transfer Status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbTransferStatus {
    /// Transfer is pending
    Pending,
    /// Transfer is in progress
    InProgress,
    /// Transfer completed successfully
    Completed,
    /// Transfer failed with error
    Failed,
    /// Transfer was cancelled
    Cancelled,
}

/// USB Transfer Result
#[derive(Debug)]
pub struct UsbTransferResult {
    /// Transfer ID
    pub transfer_id: u32,
    /// Transfer status
    pub status: UsbTransferStatus,
    /// Transferred data (for IN transfers)
    pub data: Vec<u8>,
    /// Actual transfer length
    pub actual_length: usize,
    /// Error code if failed
    pub error: Option<UsbError>,
}

/// USB Transfer Manager
pub struct UsbTransferManager {
    /// Pending transfers by transfer ID
    pending_transfers: Vec<PendingTransfer>,
    /// Next transfer ID
    next_transfer_id: u32,
    /// Maximum transfer size
    max_transfer_size: usize,
}

/// Pending Transfer
struct PendingTransfer {
    /// Transfer request
    request: UsbTransferRequest,
    /// Transfer status
    status: UsbTransferStatus,
    /// Submitted TRBs
    submitted_trbs: Vec<Trb>,
    /// Result data
    result_data: Vec<u8>,
    /// Actual transfer length
    actual_length: usize,
}

impl UsbTransferManager {
    /// Create a new transfer manager
    pub fn new() -> Self {
        Self {
            pending_transfers: Vec::new(),
            next_transfer_id: 1,
            max_transfer_size: 65536, // 64KB max transfer
        }
    }

    /// Submit a control transfer
    pub fn submit_control_transfer(
        &mut self,
        device_address: u8,
        endpoint: u8,
        setup_packet: UsbRequest,
        data: Vec<u8>,
        direction: UsbDirection,
        ring: &mut TransferRing,
    ) -> Result<u32> {
        let transfer_id = self.allocate_transfer_id();

        let request = UsbTransferRequest {
            device_address,
            endpoint,
            direction,
            transfer_type: UsbTransferType::Control,
            data: data.clone(),
            setup_packet: Some(setup_packet),
            transfer_id,
        };

        // Build control transfer TRBs
        let trbs = self.build_control_transfer_trbs(&request)?;

        // Submit TRBs to ring
        ring.enqueue_trbs(&trbs)?;

        // Add to pending transfers
        let pending = PendingTransfer {
            request,
            status: UsbTransferStatus::InProgress,
            submitted_trbs: trbs,
            result_data: Vec::new(),
            actual_length: 0,
        };

        self.pending_transfers.push(pending);

        Ok(transfer_id)
    }

    /// Submit a bulk transfer
    pub fn submit_bulk_transfer(
        &mut self,
        device_address: u8,
        endpoint: u8,
        data: Vec<u8>,
        direction: UsbDirection,
        ring: &mut TransferRing,
    ) -> Result<u32> {
        let transfer_id = self.allocate_transfer_id();

        let request = UsbTransferRequest {
            device_address,
            endpoint,
            direction,
            transfer_type: UsbTransferType::Bulk,
            data: data.clone(),
            setup_packet: None,
            transfer_id,
        };

        // Build bulk transfer TRBs
        let trbs = self.build_bulk_transfer_trbs(&request)?;

        // Submit TRBs to ring
        ring.enqueue_trbs(&trbs)?;

        // Add to pending transfers
        let pending = PendingTransfer {
            request,
            status: UsbTransferStatus::InProgress,
            submitted_trbs: trbs,
            result_data: Vec::new(),
            actual_length: 0,
        };

        self.pending_transfers.push(pending);

        Ok(transfer_id)
    }

    /// Submit an interrupt transfer
    pub fn submit_interrupt_transfer(
        &mut self,
        device_address: u8,
        endpoint: u8,
        data: Vec<u8>,
        direction: UsbDirection,
        interval: u16,
        ring: &mut TransferRing,
    ) -> Result<u32> {
        let transfer_id = self.allocate_transfer_id();

        let request = UsbTransferRequest {
            device_address,
            endpoint,
            direction,
            transfer_type: UsbTransferType::Interrupt,
            data: data.clone(),
            setup_packet: None,
            transfer_id,
        };

        // Build interrupt transfer TRBs
        let trbs = self.build_interrupt_transfer_trbs(&request, interval)?;

        // Submit TRBs to ring
        ring.enqueue_trbs(&trbs)?;

        // Add to pending transfers
        let pending = PendingTransfer {
            request,
            status: UsbTransferStatus::InProgress,
            submitted_trbs: trbs,
            result_data: Vec::new(),
            actual_length: 0,
        };

        self.pending_transfers.push(pending);

        Ok(transfer_id)
    }

    /// Submit an isochronous transfer
    pub fn submit_isochronous_transfer(
        &mut self,
        device_address: u8,
        endpoint: u8,
        data: Vec<u8>,
        direction: UsbDirection,
        frame_number: u16,
        ring: &mut TransferRing,
    ) -> Result<u32> {
        let transfer_id = self.allocate_transfer_id();

        let request = UsbTransferRequest {
            device_address,
            endpoint,
            direction,
            transfer_type: UsbTransferType::Isochronous,
            data: data.clone(),
            setup_packet: None,
            transfer_id,
        };

        // Build isochronous transfer TRBs
        let trbs = self.build_isochronous_transfer_trbs(&request, frame_number)?;

        // Submit TRBs to ring
        ring.enqueue_trbs(&trbs)?;

        // Add to pending transfers
        let pending = PendingTransfer {
            request,
            status: UsbTransferStatus::InProgress,
            submitted_trbs: trbs,
            result_data: Vec::new(),
            actual_length: 0,
        };

        self.pending_transfers.push(pending);

        Ok(transfer_id)
    }

    /// Complete a transfer
    pub fn complete_transfer(&mut self, transfer_id: u32, result: UsbTransferResult) -> Result<()> {
        if let Some(transfer) = self.pending_transfers.iter_mut().find(|t| t.request.transfer_id == transfer_id) {
            transfer.status = result.status;
            transfer.result_data = result.data;
            transfer.actual_length = result.actual_length;

            // Completion callback removed for Send + Sync compatibility

            Ok(())
        } else {
            Err(UsbError::InvalidRequest)
        }
    }

    /// Cancel a transfer
    pub fn cancel_transfer(&mut self, transfer_id: u32) -> Result<()> {
        if let Some(transfer) = self.pending_transfers.iter_mut().find(|t| t.request.transfer_id == transfer_id) {
            transfer.status = UsbTransferStatus::Cancelled;
            Ok(())
        } else {
            Err(UsbError::InvalidRequest)
        }
    }

    /// Get transfer status
    pub fn get_transfer_status(&self, transfer_id: u32) -> Option<UsbTransferStatus> {
        self.pending_transfers
            .iter()
            .find(|t| t.request.transfer_id == transfer_id)
            .map(|t| t.status)
    }

    /// Get completed transfers
    pub fn get_completed_transfers(&mut self) -> Vec<UsbTransferResult> {
        let mut completed = Vec::new();

        self.pending_transfers.retain(|transfer| {
            match transfer.status {
                UsbTransferStatus::Completed | UsbTransferStatus::Failed | UsbTransferStatus::Cancelled => {
                    let result = UsbTransferResult {
                        transfer_id: transfer.request.transfer_id,
                        status: transfer.status,
                        data: transfer.result_data.clone(),
                        actual_length: transfer.actual_length,
                        error: match transfer.status {
                            UsbTransferStatus::Failed => Some(UsbError::TransferFailed),
                            UsbTransferStatus::Cancelled => Some(UsbError::TransferCancelled),
                            _ => None,
                        },
                    };
                    completed.push(result);
                    false // Remove from pending
                }
                _ => true, // Keep in pending
            }
        });

        completed
    }

    /// Build control transfer TRBs
    fn build_control_transfer_trbs(&self, request: &UsbTransferRequest) -> Result<Vec<Trb>> {
        let mut trbs = Vec::new();

        let setup_packet = request.setup_packet.as_ref().ok_or(UsbError::InvalidRequest)?;

        // Setup Stage TRB
        let setup_trb = SetupStageTrb::new(
            setup_packet,
            request.data.len() as u32,
            true, // cycle bit will be set later
        );
        trbs.push(setup_trb);

        // Data Stage TRB (if there's data)
        if !request.data.is_empty() {
            let data_buffer_addr = request.data.as_ptr() as u64;
            let data_trb = DataStageTrb::new(
                data_buffer_addr,
                request.data.len() as u32,
                request.direction,
                true, // cycle bit will be set later
            );
            trbs.push(data_trb);
        }

        // Status Stage TRB
        let status_direction = if request.data.is_empty() {
            UsbDirection::In
        } else {
            match request.direction {
                UsbDirection::In => UsbDirection::Out,
                UsbDirection::Out => UsbDirection::In,
            }
        };

        let status_trb = StatusStageTrb::new(status_direction, true);
        trbs.push(status_trb);

        Ok(trbs)
    }

    /// Build bulk transfer TRBs
    fn build_bulk_transfer_trbs(&self, request: &UsbTransferRequest) -> Result<Vec<Trb>> {
        let mut trbs = Vec::new();

        if request.data.is_empty() {
            return Ok(trbs);
        }

        // Split large transfers into multiple TRBs
        let mut remaining_data = request.data.len();
        let mut data_offset = 0;

        while remaining_data > 0 {
            let chunk_size = remaining_data.min(self.max_transfer_size);
            let data_buffer_addr = unsafe {
                request.data.as_ptr().add(data_offset) as u64
            };

            let normal_trb = NormalTrb::new(
                data_buffer_addr,
                chunk_size as u32,
                true, // cycle bit will be set later
                remaining_data == chunk_size, // Interrupt on completion for last TRB
            );

            trbs.push(normal_trb);

            remaining_data -= chunk_size;
            data_offset += chunk_size;
        }

        Ok(trbs)
    }

    /// Build interrupt transfer TRBs
    fn build_interrupt_transfer_trbs(&self, request: &UsbTransferRequest, _interval: u16) -> Result<Vec<Trb>> {
        // Interrupt transfers are similar to bulk transfers but with different scheduling
        let mut trbs = Vec::new();

        if !request.data.is_empty() {
            let data_buffer_addr = request.data.as_ptr() as u64;
            let normal_trb = NormalTrb::new(
                data_buffer_addr,
                request.data.len() as u32,
                true, // cycle bit will be set later
                true, // Always interrupt on completion for interrupt transfers
            );
            trbs.push(normal_trb);
        }

        Ok(trbs)
    }

    /// Build isochronous transfer TRBs
    fn build_isochronous_transfer_trbs(&self, request: &UsbTransferRequest, _frame_number: u16) -> Result<Vec<Trb>> {
        // Isochronous transfers require special TRB types and frame scheduling
        let mut trbs = Vec::new();

        if !request.data.is_empty() {
            let data_buffer_addr = request.data.as_ptr() as u64;

            // For now, use normal TRB - in a real implementation, we'd use Isoch TRB
            let isoch_trb = NormalTrb::new(
                data_buffer_addr,
                request.data.len() as u32,
                true, // cycle bit will be set later
                true, // Interrupt on completion
            );
            trbs.push(isoch_trb);
        }

        Ok(trbs)
    }

    /// Allocate a new transfer ID
    fn allocate_transfer_id(&mut self) -> u32 {
        let id = self.next_transfer_id;
        self.next_transfer_id += 1;
        id
    }

    /// Get pending transfer count
    pub fn pending_count(&self) -> usize {
        self.pending_transfers.len()
    }

    /// Get statistics
    pub fn get_stats(&self) -> UsbTransferStats {
        let mut stats = UsbTransferStats::default();

        for transfer in &self.pending_transfers {
            match transfer.request.transfer_type {
                UsbTransferType::Control => stats.control_transfers += 1,
                UsbTransferType::Bulk => stats.bulk_transfers += 1,
                UsbTransferType::Interrupt => stats.interrupt_transfers += 1,
                UsbTransferType::Isochronous => stats.isochronous_transfers += 1,
            }

            match transfer.status {
                UsbTransferStatus::Pending => stats.pending_transfers += 1,
                UsbTransferStatus::InProgress => stats.in_progress_transfers += 1,
                UsbTransferStatus::Completed => stats.completed_transfers += 1,
                UsbTransferStatus::Failed => stats.failed_transfers += 1,
                UsbTransferStatus::Cancelled => stats.cancelled_transfers += 1,
            }
        }

        stats
    }
}

impl Default for UsbTransferManager {
    fn default() -> Self {
        Self::new()
    }
}

/// USB Transfer Statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct UsbTransferStats {
    pub control_transfers: usize,
    pub bulk_transfers: usize,
    pub interrupt_transfers: usize,
    pub isochronous_transfers: usize,
    pub pending_transfers: usize,
    pub in_progress_transfers: usize,
    pub completed_transfers: usize,
    pub failed_transfers: usize,
    pub cancelled_transfers: usize,
}

/// Control Transfer Helper
pub struct ControlTransfer;

impl ControlTransfer {
    /// Standard GET_DESCRIPTOR request
    pub fn get_descriptor(
        device_address: u8,
        descriptor_type: u8,
        descriptor_index: u8,
        language_id: u16,
        length: u16,
    ) -> UsbTransferRequest {
        let setup_packet = UsbRequest {
            request_type: 0x80, // Device to host, standard, device
            request: 0x06,      // GET_DESCRIPTOR
            value: ((descriptor_type as u16) << 8) | (descriptor_index as u16),
            index: language_id,
            length,
        };

        UsbTransferRequest {
            device_address,
            endpoint: 0, // Control endpoint
            direction: UsbDirection::In,
            transfer_type: UsbTransferType::Control,
            data: vec![0; length as usize],
            setup_packet: Some(setup_packet),
            transfer_id: 0, // Will be set by transfer manager
        }
    }

    /// Standard SET_ADDRESS request
    pub fn set_address(device_address: u8, new_address: u8) -> UsbTransferRequest {
        let setup_packet = UsbRequest {
            request_type: 0x00, // Host to device, standard, device
            request: 0x05,      // SET_ADDRESS
            value: new_address as u16,
            index: 0,
            length: 0,
        };

        UsbTransferRequest {
            device_address,
            endpoint: 0, // Control endpoint
            direction: UsbDirection::Out,
            transfer_type: UsbTransferType::Control,
            data: Vec::new(),
            setup_packet: Some(setup_packet),
            transfer_id: 0, // Will be set by transfer manager
        }
    }

    /// Standard SET_CONFIGURATION request
    pub fn set_configuration(device_address: u8, configuration_value: u8) -> UsbTransferRequest {
        let setup_packet = UsbRequest {
            request_type: 0x00, // Host to device, standard, device
            request: 0x09,      // SET_CONFIGURATION
            value: configuration_value as u16,
            index: 0,
            length: 0,
        };

        UsbTransferRequest {
            device_address,
            endpoint: 0, // Control endpoint
            direction: UsbDirection::Out,
            transfer_type: UsbTransferType::Control,
            data: Vec::new(),
            setup_packet: Some(setup_packet),
            transfer_id: 0, // Will be set by transfer manager
        }
    }
}