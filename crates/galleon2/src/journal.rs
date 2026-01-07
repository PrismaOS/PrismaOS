//! Transaction Journal System
//!
//! Provides crash recovery and transaction support with journaling.
//! All filesystem modifications are logged before being committed.

use core::ops::{Add, AddAssign};

use alloc::{borrow::ToOwned, collections::VecDeque, vec::{self, Vec}};
use lib_kernel::kprint;
use crate::{FilesystemResult, FilesystemError, ide_read_sectors, ide_write_sectors};

/// Transaction operation types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationType {
    CreateFile = 1,
    DeleteFile = 2,
    WriteData = 3,
    UpdateMetadata = 4,
    CreateDirectory = 5,
    DeleteDirectory = 6,
    MoveFile = 7,
    SetAttribute = 8,
}

/// Log record entry
#[derive(Debug, Clone)]
pub struct LogRecord {
    pub sequence_number: u64,
    pub operation_type: OperationType,
    pub target_file_record: u64,
    pub undo_data: Vec<u8>,
    pub redo_data: Vec<u8>,
    pub checksum: u32,
    pub timestamp: u64, // Simple timestamp counter
}

impl LogRecord {
    pub fn new(
        sequence_number: u64,
        operation_type: OperationType,
        target_file_record: u64,
        undo_data: Vec<u8>,
        redo_data: Vec<u8>,
    ) -> Self {
        let mut record = Self {
            sequence_number,
            operation_type,
            target_file_record,
            undo_data,
            redo_data,
            checksum: 0,
            timestamp: get_timestamp(),
        };
        record.checksum = record.calculate_checksum();
        record
    }

    fn calculate_checksum(&self) -> u32 {
        let mut checksum = 0u32;
        checksum ^= self.sequence_number as u32;
        checksum ^= (self.sequence_number >> 32) as u32;
        checksum ^= self.operation_type as u32;
        checksum ^= self.target_file_record as u32;
        checksum ^= (self.target_file_record >> 32) as u32;

        for &byte in &self.undo_data {
            checksum ^= byte as u32;
        }
        for &byte in &self.redo_data {
            checksum ^= byte as u32;
        }

        checksum
    }

    pub fn verify_checksum(&self) -> bool {
        self.checksum == self.calculate_checksum()
    }

    pub fn serialize(&self, data: &mut Vec<u8>) -> Vec<u8> {

        // Header
        data.extend_from_slice(b"JRNL"); // Signature
        data.extend_from_slice(&self.sequence_number.to_le_bytes());
        data.extend_from_slice(&(self.operation_type as u32).to_le_bytes());
        data.extend_from_slice(&self.target_file_record.to_le_bytes());
        data.extend_from_slice(&(self.undo_data.len() as u32).to_le_bytes());
        data.extend_from_slice(&(self.redo_data.len() as u32).to_le_bytes());
        data.extend_from_slice(&self.checksum.to_le_bytes());
        data.extend_from_slice(&self.timestamp.to_le_bytes());

        // Data
        data.extend_from_slice(&self.undo_data);
        data.extend_from_slice(&self.redo_data);

        // Pad to 8-byte boundary
        while data.len() % 8 != 0 {
            data.push(0);
        }

        data.to_owned()
    }

