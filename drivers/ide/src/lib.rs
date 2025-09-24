// lib.rs - IDE Driver with fixes for size calculation and model string corruption
#![no_std]

extern crate alloc;

use lib_kernel::{api::commands::{inb, insw, outb, outsw}, kprintln};

pub mod consts;
pub mod types;
use consts::*;
use types::{IDEChannelRegistors, IdeDevice};

// Global IDE data structures
static mut IDE_BUF: [u8; 2048] = [0; 2048];
static mut IDE_IRQ_INVOKED: u8 = 0;
static mut ATAPI_PACKET: [u8; 12] = [0xA8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// IDE Error types for no-std environment
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IdeError {
    /// Drive not found or not present
    DriveNotFound,
    /// Drive fault detected
    DriveFault,
    /// Error bit set in status register
    StatusError,
    /// Data request not ready
    DataNotReady,
    /// LBA out of supported range
    LbaOutOfRange,
    /// Timeout waiting for drive
    Timeout,
    /// Invalid parameter
    InvalidParameter,
}

/// Result type for IDE operations
pub type IdeResult<T> = Result<T, IdeError>;

/// IDE channel configurations
static mut CHANNELS: [IDEChannelRegistors; 2] = [
    IDEChannelRegistors {
        base: 0x1F0,
        ctrl: 0x3F6,
        bmide: 0,
        n_ien: 0,
    },
    IDEChannelRegistors {
        base: 0x170,
        ctrl: 0x376,
        bmide: 0,
        n_ien: 0,
    },
];

static mut IDE_DEVICES: [IdeDevice; 4] = [IdeDevice::new(); 4];

fn sleep(ms: u32) {
    for _ in 0..(ms * 10000) {  // Increased delay multiplier
        core::hint::spin_loop();
    }
}

/* ============================================================================
 * LOW-LEVEL PORT I/O FUNCTIONS
 * ============================================================================ */

fn ide_read(channel: u8, reg: u8) -> u8 {
    unsafe {
        if reg > 0x07 && reg < 0x0C {
            ide_write(channel, ATA_REG_CONTROL, CHANNELS[channel as usize].n_ien | 0x02);
        }

        let port: u16 = if reg < 0x08 {
            CHANNELS[channel as usize].base + reg as u16
        } else {
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };

        inb(port)
    }
}

fn ide_write(channel: u8, reg: u8, data: u8) {
    unsafe {
        if reg > 0x07 && reg < 0x0C {
            ide_write(channel, ATA_REG_CONTROL, CHANNELS[channel as usize].n_ien | 0x02);
        }

        let port: u16 = if reg < 0x08 {
            CHANNELS[channel as usize].base + reg as u16
        } else {
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };

        outb(port, data);
    }
}

fn ide_read_buffer(channel: u8, reg: u8, buffer: *mut u32, quads: u32) {
    unsafe {
        if reg > 0x07 && reg < 0x0C {
            ide_write(channel, ATA_REG_CONTROL, CHANNELS[channel as usize].n_ien | 0x02);
        }

        let port: u16 = if reg < 0x08 {
            CHANNELS[channel as usize].base + reg as u16
        } else {
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };

        insw(port, buffer as *mut u16, quads);
    }
}

/* ============================================================================
 * SECTOR READ OPERATIONS WITH ERROR HANDLING
 * ============================================================================ */

/// Read sectors from an IDE drive using PIO mode with proper error handling
pub fn ide_read_sectors(
    drive: u8,
    numsects: u8,
    lba: u32,
    buf: *mut core::ffi::c_void
) -> IdeResult<()> {
    unsafe {
        // Validate parameters
        if drive > 3 {
            return Err(IdeError::InvalidParameter);
        }
        
        if IDE_DEVICES[drive as usize].reserved == 0 {
            kprintln!("Drive {} not found or not initialized", drive);
            return Err(IdeError::DriveNotFound);
        }

        let device = &IDE_DEVICES[drive as usize];
        let channel: u8 = device.channel;
        let slavebit: u8 = device.drive;
        let bus: u16 = CHANNELS[channel as usize].base;
        
        kprintln!("IDE Read: drive={}, sectors={}, lba={}", drive, numsects, lba);

        // Prepare LBA addressing
        let mut lba_io: [u8; 6] = [0; 6];
        let head: u8;

        // Check if we need 48-bit LBA (for drives > 128GB)
        if lba >= 0x10000000 {
            // 48-bit LBA mode
            lba_io[0] = (lba & 0xFF) as u8;
            lba_io[1] = ((lba >> 8) & 0xFF) as u8;
            lba_io[2] = ((lba >> 16) & 0xFF) as u8;
            lba_io[3] = ((lba >> 24) & 0xFF) as u8;
            lba_io[4] = 0;
            lba_io[5] = 0;
            head = 0;
        } else {
            // 28-bit LBA mode
            lba_io[0] = (lba & 0xFF) as u8;
            lba_io[1] = ((lba >> 8) & 0xFF) as u8;
            lba_io[2] = ((lba >> 16) & 0xFF) as u8;
            lba_io[3] = ((lba >> 24) & 0x0F) as u8;
            lba_io[4] = 0;
            lba_io[5] = 0;
            head = ((lba >> 24) & 0x0F) as u8;
        }

        // Select drive and set LBA head bits
        let drive_select = 0xE0 | (slavebit << 4) | head;
        ide_write(channel, ATA_REG_HDDEVSEL, drive_select);
        sleep(1);

        // Set up the read operation
        ide_write(channel, ATA_REG_SECCOUNT0, numsects);
        ide_write(channel, ATA_REG_LBA0, lba_io[0]);
        ide_write(channel, ATA_REG_LBA1, lba_io[1]);
        ide_write(channel, ATA_REG_LBA2, lba_io[2]);
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_READ_PIO);

        // Read each sector
        for i in 0..numsects {
            ide_polling(channel, true)?;
            
            insw(
                bus,
                (buf as *mut u8).add(i as usize * 512) as *mut u16,
                256,
            );
        }

        Ok(())
    }
}

