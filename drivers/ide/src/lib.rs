// lib.rs - IDE Driver with proper error handling
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
    for _ in 0..(ms * 1000) {
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

        // Debug drive info
        let device = &IDE_DEVICES[drive as usize];
        kprintln!("Drive {} info: channel={}, drive_select={}, type={}, size={}", 
                  drive, device.channel, device.drive, device.drive_type, device.size);

        let channel: u8 = device.channel;
        let slavebit: u8 = device.drive;
        let bus: u16 = CHANNELS[channel as usize].base;
        
        kprintln!("Using channel {} (base=0x{:04X}), drive_select={}", channel, bus, slavebit);

        // Check if we can communicate with the drive first
        let initial_status = ide_read(channel, ATA_REG_STATUS);
        kprintln!("Initial drive status: 0x{:02X}", initial_status);
        
        if initial_status == 0xFF || initial_status == 0x7F {
            kprintln!("Drive appears to be non-responsive (status=0x{:02X})", initial_status);
            return Err(IdeError::DriveNotFound);
        }

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

        kprintln!("LBA breakdown: [{:02X}, {:02X}, {:02X}, {:02X}], head={:02X}", 
                  lba_io[0], lba_io[1], lba_io[2], lba_io[3], head);

        // Select drive and set LBA head bits
        let drive_select = 0xE0 | (slavebit << 4) | head;
        kprintln!("Writing drive select: 0x{:02X}", drive_select);
        ide_write(channel, ATA_REG_HDDEVSEL, drive_select);
        sleep(1);
        
        // Verify drive selection worked
        let selected = ide_read(channel, ATA_REG_HDDEVSEL);
        kprintln!("Drive select readback: 0x{:02X}", selected);

        // Set up the read operation
        ide_write(channel, ATA_REG_SECCOUNT0, numsects);
        ide_write(channel, ATA_REG_LBA0, lba_io[0]);
        ide_write(channel, ATA_REG_LBA1, lba_io[1]);
        ide_write(channel, ATA_REG_LBA2, lba_io[2]);
        
        kprintln!("Sending READ PIO command (0x{:02X})", ATA_CMD_READ_PIO);
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_READ_PIO);

        // Check status immediately after command
        let cmd_status = ide_read(channel, ATA_REG_STATUS);
        kprintln!("Status after command: 0x{:02X}", cmd_status);

        kprintln!("Read command sent, waiting for sectors...");

        // Read each sector
        for i in 0..numsects {
            kprintln!("Reading sector {} of {}", i + 1, numsects);
            ide_polling(channel, true)?;
            
            insw(
                bus,
                (buf as *mut u8).add(i as usize * 512) as *mut u16,
                256,
            );
        }

        kprintln!("Read operation completed successfully");
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
        if timeout > 100000 {
            return Err(IdeError::Timeout);
        }
    }

    if advanced_check {
        let state = ide_read(channel, ATA_REG_STATUS);

        if (state & ATA_SR_ERR) != 0 {
            // Read the error register to get more details
            let error = ide_read(channel, ATA_REG_ERROR);
            kprintln!("IDE Status Error - Status: 0x{:02X}, Error: 0x{:02X}", state, error);
            kprintln!("Error details: BBK={}, UNC={}, MC={}, IDNF={}, MCR={}, ABRT={}, TK0NF={}, AMNF={}", 
                     (error & ATA_ER_BBK) != 0,
                     (error & ATA_ER_UNC) != 0,
                     (error & ATA_ER_MC) != 0,
                     (error & ATA_ER_IDNF) != 0,
                     (error & ATA_ER_MCR) != 0,
                     (error & ATA_ER_ABRT) != 0,
                     (error & ATA_ER_TK0NF) != 0,
                     (error & ATA_ER_AMNF) != 0);
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
 * DEVICE IDENTIFICATION WITH ERROR HANDLING
 * ============================================================================ */

/// Identify and initialize an IDE device with error handling
fn ide_identify(channel: u8, drive: u8) -> IdeResult<()> {
    unsafe {
        let mut status: u8;
        let bus = CHANNELS[channel as usize].base;

        // Select the drive
        ide_write(channel, ATA_REG_HDDEVSEL, 0xA0 | (drive << 4));
        sleep(1);

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
            if timeout > 100000 {
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
        IDE_DEVICES[device_index].reserved = 1;
        IDE_DEVICES[device_index].channel = channel;
        IDE_DEVICES[device_index].drive = drive;
        IDE_DEVICES[device_index].drive_type = type_ as u16;

        if type_ == IDE_ATA {
            let buf_ptr = core::ptr::addr_of_mut!(IDE_BUF).cast::<u8>();
            insw(bus, buf_ptr as *mut u16, 256);

            let buf_ptr_u16 = buf_ptr as *const u16;
            let buf_ptr_u32 = buf_ptr as *const u32;
            
            IDE_DEVICES[device_index].signature = *buf_ptr_u16.add(0);
            IDE_DEVICES[device_index].capabilities = *buf_ptr_u16.add(49);
            IDE_DEVICES[device_index].command_sets = *buf_ptr_u32.add(83);
            IDE_DEVICES[device_index].size = *buf_ptr_u32.add(60);
            
            for i in 0..40 {
                IDE_DEVICES[device_index].model[i] = *buf_ptr.add(ATA_IDENT_MODEL as usize * 2 + i);
            }
            IDE_DEVICES[device_index].model[40] = 0;
            
        } else if type_ == IDE_ATAPI {
            let buf_ptr = core::ptr::addr_of_mut!(IDE_BUF).cast::<u8>();
            insw(bus, buf_ptr as *mut u16, 256);
            
            let buf_ptr_u16 = buf_ptr as *const u16;
            IDE_DEVICES[device_index].size = (*buf_ptr_u16.add(60) as u32) << 16 | (*buf_ptr_u16.add(61) as u32);
            
            for i in 0..40 {
                IDE_DEVICES[device_index].model[i] = *buf_ptr.add(ATA_IDENT_MODEL as usize * 2 + i);
            }
            IDE_DEVICES[device_index].model[40] = 0;
        }

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
            IDE_DEVICES[i].reserved = 0;
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
                let _model_str = core::str::from_utf8(&device.model)
                    .unwrap_or("[Invalid UTF-8]")
                    .trim_end_matches(char::from(0));

                let _size_gb = device.size as f32 / 2_097_152.0;
                kprintln!("Device {}: channel={}, drive={}, type={}, size={} GB, model='{}'", 
                          i, device.channel, device.drive, device.drive_type, _size_gb, _model_str);
            } else {
                kprintln!("Device {}: not present", i);
            }
        }

        Ok(())
    }
}

/// Get drive size in bytes with error handling
pub fn return_drive_size_bytes(drive: u8) -> IdeResult<u64> {
    unsafe {
        if drive > 3 {
            return Err(IdeError::InvalidParameter);
        }
        
        if IDE_DEVICES[drive as usize].reserved == 0 {
            return Err(IdeError::DriveNotFound);
        }

        let size = IDE_DEVICES[drive as usize].size as u64 * 512;
        kprintln!("size: {}", size);
        Ok(size)
    }
}