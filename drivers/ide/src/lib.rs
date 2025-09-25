// lib.rs - High-Performance IDE Driver  
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
    /// Buffer size insufficient
    BufferTooSmall,
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

// Optimized minimal delay
#[inline(always)]
fn sleep(ms: u32) {
    for _ in 0..(ms * 1000) {  // Reduced delay multiplier for speed
        core::hint::spin_loop();
    }
}

/* ============================================================================
 * OPTIMIZED LOW-LEVEL PORT I/O FUNCTIONS
 * ============================================================================ */

#[inline(always)]
fn ide_read(channel: u8, reg: u8) -> u8 {
    unsafe {
        // Skip control register access optimization for speed
        let port: u16 = if reg < 0x08 {
            CHANNELS[channel as usize].base + reg as u16
        } else {
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };
        inb(port)
    }
}

#[inline(always)]
fn ide_write(channel: u8, reg: u8, data: u8) {
    unsafe {
        // Skip control register access optimization for speed
        let port: u16 = if reg < 0x08 {
            CHANNELS[channel as usize].base + reg as u16
        } else {
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };
        outb(port, data);
    }
}

fn ide_read_buffer(channel: u8, reg: u8, buffer: &mut [u8]) {
    unsafe {
        let port: u16 = if reg < 0x08 {
            CHANNELS[channel as usize].base + reg as u16
        } else {
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };

        // Read words (16-bit) so we divide by 2
        let words = buffer.len() / 2;
        insw(port, buffer.as_mut_ptr() as *mut u16, words as u32);
    }
}

/* ============================================================================
 * OPTIMIZED SECTOR READ OPERATIONS
 * ============================================================================ */

/// High-performance sector reading with bulk transfers and minimal polling
pub fn ide_read_sectors(
    drive: u8,
    numsects: u8,
    lba: u32,
    buf: &mut [u8]
) -> IdeResult<()> {
    unsafe {
        // Fast parameter validation
        if drive > 3 {
            return Err(IdeError::InvalidParameter);
        }
        
        if IDE_DEVICES[drive as usize].reserved == 0 {
            return Err(IdeError::DriveNotFound);
        }

        // Check buffer size
        let required_size = numsects as usize * 512;
        if buf.len() < required_size {
            return Err(IdeError::BufferTooSmall);
        }

        let device = &IDE_DEVICES[drive as usize];
        let channel: u8 = device.channel;
        let slavebit: u8 = device.drive;
        let bus: u16 = CHANNELS[channel as usize].base;
        
        // Optimized LBA setup
        let head: u8 = if lba >= 0x10000000 { 0 } else { ((lba >> 24) & 0x0F) as u8 };

        // Fast drive selection
        ide_write(channel, ATA_REG_HDDEVSEL, 0xE0 | (slavebit << 4) | head);

        // Batch register writes for efficiency  
        ide_write(channel, ATA_REG_SECCOUNT0, numsects);
        ide_write(channel, ATA_REG_LBA0, lba as u8);
        ide_write(channel, ATA_REG_LBA1, (lba >> 8) as u8);
        ide_write(channel, ATA_REG_LBA2, (lba >> 16) as u8);
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_READ_PIO);

        // Single bulk transfer for all sectors - MAXIMUM PERFORMANCE
        ide_polling(channel, true)?;
        
        // Read all sectors in one bulk operation
        let total_words = (numsects as u32) * 256; // 256 words per sector
        insw(bus, buf.as_mut_ptr() as *mut u16, total_words);

        Ok(())
    }
}

/* ============================================================================
 * OPTIMIZED SECTOR WRITE OPERATIONS
 * ============================================================================ */