/* ============================================================================
 * SECTOR WRITE OPERATIONS WITH ERROR HANDLING
 * ============================================================================ */

/// Write sectors to an IDE drive using PIO mode with proper error handling
pub fn ide_write_sectors(
    drive: u8,
    numsects: u8,
    lba: u32,
    buf: *const core::ffi::c_void
) -> IdeResult<()> {
    unsafe {
        // Validate parameters
        if drive > 3 {
            return Err(IdeError::InvalidParameter);
        }
        
        if IDE_DEVICES[drive as usize].reserved == 0 {
            return Err(IdeError::DriveNotFound);
        }

        let channel: u8 = IDE_DEVICES[drive as usize].channel;
        let slavebit: u8 = IDE_DEVICES[drive as usize].drive;
        let bus: u16 = CHANNELS[channel as usize].base;
        let mut lba_io: [u8; 6] = [0; 6];
        let head: u8;

        // We only support 28-bit LBA for writes in this implementation
        if lba >= 0x10000000 {
            return Err(IdeError::LbaOutOfRange);
        }

        // 28-bit LBA mode
        lba_io[0] = (lba & 0xFF) as u8;
        lba_io[1] = ((lba >> 8) & 0xFF) as u8;
        lba_io[2] = ((lba >> 16) & 0xFF) as u8;
        lba_io[3] = ((lba >> 24) & 0x0F) as u8;
        lba_io[4] = 0;
        lba_io[5] = 0;
        head = ((lba >> 24) & 0x0F) as u8;

        // Select drive and set LBA head bits
        ide_write(channel, ATA_REG_HDDEVSEL, 0xE0 | (slavebit << 4) | head);
        sleep(1);

        // Set up the write operation
        ide_write(channel, ATA_REG_SECCOUNT0, numsects);
        ide_write(channel, ATA_REG_LBA0, lba_io[0]);
        ide_write(channel, ATA_REG_LBA1, lba_io[1]);
        ide_write(channel, ATA_REG_LBA2, lba_io[2]);
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_WRITE_PIO);

        // Write each sector
        for i in 0..numsects {
            ide_polling(channel, false)?;
            
            outsw(
                bus,
                (buf as *const u8).add(i as usize * 512) as *const u16,
                256,
            );
        }

        // Flush the write cache
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_CACHE_FLUSH);
        ide_polling(channel, false)?;

        Ok(())
    }
}

/* ============================================================================
 * DEVICE POLLING WITH ERROR HANDLING
 * ============================================================================ */

/// Poll IDE device status with proper error handling and timeout
fn ide_polling(channel: u8, advanced_check: bool) -> IdeResult<()> {
    // Delay 400ns by reading Alternate Status Register 4 times
    for _ in 0..4 {
        ide_read(channel, ATA_REG_ALTSTATUS);
    }

    // Wait for the BSY (Busy) bit to clear with timeout
    let mut timeout = 0;
    while (ide_read(channel, ATA_REG_STATUS) & ATA_SR_BSY) != 0 {
        timeout += 1;
        if timeout > 1000000 {  // Increased timeout
            return Err(IdeError::Timeout);
        }
    }

    if advanced_check {
        let state = ide_read(channel, ATA_REG_STATUS);

        if (state & ATA_SR_ERR) != 0 {
            let error = ide_read(channel, ATA_REG_ERROR);
            kprintln!("IDE Status Error - Status: 0x{:02X}, Error: 0x{:02X}", state, error);
            return Err(IdeError::StatusError);
        }

        if (state & ATA_SR_DF) != 0 {
            kprintln!("IDE Drive Fault detected");
            return Err(IdeError::DriveFault);
        }

        if (state & ATA_SR_DRQ) == 0 {
            kprintln!("IDE Data Request not ready");
            return Err(IdeError::DataNotReady);
        }
    }

    Ok(())
}

