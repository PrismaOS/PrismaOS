//! File Record System
//!
//! High-level file operations built on top of the MFT system.
//! Provides advanced file management with attributes and metadata.

use alloc::{vec, string::String, vec::Vec};
use crate::{
    FilesystemResult, FilesystemError,
    mft::{MftRecord, MftManager, Attribute, AttributeType, AttributeData, FileRecordNumber, RecordFlags},
    journal::{JournalManager, OperationType},
};

/// File timestamps
#[derive(Debug, Clone, Copy)]
pub struct FileTimes {
    pub creation_time: u64,
    pub last_access_time: u64,
    pub last_write_time: u64,
    pub mft_change_time: u64,
}

impl FileTimes {
    pub fn new() -> Self {
        let now = get_current_time();
        Self {
            creation_time: now,
            last_access_time: now,
            last_write_time: now,
            mft_change_time: now,
        }
    }

    pub fn update_access(&mut self) {
        self.last_access_time = get_current_time();
    }

    pub fn update_write(&mut self) {
        let now = get_current_time();
        self.last_write_time = now;
        self.mft_change_time = now;
    }

    pub fn update_metadata(&mut self) {
        self.mft_change_time = get_current_time();
    }
}

/// File attributes
#[derive(Debug, Clone, Copy)]
pub struct FileAttributes {
    pub readonly: bool,
    pub hidden: bool,
    pub system: bool,
    pub directory: bool,
    pub archive: bool,
    pub normal: bool,
    pub temporary: bool,
    pub sparse_file: bool,
    pub reparse_point: bool,
    pub compressed: bool,
    pub encrypted: bool,
}

impl FileAttributes {
    pub fn new_file() -> Self {
        Self {
            readonly:      false,
            hidden:        false,
            system:        false,
            directory:     false,
            archive:       true,
            normal:        true,
            temporary:     false,
            sparse_file:   false,
            reparse_point: false,
            compressed:    false,
            encrypted:     false,
        }
    }

    pub fn new_directory() -> Self {
        Self {
            readonly:      false,
            hidden:        false,
            system:        false,
            directory:     true,
            archive:       false,
            normal:        false,
            temporary:     false,
            sparse_file:   false,
            reparse_point: false,
            compressed:    false,
            encrypted:     false,
        }
    }

    pub fn to_u32(&self) -> u32 {
        let mut attrs = 0u32;
        if self.readonly      { attrs |= 0x00000001; }
        if self.hidden        { attrs |= 0x00000002; }
        if self.system        { attrs |= 0x00000004; }
        if self.directory     { attrs |= 0x00000010; }
        if self.archive       { attrs |= 0x00000020; }
        if self.normal        { attrs |= 0x00000080; }
        if self.temporary     { attrs |= 0x00000100; }
        if self.sparse_file   { attrs |= 0x00000200; }
        if self.reparse_point { attrs |= 0x00000400; }
        if self.compressed    { attrs |= 0x00000800; }
        if self.encrypted     { attrs |= 0x00004000; }
        attrs
    }

    pub fn from_u32(value: u32) -> Self {
        Self {
            readonly:      (value & 0x00000001) != 0,
            hidden:        (value & 0x00000002) != 0,
            system:        (value & 0x00000004) != 0,
            directory:     (value & 0x00000010) != 0,
            archive:       (value & 0x00000020) != 0,
            normal:        (value & 0x00000080) != 0,
            temporary:     (value & 0x00000100) != 0,
            sparse_file:   (value & 0x00000200) != 0,
            reparse_point: (value & 0x00000400) != 0,
            compressed:    (value & 0x00000800) != 0,
            encrypted:     (value & 0x00004000) != 0,
        }
    }
}

/// Standard Information attribute data
#[derive(Debug, Clone)]
pub struct StandardInformation {
    pub times: FileTimes,
    pub file_attributes: FileAttributes,
    pub maximum_versions: u32,
    pub version_number: u32,
    pub class_id: u32,
    pub owner_id: u32,
    pub security_id: u32,
    pub quota_charged: u64,
    pub update_sequence_number: u64,
}

impl StandardInformation {
    pub fn new_file() -> Self {
        Self {
            times: FileTimes::new(),
            file_attributes: FileAttributes::new_file(),
            maximum_versions: 0,
            version_number: 0,
            class_id: 0,
            owner_id: 0,
            security_id: 0,
            quota_charged: 0,
            update_sequence_number: 0,
        }
    }