/// High-performance sector writing with bulk transfers
pub fn ide_write_sectors(
    drive: u8,
    numsects: u8,
    lba: u32,
    buf: &[u8]
) -> IdeResult<()> {
    unsafe {
        // Fast validation
        if drive > 3 {
            return Err(IdeError::InvalidParameter);
        }
        
        if IDE_DEVICES[drive as usize].reserved == 0 {
            return Err(IdeError::DriveNotFound);
        }

        // Check buffer size
        let required_size = numsects as usize * 512;
        if buf.len() < required_size {
            return Err(IdeError::BufferTooSmall);
        }

        let channel: u8 = IDE_DEVICES[drive as usize].channel;
        let slavebit: u8 = IDE_DEVICES[drive as usize].drive;
        let bus: u16 = CHANNELS[channel as usize].base;

        // Only support 28-bit LBA for writes (optimization)
        if lba >= 0x10000000 {
            return Err(IdeError::LbaOutOfRange);
        }

        let head = ((lba >> 24) & 0x0F) as u8;

        // Fast setup
        ide_write(channel, ATA_REG_HDDEVSEL, 0xE0 | (slavebit << 4) | head);

        // Batch register writes
        ide_write(channel, ATA_REG_SECCOUNT0, numsects);
        ide_write(channel, ATA_REG_LBA0, lba as u8);
        ide_write(channel, ATA_REG_LBA1, (lba >> 8) as u8);
        ide_write(channel, ATA_REG_LBA2, (lba >> 16) as u8);
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_WRITE_PIO);

        // Single bulk transfer for all sectors - MAXIMUM PERFORMANCE
        ide_polling(channel, false)?;
        
        // Write all sectors in one bulk operation
        let total_words = (numsects as u32) * 256; // 256 words per sector
        outsw(bus, buf.as_ptr() as *const u16, total_words);

        // Fast cache flush
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_CACHE_FLUSH);
        ide_polling(channel, false)?;

        Ok(())
    }
}

/* ============================================================================
 * OPTIMIZED DEVICE POLLING
 * ============================================================================ */

/// Ultra-fast polling with minimal overhead
#[inline(always)]
fn ide_polling(channel: u8, advanced_check: bool) -> IdeResult<()> {
    // Minimal 400ns delay - read alternate status once
    ide_read(channel, ATA_REG_ALTSTATUS);

    // Optimized busy wait with reduced timeout for speed
    let mut timeout = 0u32;
    loop {
        let status = ide_read(channel, ATA_REG_STATUS);
        if (status & ATA_SR_BSY) == 0 {
            if advanced_check {
                // Fast error checking in single pass
                if (status & ATA_SR_ERR) != 0 {
                    return Err(IdeError::StatusError);
                }
                if (status & ATA_SR_DF) != 0 {
                    return Err(IdeError::DriveFault);
                }
                if (status & ATA_SR_DRQ) == 0 {
                    return Err(IdeError::DataNotReady);
                }
            }
            break;
        }
        timeout += 1;
        if timeout > 100000 {  // Reduced timeout for speed
            return Err(IdeError::Timeout);
        }
    }

    Ok(())
}

/* ============================================================================
 * DEVICE IDENTIFICATION WITH OPTIMIZATIONS
 * ============================================================================ */

