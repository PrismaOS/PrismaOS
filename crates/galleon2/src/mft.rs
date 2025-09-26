//! Master File Table (MFT) Implementation
//!
//! Core metadata storage system using Master File Table architecture.
//! Every file and directory is represented as a record in the MFT.

use alloc::{vec, vec::Vec, string::String};
use crate::{FilesystemResult, FilesystemError, ide_read_sectors, ide_write_sectors};

pub const MFT_RECORD_SIZE: usize = 1024; // 1KB per record
pub const MFT_RECORDS_PER_SECTOR: usize = 512 / MFT_RECORD_SIZE;

/// File Record Number - unique identifier for each file/directory
pub type FileRecordNumber = u64;

/// Special system file record numbers
pub const MFT_RECORD_MFT: FileRecordNumber = 0;      // MFT itself
pub const MFT_RECORD_MIRROR: FileRecordNumber = 1;   // MFT backup
pub const MFT_RECORD_LOG: FileRecordNumber = 2;      // Journal log
pub const MFT_RECORD_VOLUME: FileRecordNumber = 3;   // Volume info
pub const MFT_RECORD_ATTRDEF: FileRecordNumber = 4;  // Attribute definitions
pub const MFT_RECORD_ROOT: FileRecordNumber = 5;     // Root directory
pub const MFT_RECORD_BITMAP: FileRecordNumber = 6;   // Cluster bitmap
pub const MFT_RECORD_BOOT: FileRecordNumber = 7;     // Boot sector
pub const MFT_RECORD_BADCLUS: FileRecordNumber = 8;  // Bad cluster list

/// MFT Record flags
#[derive(Debug, Clone, Copy)]
pub struct RecordFlags {
    pub in_use: bool,
    pub is_directory: bool,
    pub has_view_index: bool,
}

impl RecordFlags {
    pub fn new() -> Self {
        Self {
            in_use: false,
            is_directory: false,
            has_view_index: false,
        }
    }

    pub fn to_u16(&self) -> u16 {
        let mut flags = 0u16;
        if self.in_use { flags |= 0x0001; }
        if self.is_directory { flags |= 0x0002; }
        if self.has_view_index { flags |= 0x0004; }
        flags
    }

    pub fn from_u16(value: u16) -> Self {
        Self {
            in_use: (value & 0x0001) != 0,
            is_directory: (value & 0x0002) != 0,
            has_view_index: (value & 0x0004) != 0,
        }
    }
}

/// MFT Record Header
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MftRecordHeader {
    pub signature: [u8; 4],            // "FILE"
    pub update_sequence_offset: u16,   // Offset to update sequence
    pub update_sequence_size: u16,     // Size of update sequence
    pub log_file_sequence_number: u64, // Journal sequence number
    pub sequence_number: u16,          // Record sequence number
    pub hard_link_count: u16,          // Number of hard links
    pub first_attribute_offset: u16,   // Offset to first attribute
    pub flags: u16,                    // Record flags
    pub bytes_in_use: u32,             // Bytes used in this record
    pub bytes_allocated: u32,          // Bytes allocated for this record
    pub base_file_record: u64,         // Base file record (for attribute lists)
    pub next_attribute_id: u16,        // Next attribute ID
}

impl MftRecordHeader {
    pub fn new(record_number: FileRecordNumber) -> Self {
        Self {
            signature: *b"FILE",
            update_sequence_offset: 48,
            update_sequence_size: 3,
            log_file_sequence_number: 0,
            sequence_number: 1,
            hard_link_count: 1,
            first_attribute_offset: 56,
            flags: RecordFlags::new().to_u16(),
            bytes_in_use: 56,
            bytes_allocated: MFT_RECORD_SIZE as u32,
            base_file_record: record_number,
            next_attribute_id: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        &self.signature == b"FILE" &&
        self.bytes_in_use <= self.bytes_allocated &&
        self.bytes_allocated == MFT_RECORD_SIZE as u32
    }

    pub fn get_flags(&self) -> RecordFlags {
        RecordFlags::from_u16(self.flags)
    }

    pub fn set_flags(&mut self, flags: RecordFlags) {
        self.flags = flags.to_u16();
    }
}

/// Complete MFT Record
#[derive(Debug, Clone)]
pub struct MftRecord {
    pub header: MftRecordHeader,
    pub attributes: Vec<Attribute>,
    pub record_number: FileRecordNumber,
}

impl MftRecord {
    pub fn new(record_number: FileRecordNumber) -> Self {
        Self {
            header: MftRecordHeader::new(record_number),
            attributes: Vec::new(),
            record_number,
        }
    }