    pub fn new_directory() -> Self {
        Self {
            times: FileTimes::new(),
            file_attributes: FileAttributes::new_directory(),
            maximum_versions: 0,
            version_number: 0,
            class_id: 0,
            owner_id: 0,
            security_id: 0,
            quota_charged: 0,
            update_sequence_number: 0,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.times.creation_time.to_le_bytes());
        data.extend_from_slice(&self.times.last_access_time.to_le_bytes());
        data.extend_from_slice(&self.times.last_write_time.to_le_bytes());
        data.extend_from_slice(&self.times.mft_change_time.to_le_bytes());
        data.extend_from_slice(&self.file_attributes.to_u32().to_le_bytes());
        data.extend_from_slice(&self.maximum_versions.to_le_bytes());
        data.extend_from_slice(&self.version_number.to_le_bytes());
        data.extend_from_slice(&self.class_id.to_le_bytes());
        data.extend_from_slice(&self.owner_id.to_le_bytes());
        data.extend_from_slice(&self.security_id.to_le_bytes());
        data.extend_from_slice(&self.quota_charged.to_le_bytes());
        data.extend_from_slice(&self.update_sequence_number.to_le_bytes());
        data
    }

    pub fn deserialize(data: &[u8]) -> FilesystemResult<Self> {
        if data.len() < 72 {
            return Err(FilesystemError::InvalidParameter);
        }

        let times = FileTimes {
            creation_time: u64::from_le_bytes(data[0..8].try_into().unwrap()),
            last_access_time: u64::from_le_bytes(data[8..16].try_into().unwrap()),
            last_write_time: u64::from_le_bytes(data[16..24].try_into().unwrap()),
            mft_change_time: u64::from_le_bytes(data[24..32].try_into().unwrap()),
        };

        let file_attributes = FileAttributes::from_u32(u32::from_le_bytes(data[32..36].try_into().unwrap()));
        let maximum_versions = u32::from_le_bytes(data[36..40].try_into().unwrap());
        let version_number = u32::from_le_bytes(data[40..44].try_into().unwrap());
        let class_id = u32::from_le_bytes(data[44..48].try_into().unwrap());
        let owner_id = u32::from_le_bytes(data[48..52].try_into().unwrap());
        let security_id = u32::from_le_bytes(data[52..56].try_into().unwrap());
        let quota_charged = u64::from_le_bytes(data[56..64].try_into().unwrap());
        let update_sequence_number = u64::from_le_bytes(data[64..72].try_into().unwrap());

        Ok(Self {
            times,
            file_attributes,
            maximum_versions,
            version_number,
            class_id,
            owner_id,
            security_id,
            quota_charged,
            update_sequence_number,
        })
    }
}

/// File name attribute
#[derive(Debug, Clone)]
pub struct FileName {
    pub parent_directory: FileRecordNumber,
    pub name: String,
    pub namespace: u8, // 0=POSIX, 1=Win32, 2=DOS, 3=Win32&DOS
    pub file_size: u64,
    pub allocated_size: u64,
    pub times: FileTimes,
    pub file_attributes: FileAttributes,
}

impl FileName {
    pub fn new(parent: FileRecordNumber, name: String, is_directory: bool) -> Self {
        let file_attributes = if is_directory {
            FileAttributes::new_directory()
        } else {
            FileAttributes::new_file()
        };

        Self {
            parent_directory: parent,
            name,
            namespace: 1, // Win32
            file_size: 0,
            allocated_size: 0,
            times: FileTimes::new(),
            file_attributes,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.parent_directory.to_le_bytes());
        data.extend_from_slice(&self.times.creation_time.to_le_bytes());
        data.extend_from_slice(&self.times.last_access_time.to_le_bytes());
        data.extend_from_slice(&self.times.last_write_time.to_le_bytes());
        data.extend_from_slice(&self.times.mft_change_time.to_le_bytes());
        data.extend_from_slice(&self.allocated_size.to_le_bytes());
        data.extend_from_slice(&self.file_size.to_le_bytes());
        data.extend_from_slice(&self.file_attributes.to_u32().to_le_bytes());
        data.push(self.name.len() as u8);
        data.push(self.namespace);

        // Convert to UTF-16LE (simplified - just expand ASCII to 16-bit)
        for byte in self.name.bytes() {
            data.push(byte);
            data.push(0);
        }

        // Pad to 8-byte boundary
        while data.len() % 8 != 0 {
            data.push(0);
        }

        data
    }