    pub fn deserialize(data: &[u8]) -> FilesystemResult<Self> {
        if data.len() < 48 {
            return Err(FilesystemError::InvalidParameter);
        }

        if &data[0..4] != b"JRNL" {
            return Err(FilesystemError::InvalidParameter);
        }

        let sequence_number = u64::from_le_bytes(data[4..12].try_into().unwrap());
        let operation_type = match u32::from_le_bytes(data[12..16].try_into().unwrap()) {
            1 => OperationType::CreateFile,
            2 => OperationType::DeleteFile,
            3 => OperationType::WriteData,
            4 => OperationType::UpdateMetadata,
            5 => OperationType::CreateDirectory,
            6 => OperationType::DeleteDirectory,
            7 => OperationType::MoveFile,
            8 => OperationType::SetAttribute,
            _ => return Err(FilesystemError::InvalidParameter),
        };

        let target_file_record = u64::from_le_bytes(data[16..24].try_into().unwrap());
        let undo_data_len = u32::from_le_bytes(data[24..28].try_into().unwrap()) as usize;
        let redo_data_len = u32::from_le_bytes(data[28..32].try_into().unwrap()) as usize;
        let checksum = u32::from_le_bytes(data[32..36].try_into().unwrap());
        let timestamp = u64::from_le_bytes(data[36..44].try_into().unwrap());

        if data.len() < 44 + undo_data_len + redo_data_len {
            return Err(FilesystemError::InvalidParameter);
        }

        let undo_data = data[44..44 + undo_data_len].to_vec();
        let redo_data = data[44 + undo_data_len..44 + undo_data_len + redo_data_len].to_vec();

        let record = Self {
            sequence_number,
            operation_type,
            target_file_record,
            undo_data,
            redo_data,
            checksum,
            timestamp,
        };

        if !record.verify_checksum() {
            return Err(FilesystemError::InvalidParameter);
        }

        Ok(record)
    }
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionState {
    Active,
    Committed,
    Aborted,
}

/// Active transaction
#[derive(Debug)]
pub struct Transaction {
    pub id: u64,
    pub state: TransactionState,
    pub records: Vec<LogRecord>,
    pub start_sequence: u64,
}

impl Transaction {
    pub fn new(id: u64, start_sequence: u64) -> Self {
        Self {
            id,
            state: TransactionState::Active,
            records: Vec::new(),
            start_sequence,
        }
    }

    pub fn add_record(&mut self, record: LogRecord) {
        if self.state == TransactionState::Active {
            self.records.push(record);
        }
    }

    pub fn commit(&mut self) {
        self.state = TransactionState::Committed;
    }

    pub fn abort(&mut self) {
        self.state = TransactionState::Aborted;
    }
}

/// Journal Manager
pub struct JournalManager {
    pub drive: u8,
    pub journal_start_sector: u64,
    pub journal_size_sectors: u64,
    pub current_sequence: u64,
    pub active_transactions: VecDeque<Transaction>,
    pub next_transaction_id: u64,
}

impl JournalManager {
    pub fn new(drive: u8, journal_start_sector: u64, journal_size_sectors: u64) -> Self {
        Self {
            drive,
            journal_start_sector,
            journal_size_sectors,
            current_sequence: 1,
            active_transactions: VecDeque::new(),
            next_transaction_id: 1,
        }
    }

    pub fn begin_transaction(&mut self) -> u64 {
        let transaction_id = self.next_transaction_id;
        self.next_transaction_id += 1;

        let transaction = Transaction::new(transaction_id, self.current_sequence);
        self.active_transactions.push_back(transaction);

        transaction_id
    }

    pub fn log_operation(
        &mut self,
        transaction_id: u64,
        operation_type: OperationType,
        target_file_record: u64,
        undo_data: Vec<u8>,
        redo_data: Vec<u8>,
    ) -> FilesystemResult<()> {
        lib_kernel::kprintln!("[JOURNAL] Logging operation {:?} for transaction {}\n", operation_type, transaction_id);
        let record = LogRecord::new(
            self.current_sequence,
            operation_type,
            target_file_record,
            undo_data,
            redo_data,
        );
        lib_kernel::kprintln!("[JOURNAL] Created log record: {:?}", record);

        // Find the transaction
        if let Some(transaction) = self.active_transactions.iter_mut()
            .find(|t| t.id == transaction_id && t.state == TransactionState::Active) {
            transaction.add_record(record.clone());
        } else {
            return Err(FilesystemError::InvalidParameter);
        }

        lib_kernel::kprintln!("[JOURNAL] Writing log record to journal on disk...");

        // Write the record to journal
        self.write_log_record(&record)?;
        lib_kernel::kprintln!("[JOURNAL] Log record written successfully.");
        self.current_sequence += 1;



        Ok(())
    }