    pub fn add_attribute(&mut self, attr: Attribute) {
        self.attributes.push(attr);
        self.header.next_attribute_id += 1;
        // Update bytes_in_use based on attributes
        self.update_bytes_in_use();
    }

    fn update_bytes_in_use(&mut self) {
        let mut total = self.header.first_attribute_offset as u32;
        for attr in &self.attributes {
            total += attr.get_size();
        }
        total += 4; // End marker
        self.header.bytes_in_use = total;
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = vec![0u8; MFT_RECORD_SIZE];
        let mut offset = 0;

        // Serialize header
        data[offset..offset+4].copy_from_slice(&self.header.signature);
        offset += 4;
        data[offset..offset+2].copy_from_slice(&self.header.update_sequence_offset.to_le_bytes());
        offset += 2;
        data[offset..offset+2].copy_from_slice(&self.header.update_sequence_size.to_le_bytes());
        offset += 2;
        data[offset..offset+8].copy_from_slice(&self.header.log_file_sequence_number.to_le_bytes());
        offset += 8;
        data[offset..offset+2].copy_from_slice(&self.header.sequence_number.to_le_bytes());
        offset += 2;
        data[offset..offset+2].copy_from_slice(&self.header.hard_link_count.to_le_bytes());
        offset += 2;
        data[offset..offset+2].copy_from_slice(&self.header.first_attribute_offset.to_le_bytes());
        offset += 2;
        data[offset..offset+2].copy_from_slice(&self.header.flags.to_le_bytes());
        offset += 2;
        data[offset..offset+4].copy_from_slice(&self.header.bytes_in_use.to_le_bytes());
        offset += 4;
        data[offset..offset+4].copy_from_slice(&self.header.bytes_allocated.to_le_bytes());
        offset += 4;
        data[offset..offset+8].copy_from_slice(&self.header.base_file_record.to_le_bytes());
        offset += 8;
        data[offset..offset+2].copy_from_slice(&self.header.next_attribute_id.to_le_bytes());
        offset += 2;

        // Skip to first attribute offset
        offset = self.header.first_attribute_offset as usize;

        // Serialize attributes
        for attr in &self.attributes {
            let attr_data = attr.serialize();
            data[offset..offset+attr_data.len()].copy_from_slice(&attr_data);
            offset += attr_data.len();
        }

        // End marker
        data[offset..offset+4].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());

        data
    }

    pub fn deserialize(data: &[u8], record_number: FileRecordNumber) -> FilesystemResult<Self> {
        if data.len() < MFT_RECORD_SIZE {
            return Err(FilesystemError::InvalidParameter);
        }

        let mut offset = 0;

        // Deserialize header
        let mut signature = [0u8; 4];
        signature.copy_from_slice(&data[offset..offset+4]);
        offset += 4;

        let update_sequence_offset = u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());
        offset += 2;
        let update_sequence_size = u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());
        offset += 2;
        let log_file_sequence_number = u64::from_le_bytes(data[offset..offset+8].try_into().unwrap());
        offset += 8;
        let sequence_number = u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());
        offset += 2;
        let hard_link_count = u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());
        offset += 2;
        let first_attribute_offset = u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());
        offset += 2;
        let flags = u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());
        offset += 2;
        let bytes_in_use = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        offset += 4;
        let bytes_allocated = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        offset += 4;
        let base_file_record = u64::from_le_bytes(data[offset..offset+8].try_into().unwrap());
        offset += 8;
        let next_attribute_id = u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());

        let header = MftRecordHeader {
            signature,
            update_sequence_offset,
            update_sequence_size,
            log_file_sequence_number,
            sequence_number,
            hard_link_count,
            first_attribute_offset,
            flags,
            bytes_in_use,
            bytes_allocated,
            base_file_record,
            next_attribute_id,
        };

        if !header.is_valid() {
            return Err(FilesystemError::InvalidParameter);
        }

        // Parse attributes
        let mut attributes = Vec::new();
        offset = first_attribute_offset as usize;

        while offset < bytes_in_use as usize {
            let attr_type = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
            if attr_type == 0xFFFFFFFF {
                break; // End marker
            }

            let attr = Attribute::deserialize(&data[offset..])?;
            offset += attr.get_size() as usize;
            attributes.push(attr);
        }

        Ok(Self {
            header,
            attributes,
            record_number,
        })
    }
}