    pub fn deserialize(data: &[u8]) -> FilesystemResult<Self> {
        if data.len() < 66 {
            return Err(FilesystemError::InvalidParameter);
        }

        let parent_directory = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let times = FileTimes {
            creation_time: u64::from_le_bytes(data[8..16].try_into().unwrap()),
            last_access_time: u64::from_le_bytes(data[16..24].try_into().unwrap()),
            last_write_time: u64::from_le_bytes(data[24..32].try_into().unwrap()),
            mft_change_time: u64::from_le_bytes(data[32..40].try_into().unwrap()),
        };
        let allocated_size = u64::from_le_bytes(data[40..48].try_into().unwrap());
        let file_size = u64::from_le_bytes(data[48..56].try_into().unwrap());
        let file_attributes = FileAttributes::from_u32(u32::from_le_bytes(data[56..60].try_into().unwrap()));
        let name_length = data[60] as usize;
        let namespace = data[61];

        // Read UTF-16LE name (simplified - assume ASCII)
        let mut name = String::new();
        for i in 0..name_length {
            if 62 + i * 2 < data.len() {
                name.push(data[62 + i * 2] as char);
            }
        }

        Ok(Self {
            parent_directory,
            name,
            namespace,
            file_size,
            allocated_size,
            times,
            file_attributes,
        })
    }
}

/// File Record Manager - high-level file operations
pub struct FileRecordManager {
    mft_manager: MftManager,
    journal_manager: JournalManager,
}

impl FileRecordManager {
    pub fn new(mft_manager: MftManager, journal_manager: JournalManager) -> Self {
        Self {
            mft_manager,
            journal_manager,
        }
    }

    pub fn create_file(
        &mut self,
        parent_directory: FileRecordNumber,
        name: String,
        data: Option<Vec<u8>>,
    ) -> FilesystemResult<FileRecordNumber> {
        let transaction_id = self.journal_manager.begin_transaction();

        // Allocate new file record
        let file_record_number = self.mft_manager.allocate_record()?;

        // Create MFT record
        let mut record = MftRecord::new(file_record_number);
        let mut flags = RecordFlags::new();
        flags.in_use = true;
        record.header.set_flags(flags);

        // Add Standard Information attribute
        let std_info = StandardInformation::new_file();
        let std_info_attr = Attribute::new_resident(AttributeType::StandardInformation, std_info.serialize());
        record.add_attribute(std_info_attr);

        // Add File Name attribute
        let file_name = FileName::new(parent_directory, name.clone(), false);
        let file_name_attr = Attribute::new_resident(AttributeType::FileName, file_name.serialize());
        record.add_attribute(file_name_attr);

        // Add Data attribute
        if let Some(file_data) = data {
            let data_attr = if file_data.len() <= 700 { // Resident threshold
                Attribute::new_resident(AttributeType::Data, file_data.clone())
            } else {
                // Create non-resident data (would need cluster allocation)
                let runs = vec![crate::allocation::ClusterRun::new(0, 1)]; // Simplified
                Attribute::new_non_resident(AttributeType::Data, runs, file_data.len() as u64)
            };
            record.add_attribute(data_attr);

            // Log the operation
            self.journal_manager.log_operation(
                transaction_id,
                OperationType::CreateFile,
                file_record_number,
                Vec::new(), // No undo data for create
                record.serialize(),
            )?;
        }

        // Write the record
        self.mft_manager.write_record(&record)?;

        // Commit the transaction
        self.journal_manager.commit_transaction(transaction_id)?;

        Ok(file_record_number)
    }

    pub fn create_directory(
        &mut self,
        parent_directory: FileRecordNumber,
        name: String,
    ) -> FilesystemResult<FileRecordNumber> {
        let transaction_id = self.journal_manager.begin_transaction();

        // Allocate new directory record
        let dir_record_number = self.mft_manager.allocate_record()?;

        // Create MFT record for directory
        let mut record = MftRecord::new(dir_record_number);
        let mut flags = RecordFlags::new();
        flags.in_use = true;
        flags.is_directory = true;
        record.header.set_flags(flags);

        // Add Standard Information attribute
        let std_info = StandardInformation::new_directory();
        let std_info_attr = Attribute::new_resident(AttributeType::StandardInformation, std_info.serialize());
        record.add_attribute(std_info_attr);

        // Add File Name attribute
        let file_name = FileName::new(parent_directory, name.clone(), true);
        let file_name_attr = Attribute::new_resident(AttributeType::FileName, file_name.serialize());
        record.add_attribute(file_name_attr);

        // Add Index Root attribute for directory entries
        let index_root_data = create_empty_index_root();
        let index_root_attr = Attribute::new_resident(AttributeType::IndexRoot, index_root_data);
        record.add_attribute(index_root_attr);

        // Log the operation
        self.journal_manager.log_operation(
            transaction_id,
            OperationType::CreateDirectory,
            dir_record_number,
            Vec::new(),
            record.serialize(),
        )?;

        // Write the record
        self.mft_manager.write_record(&record)?;

        // Commit the transaction
        self.journal_manager.commit_transaction(transaction_id)?;

        Ok(dir_record_number)
    }