/* ============================================================================
 * DEVICE IDENTIFICATION WITH ERROR HANDLING - FIXED VERSION
 * ============================================================================ */

/// Identify and initialize an IDE device with error handling - FIXED
fn ide_identify(channel: u8, drive: u8) -> IdeResult<()> {
    unsafe {
        let mut status: u8;
        let bus = CHANNELS[channel as usize].base;

        // Select the drive
        ide_write(channel, ATA_REG_HDDEVSEL, 0xA0 | (drive << 4));
        sleep(10);  // Increased delay

        // Send the IDENTIFY command
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_IDENTIFY);
        sleep(1);

        // Check if drive exists
        status = ide_read(channel, ATA_REG_STATUS);
        if status == 0 {
            IDE_DEVICES[(channel as usize) * 2 + drive as usize].reserved = 0;
            return Err(IdeError::DriveNotFound);
        }

        // Wait for the drive to finish processing with timeout
        let mut timeout = 0;
        while {
            status = ide_read(channel, ATA_REG_STATUS);
            (status & ATA_SR_BSY) != 0
        } {
            timeout += 1;
            if timeout > 1000000 {  // Increased timeout
                return Err(IdeError::Timeout);
            }
        }

        // Check for errors and determine device type
        let err = ide_read(channel, ATA_REG_ERROR);
        let type_ = if err != 0 {
            ide_write(channel, ATA_REG_COMMAND, ATA_CMD_IDENTIFY_PACKET);
            sleep(1);
            IDE_ATAPI
        } else {
            IDE_ATA
        };

        // Initialize device structure
        let device_index = (channel as usize) * 2 + drive as usize;
        
        // Clear the device structure first
        IDE_DEVICES[device_index] = IdeDevice::new();
        
        IDE_DEVICES[device_index].reserved = 1;
        IDE_DEVICES[device_index].channel = channel;
        IDE_DEVICES[device_index].drive = drive;
        IDE_DEVICES[device_index].drive_type = type_ as u16;

        // Clear the buffer before reading
        for i in 0..2048 {
            IDE_BUF[i] = 0;
        }

        // Read IDENTIFY data
        insw(bus, IDE_BUF.as_mut_ptr() as *mut u16, 256);

        let buf_ptr_u16 = IDE_BUF.as_ptr() as *const u16;
        
        if type_ == IDE_ATA {
            // FIXED: Proper capacity calculation for ATA drives
            IDE_DEVICES[device_index].signature = *buf_ptr_u16.add(0);
            IDE_DEVICES[device_index].capabilities = *buf_ptr_u16.add(49);
            
            // FIXED: Read command sets properly (words 82-83)
            let cmd_set_low = *buf_ptr_u16.add(82) as u32;
            let cmd_set_high = *buf_ptr_u16.add(83) as u32;
            IDE_DEVICES[device_index].command_sets = cmd_set_low | (cmd_set_high << 16);
            
            // FIXED: Proper size calculation - read words 60-61 for 28-bit LBA
            let size_low = *buf_ptr_u16.add(60) as u32;
            let size_high = *buf_ptr_u16.add(61) as u32;
            let sectors_28bit = size_low | (size_high << 16);
            
            // Check if drive supports 48-bit LBA (bit 10 of word 83)
            let supports_48bit = (IDE_DEVICES[device_index].command_sets & (1 << 26)) != 0;
            
            if supports_48bit && sectors_28bit == 0xFFFFFFFF {
                // Use 48-bit LBA capacity (words 100-103)
                let lba48_0 = *buf_ptr_u16.add(100) as u64;
                let lba48_1 = *buf_ptr_u16.add(101) as u64;
                let lba48_2 = *buf_ptr_u16.add(102) as u64;
                let lba48_3 = *buf_ptr_u16.add(103) as u64;
                let sectors_48bit = lba48_0 | (lba48_1 << 16) | (lba48_2 << 32) | (lba48_3 << 48);
                IDE_DEVICES[device_index].size = sectors_48bit as u32; // Truncate for now
            } else {
                IDE_DEVICES[device_index].size = sectors_28bit;
            }
            
            // FIXED: Proper model string extraction with byte swapping
            for i in 0..20 {  // 20 words = 40 characters
                let word = *buf_ptr_u16.add(ATA_IDENT_MODEL as usize / 2 + i);
                // ATA strings are byte-swapped within each word
                IDE_DEVICES[device_index].model[i * 2] = (word >> 8) as u8;     // High byte first
                IDE_DEVICES[device_index].model[i * 2 + 1] = (word & 0xFF) as u8; // Then low byte
            }
            IDE_DEVICES[device_index].model[40] = 0; // Null terminator
            
        } else if type_ == IDE_ATAPI {
            // ATAPI device handling
            IDE_DEVICES[device_index].signature = *buf_ptr_u16.add(0);
            IDE_DEVICES[device_index].capabilities = *buf_ptr_u16.add(49);
            
            // For ATAPI, size is different - this is more of a device identifier
            IDE_DEVICES[device_index].size = 0; // ATAPI doesn't have a fixed size like ATA
            
            // FIXED: Proper model string extraction for ATAPI too
            for i in 0..20 {  // 20 words = 40 characters
                let word = *buf_ptr_u16.add(ATA_IDENT_MODEL as usize / 2 + i);
                // ATA strings are byte-swapped within each word
                IDE_DEVICES[device_index].model[i * 2] = (word >> 8) as u8;     // High byte first
                IDE_DEVICES[device_index].model[i * 2 + 1] = (word & 0xFF) as u8; // Then low byte
            }
            IDE_DEVICES[device_index].model[40] = 0; // Null terminator
        }

        kprintln!("Device {} identified: type={}, size={} sectors", 
                  device_index, IDE_DEVICES[device_index].drive_type, IDE_DEVICES[device_index].size);

        Ok(())
    }
}

