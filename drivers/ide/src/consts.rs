#![allow(unused)]

// ATA Status Register bits
pub const ATA_SR_BSY: u8 = 0x80;    // Busy
pub const ATA_SR_DRDY: u8 = 0x40;   // Drive ready
pub const ATA_SR_DF: u8 = 0x20;     // Drive write fault
pub const ATA_SR_DSC: u8 = 0x10;    // Drive seek complete
pub const ATA_SR_DRQ: u8 = 0x08;    // Data request ready
pub const ATA_SR_CORR: u8 = 0x04;   // Corrected data
pub const ATA_SR_IDX: u8 = 0x02;    // Index
pub const ATA_SR_ERR: u8 = 0x01;    // Error

// ATA Error Register bits
pub const ATA_ER_BBK: u8 = 0x80;    // Bad block
pub const ATA_ER_UNC: u8 = 0x40;    // Uncorrectable data
pub const ATA_ER_MC: u8 = 0x20;     // Media changed
pub const ATA_ER_IDNF: u8 = 0x10;   // ID mark not found
pub const ATA_ER_MCR: u8 = 0x08;    // Media change request
pub const ATA_ER_ABRT: u8 = 0x04;   // Command aborted
pub const ATA_ER_TK0NF: u8 = 0x02;  // Track 0 not found
pub const ATA_ER_AMNF: u8 = 0x01;   // No address mark

// ATA Commands
pub const ATA_CMD_READ_PIO: u8 = 0x20;
pub const ATA_CMD_READ_PIO_EXT: u8 = 0x24;
pub const ATA_CMD_READ_DMA: u8 = 0xC8;
pub const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
pub const ATA_CMD_WRITE_PIO: u8 = 0x30;
pub const ATA_CMD_WRITE_PIO_EXT: u8 = 0x34;
pub const ATA_CMD_WRITE_DMA: u8 = 0xCA;
pub const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
pub const ATA_CMD_CACHE_FLUSH: u8 = 0xE7;
pub const ATA_CMD_CACHE_FLUSH_EXT: u8 = 0xEA;
pub const ATA_CMD_PACKET: u8 = 0xA0;
pub const ATA_CMD_IDENTIFY_PACKET: u8 = 0xA1;
pub const ATA_CMD_IDENTIFY: u8 = 0xEC;

// ATAPI Commands
pub const ATAPI_CMD_READ: u8 = 0xA8;
pub const ATAPI_CMD_EJECT: u8 = 0x1B;

// ATA Identity Data offsets (in words)
pub const ATA_IDENT_DEVICETYPE: u8 = 0;
pub const ATA_IDENT_CYLINDERS: u8 = 2;
pub const ATA_IDENT_HEADS: u8 = 6;
pub const ATA_IDENT_SECTORS: u8 = 12;
pub const ATA_IDENT_SERIAL: u8 = 20;
pub const ATA_IDENT_MODEL: u8 = 54;
pub const ATA_IDENT_CAPABILITIES: u8 = 98;
pub const ATA_IDENT_FIELDVALID: u8 = 106;
pub const ATA_IDENT_MAX_LBA: u8 = 120;
pub const ATA_IDENT_COMMANDSETS: u8 = 164;
pub const ATA_IDENT_MAX_LBA_EXT: u8 = 200;

// Device types
pub const IDE_ATA: u8 = 0x00;       // Standard ATA device
pub const IDE_ATAPI: u8 = 0x01;     // ATAPI device (CD-ROM, etc.)

// Drive selection
pub const ATA_MASTER: u8 = 0x00;    // Master drive
pub const ATA_SLAVE: u8 = 0x01;     // Slave drive

// ATA Register offsets
pub const ATA_REG_DATA: u8 = 0x00;      // Data register
pub const ATA_REG_ERROR: u8 = 0x01;     // Error/Features register
pub const ATA_REG_FEATURES: u8 = 0x01;  // Features register
pub const ATA_REG_SECCOUNT0: u8 = 0x02; // Sector count register
pub const ATA_REG_LBA0: u8 = 0x03;      // LBA register 0
pub const ATA_REG_LBA1: u8 = 0x04;      // LBA register 1
pub const ATA_REG_LBA2: u8 = 0x05;      // LBA register 2
pub const ATA_REG_HDDEVSEL: u8 = 0x06;  // Head/drive select register
pub const ATA_REG_COMMAND: u8 = 0x07;   // Command register
pub const ATA_REG_STATUS: u8 = 0x07;    // Status register (read-only)
pub const ATA_REG_SECCOUNT1: u8 = 0x08; // Sector count 1 (for 48-bit LBA)
pub const ATA_REG_LBA3: u8 = 0x09;      // LBA register 3 (for 48-bit LBA)
pub const ATA_REG_LBA4: u8 = 0x0A;      // LBA register 4 (for 48-bit LBA)
pub const ATA_REG_LBA5: u8 = 0x0B;      // LBA register 5 (for 48-bit LBA)
pub const ATA_REG_CONTROL: u8 = 0x0C;   // Control register
pub const ATA_REG_ALTSTATUS: u8 = 0x0C; // Alternate status register (read-only)
pub const ATA_REG_DEVADDRESS: u8 = 0x0D; // Device address register

// Channel types
pub const ATA_PRIMARY: u8 = 0x00;       // Primary channel
pub const ATA_SECONDARY: u8 = 0x01;     // Secondary channel

// Operation directions
pub const ATA_READ: u8 = 0x00;          // Read operation
pub const ATA_WRITE: u8 = 0x01;         // Write operation