/// File attribute types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttributeType {
    StandardInformation = 0x10,
    FileName = 0x30,
    Data = 0x80,
    IndexRoot = 0x90,
    IndexAllocation = 0xA0,
    Bitmap = 0xB0,
}

/// Attribute header
#[derive(Debug, Clone)]
pub struct AttributeHeader {
    pub attr_type: u32,
    pub length: u32,
    pub non_resident: bool,
    pub name_length: u8,
    pub name_offset: u16,
    pub flags: u16,
    pub attribute_id: u16,
}

/// File attribute for storing file data
#[derive(Debug, Clone)]
pub struct Attribute {
    pub header: AttributeHeader,
    pub name: String,
    pub data: AttributeData,
}

#[derive(Debug, Clone)]
pub enum AttributeData {
    Resident(Vec<u8>),
    NonResident {
        runs: Vec<DataRun>,
        allocated_size: u64,
        real_size: u64,
        initialized_size: u64,
    },
}

/// Data run for non-resident attributes (extents)
pub type DataRun = crate::allocation::ClusterRun;

impl Attribute {
    pub fn new_resident(attr_type: AttributeType, data: Vec<u8>) -> Self {
        let header = AttributeHeader {
            attr_type: attr_type as u32,
            length: 24 + data.len() as u32, // Header + data
            non_resident: false,
            name_length: 0,
            name_offset: 24,
            flags: 0,
            attribute_id: 0,
        };

        Self {
            header,
            name: String::new(),
            data: AttributeData::Resident(data),
        }
    }

    pub fn new_non_resident(attr_type: AttributeType, runs: Vec<DataRun>, real_size: u64) -> Self {
        let allocated_size = runs.iter().map(|r| r.cluster_count * 4096).sum(); // Assuming 4KB clusters

        let header = AttributeHeader {
            attr_type: attr_type as u32,
            length: 64, // Non-resident header size
            non_resident: true,
            name_length: 0,
            name_offset: 64,
            flags: 0,
            attribute_id: 0,
        };

        Self {
            header,
            name: String::new(),
            data: AttributeData::NonResident {
                runs,
                allocated_size,
                real_size,
                initialized_size: real_size,
            },
        }
    }

    pub fn get_size(&self) -> u32 {
        self.header.length
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Serialize header
        data.extend_from_slice(&self.header.attr_type.to_le_bytes());
        data.extend_from_slice(&self.header.length.to_le_bytes());
        data.push(if self.header.non_resident { 1 } else { 0 });
        data.push(self.header.name_length);
        data.extend_from_slice(&self.header.name_offset.to_le_bytes());
        data.extend_from_slice(&self.header.flags.to_le_bytes());
        data.extend_from_slice(&self.header.attribute_id.to_le_bytes());

        match &self.data {
            AttributeData::Resident(content) => {
                data.extend_from_slice(&(content.len() as u32).to_le_bytes());
                data.extend_from_slice(&24u16.to_le_bytes()); // Data offset
                data.extend_from_slice(&0u16.to_le_bytes()); // Padding
                data.extend_from_slice(content);
            }
            AttributeData::NonResident { runs, allocated_size, real_size, initialized_size } => {
                data.extend_from_slice(&0u64.to_le_bytes()); // Starting VCN
                data.extend_from_slice(&0u64.to_le_bytes()); // Ending VCN
                data.extend_from_slice(&40u16.to_le_bytes()); // Run list offset
                data.extend_from_slice(&0u16.to_le_bytes()); // Compression unit
                data.extend_from_slice(&0u32.to_le_bytes()); // Padding
                data.extend_from_slice(&allocated_size.to_le_bytes());
                data.extend_from_slice(&real_size.to_le_bytes());
                data.extend_from_slice(&initialized_size.to_le_bytes());

                // Serialize run list
                for run in runs {
                    // Simplified run encoding
                    data.extend_from_slice(&run.cluster_count.to_le_bytes());
                    data.extend_from_slice(&run.start_cluster.to_le_bytes());
                }
                data.push(0); // End of runs
            }
        }

        // Pad to 8-byte boundary
        while data.len() % 8 != 0 {
            data.push(0);
        }

        data
    }