/// Fast device identification with reduced overhead
fn ide_identify(channel: u8, drive: u8) -> IdeResult<()> {
    unsafe {
        let mut status: u8;

        // Fast drive selection
        ide_write(channel, ATA_REG_HDDEVSEL, 0xA0 | (drive << 4));
        sleep(1);  // Minimal delay

        // Send IDENTIFY command
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_IDENTIFY);

        // Fast existence check
        status = ide_read(channel, ATA_REG_STATUS);
        if status == 0 {
            IDE_DEVICES[(channel as usize) * 2 + drive as usize].reserved = 0;
            return Err(IdeError::DriveNotFound);
        }

        // Optimized busy wait
        let mut timeout = 0u32;
        loop {
            status = ide_read(channel, ATA_REG_STATUS);
            if (status & ATA_SR_BSY) == 0 {
                break;
            }
            timeout += 1;
            if timeout > 100000 {  // Reduced timeout
                return Err(IdeError::Timeout);
            }
        }

        // Fast device type detection
        let err = ide_read(channel, ATA_REG_ERROR);
        let type_ = if err != 0 {
            ide_write(channel, ATA_REG_COMMAND, ATA_CMD_IDENTIFY_PACKET);
            sleep(1);
            IDE_ATAPI
        } else {
            IDE_ATA
        };

        // Initialize device structure efficiently
        let device_index = (channel as usize) * 2 + drive as usize;
        
        IDE_DEVICES[device_index] = IdeDevice::new();
        IDE_DEVICES[device_index].reserved = 1;
        IDE_DEVICES[device_index].channel = channel;
        IDE_DEVICES[device_index].drive = drive;
        IDE_DEVICES[device_index].drive_type = type_ as u16;

        // Fast buffer clear and read
        for i in 0..512 { IDE_BUF[i] = 0; }
        ide_read_buffer(channel, ATA_REG_DATA, &mut IDE_BUF[..512]);

        let buf_ptr_u16 = IDE_BUF.as_ptr() as *const u16;
        
        if type_ == IDE_ATA {
            // Optimized ATA device setup
            IDE_DEVICES[device_index].signature = *buf_ptr_u16.add(0);
            IDE_DEVICES[device_index].capabilities = *buf_ptr_u16.add(49);
            
            // Fast command set reading
            let cmd_set_low = *buf_ptr_u16.add(82) as u32;
            let cmd_set_high = *buf_ptr_u16.add(83) as u32;
            IDE_DEVICES[device_index].command_sets = cmd_set_low | (cmd_set_high << 16);
            
            // Efficient size calculation
            let size_low = *buf_ptr_u16.add(60) as u32;
            let size_high = *buf_ptr_u16.add(61) as u32;
            let sectors_28bit = size_low | (size_high << 16);
            
            // Check for 48-bit LBA support
            let supports_48bit = (IDE_DEVICES[device_index].command_sets & (1 << 26)) != 0;
            
            if supports_48bit && sectors_28bit == 0xFFFFFFFF {
                let lba48_0 = *buf_ptr_u16.add(100) as u64;
                let lba48_1 = *buf_ptr_u16.add(101) as u64;
                let lba48_2 = *buf_ptr_u16.add(102) as u64;
                let lba48_3 = *buf_ptr_u16.add(103) as u64;
                let sectors_48bit = lba48_0 | (lba48_1 << 16) | (lba48_2 << 32) | (lba48_3 << 48);
                IDE_DEVICES[device_index].size = sectors_48bit as u32;
            } else {
                IDE_DEVICES[device_index].size = sectors_28bit;
            }
            
            // Fast model string extraction
            for i in 0..20 {
                let word = *buf_ptr_u16.add(ATA_IDENT_MODEL as usize / 2 + i);
                IDE_DEVICES[device_index].model[i * 2] = (word >> 8) as u8;
                IDE_DEVICES[device_index].model[i * 2 + 1] = (word & 0xFF) as u8;
            }
            IDE_DEVICES[device_index].model[40] = 0;
            
        } else if type_ == IDE_ATAPI {
            // Fast ATAPI setup
            IDE_DEVICES[device_index].signature = *buf_ptr_u16.add(0);
            IDE_DEVICES[device_index].capabilities = *buf_ptr_u16.add(49);
            IDE_DEVICES[device_index].size = 0;
            
            // Fast model string for ATAPI
            for i in 0..20 {
                let word = *buf_ptr_u16.add(ATA_IDENT_MODEL as usize / 2 + i);
                IDE_DEVICES[device_index].model[i * 2] = (word >> 8) as u8;
                IDE_DEVICES[device_index].model[i * 2 + 1] = (word & 0xFF) as u8;
            }
            IDE_DEVICES[device_index].model[40] = 0;
        }

        Ok(())
    }
}

/* ============================================================================
 * FAST INITIALIZATION
 * ============================================================================ */

pub fn ide_initialize() -> IdeResult<()> {
    unsafe {
        kprintln!("Starting fast IDE initialization...");
        
        // Fast channel setup
        for i in 0..2 {
            CHANNELS[i].n_ien = 0x02;
        }

        // Fast device initialization
        for i in 0..4 {
            IDE_DEVICES[i] = IdeDevice::new();
        }

        // Fast device identification
        for i in 0..4 {
            let channel = (i / 2) as u8;
            let drive = (i % 2) as u8;
            
            match ide_identify(channel, drive) {
                Ok(()) => {},  // Skip verbose logging for speed
                Err(_) => {},  // Skip error logging for speed
            }
        }

        kprintln!("Fast IDE initialization complete. Active devices:");
        
        // Quick device summary
        for i in 0..4 {
            if IDE_DEVICES[i].reserved != 0 {
                let device = &IDE_DEVICES[i];
                let size_mb = (device.size as u64 * 512) / (1024 * 1024);
                kprintln!("Device {}: {} MB", i, size_mb);
            }
        }

        Ok(())
    }
}

/// Fast drive size retrieval
pub fn return_drive_size_bytes(drive: u8) -> IdeResult<u64> {
    unsafe {
        if drive > 3 || IDE_DEVICES[drive as usize].reserved == 0 {
            return Err(IdeError::DriveNotFound);
        }

        let device = &IDE_DEVICES[drive as usize];
        
        if device.drive_type as u8 == IDE_ATA {
            Ok(device.size as u64 * 512)
        } else {
            Ok(0)
        }
    }
}