    pub fn commit_transaction(&mut self, transaction_id: u64) -> FilesystemResult<()> {
        if let Some(transaction) = self.active_transactions.iter_mut()
            .find(|t| t.id == transaction_id) {
            transaction.commit();

            // Write commit record
            let commit_record = LogRecord::new(
                self.current_sequence,
                OperationType::UpdateMetadata, // Use as commit marker
                transaction_id,
                Vec::new(),
                b"COMMIT".to_vec(),
            );

            self.write_log_record(&commit_record)?;
            self.current_sequence += 1;

            Ok(())
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }

    pub fn abort_transaction(&mut self, transaction_id: u64) -> FilesystemResult<()> {
        // Find the transaction and get its records
        let records = if let Some(transaction) = self.active_transactions.iter()
            .find(|t| t.id == transaction_id) {
            transaction.records.clone()
        } else {
            return Err(FilesystemError::InvalidParameter);
        };

        // Apply undo operations in reverse order
        for record in records.iter().rev() {
            self.apply_undo(record)?;
        }

        // Now abort the transaction
        if let Some(transaction) = self.active_transactions.iter_mut()
            .find(|t| t.id == transaction_id) {
            transaction.abort();

            // Write abort record
            let abort_record = LogRecord::new(
                self.current_sequence,
                OperationType::UpdateMetadata, // Use as abort marker
                transaction_id,
                Vec::new(),
                b"ABORT".to_vec(),
            );

            self.write_log_record(&abort_record)?;
            self.current_sequence += 1;

            Ok(())
        } else {
            Err(FilesystemError::InvalidParameter)
        }
    }

    fn write_log_record(&self, record: &LogRecord) -> FilesystemResult<()> {
        lib_kernel::kprintln!("[JOURNAL] Preparing to write log record: {:?}", record);
        let mut data = Vec::new();
        let serialized = record.serialize(&mut data);
        lib_kernel::kprintln!("[JOURNAL] Serialized log record ({} bytes)", serialized.len());
        if serialized.is_empty() {
            return Err(FilesystemError::InvalidParameter);
        }

        let sectors_needed = serialized.len().add(511).div_ceil(512);

        // Calculate journal position (circular buffer)
        // Ensure we don't divide by zero and have a reasonable record size
        let record_size = serialized.len().max(64) as u64; // Minimum 64 bytes
        let max_records_in_journal = (self.journal_size_sectors * 512) / record_size;
        let journal_offset = if max_records_in_journal > 0 {
            (record.sequence_number % max_records_in_journal) * record_size / 512
        } else {
            0 // Fallback if journal is too small
        };
        let write_sector = self.journal_start_sector + journal_offset;

        let thing = align_of_val(&sectors_needed);

        lib_kernel::kprintln!("Aligned size: {}", thing);

        // Prepare sector-aligned data
        let mut sector_data = alloc::vec![0u8; sectors_needed * 512];

        let other_thing = align_of_val(&sector_data);
        lib_kernel::kprintln!("Aligned sector data size: {}", other_thing);

        sector_data[..serialized.len()].copy_from_slice(&serialized);

        ide_write_sectors(self.drive, sectors_needed as u8, write_sector as u32, &sector_data)?;
        Ok(())
    }

    fn apply_undo(&self, _record: &LogRecord) -> FilesystemResult<()> {
        // Apply the undo data to restore previous state
        // This would interact with the MFT manager to restore file records
        // For now, this is a stub implementation
        Ok(())
    }

    pub fn recover(&mut self) -> FilesystemResult<()> {
        // Scan the journal for uncommitted transactions and apply undo operations
        // This would be called during filesystem mount after a crash

        let mut current_sector = 0u64;
        while current_sector < self.journal_size_sectors {
            let sector_data = self.read_journal_sector(current_sector)?;

            // Parse log records from the sector
            let mut offset = 0;
            while offset + 48 <= sector_data.len() {
                if &sector_data[offset..offset+4] == b"JRNL" {
                    if let Ok(record) = LogRecord::deserialize(&sector_data[offset..]) {
                        // Check if this is part of a committed transaction
                        // If not, apply undo operation
                        if !self.is_transaction_committed(record.target_file_record) {
                            self.apply_undo(&record)?;
                        }
                        offset += record.serialize(&mut Vec::new()).len();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            current_sector += 1;
        }

        Ok(())
    }

    fn read_journal_sector(&self, sector_offset: u64) -> FilesystemResult<Vec<u8>> {
        let mut sector_data = alloc::vec![0u8; 512];
        ide_read_sectors(
            self.drive,
            1,
            (self.journal_start_sector + sector_offset) as u32,
            &mut sector_data,
        )?;
        Ok(sector_data)
    }

    fn is_transaction_committed(&self, _transaction_id: u64) -> bool {
        // Check if transaction was committed by scanning for commit record
        // Simplified implementation
        true
    }

    pub fn cleanup_completed_transactions(&mut self) {
        // Remove committed/aborted transactions that are no longer needed
        self.active_transactions.retain(|t| t.state == TransactionState::Active);
    }
}

// Simple timestamp function
fn get_timestamp() -> u64 {
    static mut COUNTER: u64 = 0;
    unsafe {
        COUNTER += 1;
        COUNTER
    }
}