    pub fn deserialize(data: &[u8]) -> FilesystemResult<Self> {
        if data.len() < 16 {
            return Err(FilesystemError::InvalidParameter);
        }

        let attr_type = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let length = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let non_resident = data[8] != 0;
        let name_length = data[9];
        let name_offset = u16::from_le_bytes(data[10..12].try_into().unwrap());
        let flags = u16::from_le_bytes(data[12..14].try_into().unwrap());
        let attribute_id = u16::from_le_bytes(data[14..16].try_into().unwrap());

        let header = AttributeHeader {
            attr_type,
            length,
            non_resident,
            name_length,
            name_offset,
            flags,
            attribute_id,
        };

        let attr_data = if non_resident {
            // Parse non-resident data (simplified)
            let allocated_size = u64::from_le_bytes(data[32..40].try_into().unwrap());
            let real_size = u64::from_le_bytes(data[40..48].try_into().unwrap());
            let initialized_size = u64::from_le_bytes(data[48..56].try_into().unwrap());

            AttributeData::NonResident {
                runs: Vec::new(), // Simplified - would parse run list here
                allocated_size,
                real_size,
                initialized_size,
            }
        } else {
            let content_size = u32::from_le_bytes(data[16..20].try_into().unwrap());
            let data_offset = u16::from_le_bytes(data[20..22].try_into().unwrap()) as usize;
            let content = data[data_offset..data_offset + content_size as usize].to_vec();
            AttributeData::Resident(content)
        };

        Ok(Self {
            header,
            name: String::new(),
            data: attr_data,
        })
    }
}

/// MFT Manager
pub struct MftManager {
    pub drive: u8,
    pub mft_start_cluster: u64,
    pub cluster_size: u32,
}

impl MftManager {
    pub fn new(drive: u8, mft_start_cluster: u64, cluster_size: u32) -> Self {
        Self {
            drive,
            mft_start_cluster,
            cluster_size,
        }
    }

    pub fn read_record(&self, record_number: FileRecordNumber) -> FilesystemResult<MftRecord> {
        let records_per_cluster = self.cluster_size as usize / MFT_RECORD_SIZE;
        if records_per_cluster == 0 {
            return Err(FilesystemError::InvalidParameter); // Cluster too small for MFT records
        }
        let cluster_offset = record_number / records_per_cluster as u64;
        let record_offset = (record_number % records_per_cluster as u64) * MFT_RECORD_SIZE as u64;

        let sectors_per_cluster = (self.cluster_size / 512).max(1) as u64; // Ensure at least 1 sector per cluster
        let cluster_sector = (self.mft_start_cluster + cluster_offset) * sectors_per_cluster;
        let mut cluster_data = vec![0u8; self.cluster_size as usize];

        // Read the cluster containing the record
        ide_read_sectors(self.drive, sectors_per_cluster as u8, cluster_sector as u32, &mut cluster_data)?;

        let record_data = &cluster_data[record_offset as usize..(record_offset as usize + MFT_RECORD_SIZE)];
        MftRecord::deserialize(record_data, record_number)
    }

    pub fn write_record(&self, record: &MftRecord) -> FilesystemResult<()> {
        let records_per_cluster = self.cluster_size as usize / MFT_RECORD_SIZE;
        if records_per_cluster == 0 {
            return Err(FilesystemError::InvalidParameter); // Cluster too small for MFT records
        }
        let cluster_offset = record.record_number / records_per_cluster as u64;
        let record_offset = (record.record_number % records_per_cluster as u64) * MFT_RECORD_SIZE as u64;

        let sectors_per_cluster = (self.cluster_size / 512).max(1) as u64; // Ensure at least 1 sector per cluster
        let cluster_sector = (self.mft_start_cluster + cluster_offset) * sectors_per_cluster;
        let mut cluster_data = vec![0u8; self.cluster_size as usize];

        // Read existing cluster data
        ide_read_sectors(self.drive, sectors_per_cluster as u8, cluster_sector as u32, &mut cluster_data)?;

        // Update the specific record
        let record_data = record.serialize();
        cluster_data[record_offset as usize..(record_offset as usize + MFT_RECORD_SIZE)]
            .copy_from_slice(&record_data[..MFT_RECORD_SIZE]);

        // Write back the cluster
        ide_write_sectors(self.drive, sectors_per_cluster as u8, cluster_sector as u32, &cluster_data)?;
        Ok(())
    }

    pub fn allocate_record(&self) -> FilesystemResult<FileRecordNumber> {
        // Simplified allocation - would typically use MFT bitmap
        // For now, we'll use a simple counter approach
        // In a real implementation, this would scan the MFT bitmap
        static mut NEXT_RECORD: FileRecordNumber = 16; // Start after system records

        unsafe {
            let record_number = NEXT_RECORD;
            NEXT_RECORD += 1;
            Ok(record_number)
        }
    }
}