/* ============================================================================
 * INITIALIZATION
 * ============================================================================ */

pub fn ide_initialize() -> IdeResult<()> {
    unsafe {
        kprintln!("Starting IDE initialization...");
        
        #[allow(clippy::needless_range_loop)]
        for i in 0..2 {
            CHANNELS[i].n_ien = 0x02;
        }

        #[allow(clippy::needless_range_loop)]
        for i in 0..4 {
            IDE_DEVICES[i] = IdeDevice::new(); // Properly initialize each device
        }

        for i in 0..4 {
            let channel = (i / 2) as u8;
            let drive = (i % 2) as u8;
            kprintln!("Identifying device {}: channel={}, drive={}", i, channel, drive);
            
            match ide_identify(channel, drive) {
                Ok(()) => kprintln!("Device {} identified successfully", i),
                Err(e) => kprintln!("Device {} identification failed: {:?}", i, e),
            }
        }

        kprintln!("IDE initialization complete. Detected devices:");
        #[allow(clippy::needless_range_loop)]
        for i in 0..4 {
            if IDE_DEVICES[i].reserved != 0 {
                let device = &IDE_DEVICES[i];
                
                // FIXED: Proper string handling for model
                let mut model_end = 40;
                for j in 0..40 {
                    if device.model[j] == 0 {
                        model_end = j;
                        break;
                    }
                }
                
                let model_slice = &device.model[0..model_end];
                let model_str = core::str::from_utf8(model_slice)
                    .unwrap_or("[Invalid UTF-8]")
                    .trim();

                let size_mb = (device.size as u64 * 512) / (1024 * 1024);
                kprintln!("Device {}: channel={}, drive={}, type={}, size={} MB, model='{}'", 
                          i, device.channel, device.drive, device.drive_type, size_mb, model_str);
            } else {
                kprintln!("Device {}: not present", i);
            }
        }

        Ok(())
    }
}

/// Get drive size in bytes with error handling - FIXED
pub fn return_drive_size_bytes(drive: u8) -> IdeResult<u64> {
    unsafe {
        if drive > 3 {
            return Err(IdeError::InvalidParameter);
        }
        
        if IDE_DEVICES[drive as usize].reserved == 0 {
            return Err(IdeError::DriveNotFound);
        }

        let device = &IDE_DEVICES[drive as usize];
        
        // FIXED: Proper size calculation
        if device.drive_type as u8 == IDE_ATA {
            let size_bytes = device.size as u64 * 512;
            kprintln!("Drive {} size: {} sectors = {} bytes", drive, device.size, size_bytes);
            Ok(size_bytes)
        } else {
            // ATAPI devices don't have a meaningful size in this context
            kprintln!("Drive {} is ATAPI - no fixed size", drive);
            Ok(0)
        }
    }
}