    pub fn read_file(&mut self, file_record_number: FileRecordNumber) -> FilesystemResult<Vec<u8>> {
        let record = self.mft_manager.read_record(file_record_number)?;

        // Find the data attribute
        for attr in &record.attributes {
            if attr.header.attr_type == AttributeType::Data as u32 {
                match &attr.data {
                    AttributeData::Resident(data) => {
                        // Update access time
                        self.update_access_time(file_record_number)?;
                        return Ok(data.clone());
                    }
                    AttributeData::NonResident { runs, real_size, .. } => {
                        // Read data from clusters (simplified)
                        let mut file_data = vec![0u8; *real_size as usize];
                        // Would read from clusters specified in runs
                        self.update_access_time(file_record_number)?;
                        return Ok(file_data);
                    }
                }
            }
        }

        Err(FilesystemError::InvalidParameter)
    }

    pub fn write_file(&mut self, file_record_number: FileRecordNumber, data: Vec<u8>) -> FilesystemResult<()> {
        let transaction_id = self.journal_manager.begin_transaction();

        let mut record = self.mft_manager.read_record(file_record_number)?;
        let old_record_data = record.serialize();

        // Find and update the data attribute
        for attr in &mut record.attributes {
            if attr.header.attr_type == AttributeType::Data as u32 {
                attr.data = if data.len() <= 700 {
                    AttributeData::Resident(data.clone())
                } else {
                    // Convert to non-resident or update existing runs
                    let runs = vec![crate::allocation::ClusterRun::new(0, 1)]; // Simplified
                    AttributeData::NonResident {
                        runs,
                        allocated_size: (data.len() + 4095) as u64 & !4095, // Round up to cluster
                        real_size: data.len() as u64,
                        initialized_size: data.len() as u64,
                    }
                };
                break;
            }
        }

        // Update file times
        self.update_write_time(&mut record)?;

        // Log the operation
        self.journal_manager.log_operation(
            transaction_id,
            OperationType::WriteData,
            file_record_number,
            old_record_data,
            record.serialize(),
        )?;

        // Write the updated record
        self.mft_manager.write_record(&record)?;

        // Commit the transaction
        self.journal_manager.commit_transaction(transaction_id)?;

        Ok(())
    }

    fn update_access_time(&mut self, file_record_number: FileRecordNumber) -> FilesystemResult<()> {
        let mut record = self.mft_manager.read_record(file_record_number)?;

        // Update access time in Standard Information
        for attr in &mut record.attributes {
            if attr.header.attr_type == AttributeType::StandardInformation as u32 {
                if let AttributeData::Resident(ref mut data) = attr.data {
                    let mut std_info = StandardInformation::deserialize(data)?;
                    std_info.times.update_access();
                    *data = std_info.serialize();
                }
                break;
            }
        }

        self.mft_manager.write_record(&record)?;
        Ok(())
    }

    fn update_write_time(&self, record: &mut MftRecord) -> FilesystemResult<()> {
        // Update write time in Standard Information
        for attr in &mut record.attributes {
            if attr.header.attr_type == AttributeType::StandardInformation as u32 {
                if let AttributeData::Resident(ref mut data) = attr.data {
                    let mut std_info = StandardInformation::deserialize(data)?;
                    std_info.times.update_write();
                    *data = std_info.serialize();
                }
                break;
            }
        }
        Ok(())
    }

    pub fn delete_file(&mut self, file_record_number: FileRecordNumber) -> FilesystemResult<()> {
        let transaction_id = self.journal_manager.begin_transaction();

        let record = self.mft_manager.read_record(file_record_number)?;
        let old_record_data = record.serialize();

        // Mark record as not in use
        let mut new_record = record.clone();
        let mut flags = new_record.header.get_flags();
        flags.in_use = false;
        new_record.header.set_flags(flags);

        // Log the operation
        self.journal_manager.log_operation(
            transaction_id,
            OperationType::DeleteFile,
            file_record_number,
            old_record_data, // For recovery
            new_record.serialize(),
        )?;

        // Write the updated record
        self.mft_manager.write_record(&new_record)?;

        // Commit the transaction
        self.journal_manager.commit_transaction(transaction_id)?;

        Ok(())
    }
}

// Helper functions
fn get_current_time() -> u64 {
    static mut TIME_COUNTER: u64 = 1;
    unsafe {
        TIME_COUNTER += 1;
        TIME_COUNTER
    }
}

fn create_empty_index_root() -> Vec<u8> {
    // Create empty B+ tree index root for directory
    let mut data = Vec::new();
    data.extend_from_slice(b"INDX"); // Signature
    data.extend_from_slice(&0u32.to_le_bytes()); // Entry count
    data.extend_from_slice(&16u32.to_le_bytes()); // Index size
    data.extend_from_slice(&0u32.to_le_bytes()); // Flags
    data
}
