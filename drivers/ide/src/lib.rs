// TODO: As much of this as possible should be moved to a safe Rust implementation.
#![no_std]

extern crate alloc;

use lib_kernel::api::commands::{inb, outb, insw, outsw};

pub mod consts;
pub mod types;
use consts::*;
use types::{IDEChannelRegistors, IdeDevice};

// Global IDE data structures
static mut IDE_BUF: [u8; 2048] = [0; 2048];           // Buffer for IDE data transfers
static mut IDE_IRQ_INVOKED: u8 = 0;                    // IRQ invocation flag
static mut ATAPI_PACKET: [u8; 12] = [0xA8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]; // ATAPI packet buffer

/// IDE channel configurations for primary and secondary channels
static mut CHANNELS: [IDEChannelRegistors; 2] = [
    IDEChannelRegistors {
        base: 0x1F0, // Primary channel base port
        ctrl: 0x3F6, // Primary channel control port
        bmide: 0,    // Bus master IDE (initialized later)
        n_ien: 0,    // No interrupt enable
    },
    IDEChannelRegistors {
        base: 0x170, // Secondary channel base port
        ctrl: 0x376, // Secondary channel control port
        bmide: 0,    // Bus master IDE (initialized later)
        n_ien: 0,    // No interrupt enable
    },
];

/// Array of detected IDE devices (4 possible: Primary Master/Slave, Secondary Master/Slave)
static mut IDE_DEVICES: [IdeDevice; 4] = [IdeDevice::new(); 4];

/// Simple busy-wait sleep function for timing delays
/// 
/// # Arguments
/// * `ms` - Number of milliseconds to sleep
fn sleep(ms: u32) {
    for _ in 0..(ms * 1000) {
        core::hint::spin_loop();
    }
}

/* ============================================================================
 * LOW-LEVEL PORT I/O FUNCTIONS
 * ============================================================================ */

/// Read a byte from an IDE register
/// 
/// # Arguments
/// * `channel` - IDE channel (0 = primary, 1 = secondary)
/// * `reg` - Register offset to read from
/// 
/// # Returns
/// The byte value read from the register
fn ide_read(channel: u8, reg: u8) -> u8 {
    unsafe {
        // For registers 0x08-0x0B, we need to set the control register first
        if reg > 0x07 && reg < 0x0C {
            ide_write(channel, ATA_REG_CONTROL, CHANNELS[channel as usize].n_ien | 0x02);
        }

        // Calculate the actual port address based on register type
        let port: u16 = if reg < 0x08 {
            // Base registers (0x00-0x07)
            CHANNELS[channel as usize].base + reg as u16
        } else {
            // Control registers (0x08-0x0F)
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };

        inb(port)
    }
}

/// Write a byte to an IDE register
/// 
/// # Arguments
/// * `channel` - IDE channel (0 = primary, 1 = secondary)
/// * `reg` - Register offset to write to
/// * `data` - Byte value to write
fn ide_write(channel: u8, reg: u8, data: u8) {
    unsafe {    
        // For registers 0x08-0x0B, we need to set the control register first
        if reg > 0x07 && reg < 0x0C {
            ide_write(channel, ATA_REG_CONTROL, CHANNELS[channel as usize].n_ien | 0x02);
        }

        // Calculate the actual port address based on register type
        let port: u16 = if reg < 0x08 {
            // Base registers (0x00-0x07)
            CHANNELS[channel as usize].base + reg as u16
        } else {
            // Control registers (0x08-0x0F)
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };
    
        outb(port, data);
    }
}

/// Read multiple words from an IDE register into a buffer
/// 
/// # Arguments
/// * `channel` - IDE channel (0 = primary, 1 = secondary)
/// * `reg` - Register offset to read from
/// * `buffer` - Pointer to buffer to store data
/// * `quads` - Number of 16-bit words to read
fn ide_read_buffer(channel: u8, reg: u8, buffer: *mut u32, quads: u32) {
    unsafe {
        // For registers 0x08-0x0B, we need to set the control register first
        if reg > 0x07 && reg < 0x0C {
            ide_write(channel, ATA_REG_CONTROL,     CHANNELS[channel as usize].n_ien | 0x02);
        }

        // Calculate the actual port address
        let port: u16 = if reg < 0x08 {
            CHANNELS[channel as usize].base + reg as u16
        } else {
            CHANNELS[channel as usize].ctrl + (reg - 0x08) as u16
        };

        // Read the specified number of words
        insw(port, buffer as *mut u16, quads);
    }
}

/* ============================================================================
 * SECTOR READ OPERATIONS
 * ============================================================================ */

/// Read sectors from an IDE drive using PIO mode
/// 
/// # Arguments
/// * `drive` - Drive index (0-3)
/// * `numsects` - Number of sectors to read
/// * `lba` - Logical Block Address to start reading from
/// * `buf` - Buffer to store the read data
/// 
/// # Returns
/// 0 on success, error code on failure
pub fn ide_read_sectors(
    drive: u8, 
    numsects: u8, 
    lba: u32, 
    buf: *mut core::ffi::c_void
) -> u32 {
    unsafe {
        // Validate drive index and ensure device is present
        if drive > 3 || IDE_DEVICES[drive as usize].reserved == 0 {
            return 1; // Invalid drive or no device present
        }

        // Get device information
        let channel: u8 = IDE_DEVICES[drive as usize].channel;
        let slavebit: u8 = IDE_DEVICES[drive as usize].drive;
        let bus: u16 = CHANNELS[channel as usize].base;

        // Prepare LBA addressing
        let mut lba_io: [u8; 6] = [0; 6];
        let head: u8;
        let mut err: u8;

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
        ide_write(channel, ATA_REG_HDDEVSEL, 0xE0 | (slavebit << 4) | head);
        sleep(1); // Small delay for drive selection

        // Set up the read operation
        ide_write(channel, ATA_REG_SECCOUNT0, numsects);
        ide_write(channel, ATA_REG_LBA0, lba_io[0]);
        ide_write(channel, ATA_REG_LBA1, lba_io[1]);
        ide_write(channel, ATA_REG_LBA2, lba_io[2]);
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_READ_PIO);

        // Read each sector
        for i in 0..numsects {
            // Wait for the drive to be ready
            err = ide_polling(channel, true);
            if err != 0 {
                return err as u32; // Return error code
            }
            
            // Read 512 bytes (256 words) from the data port
            insw(
                bus,
                (buf as *mut u8).add(i as usize * 512) as *mut u16,
                256,
            );
        }

        0 // Success
    }
}

/* ============================================================================
 * SECTOR WRITE OPERATIONS
 * ============================================================================ */

/// Write sectors to an IDE drive using PIO mode
/// 
/// # Arguments
/// * `drive` - Drive index (0-3)
/// * `numsects` - Number of sectors to write
/// * `lba` - Logical Block Address to start writing to
/// * `buf` - Buffer containing data to write
/// 
/// # Returns
/// 0 on success, error code on failure
pub fn ide_write_sectors(
    drive: u8,
    numsects: u8,
    lba: u32,
    buf: *const core::ffi::c_void
) -> u32 {
    unsafe {
        // Validate drive index and ensure device is present
        if drive > 3 || IDE_DEVICES[drive as usize].reserved == 0 {
            return 1; // Invalid drive or no device present
        }

        // Get device information
        let channel: u8 = IDE_DEVICES[drive as usize].channel;
        let slavebit: u8 = IDE_DEVICES[drive as usize].drive;
        let bus: u16 = CHANNELS[channel as usize].base;
        let mut lba_io: [u8; 6] = [0; 6];
        let head: u8;
        let mut err: u8;

        // We only support 28-bit LBA for writes in this implementation
        if lba >= 0x10000000 {
            return 2; // LBA out of supported range for write operations
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
        ide_write(channel, ATA_REG_HDDEVSEL, 0xE0 | (slavebit << 4) | head);
        sleep(1); // Small delay for drive selection

        // Set up the write operation
        ide_write(channel, ATA_REG_SECCOUNT0, numsects);
        ide_write(channel, ATA_REG_LBA0, lba_io[0]);
        ide_write(channel, ATA_REG_LBA1, lba_io[1]);
        ide_write(channel, ATA_REG_LBA2, lba_io[2]);
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_WRITE_PIO);

        // Write each sector
        for i in 0..numsects {
            // Wait for the drive to be ready for data
            err = ide_polling(channel, false);
            if err != 0 {
                return err as u32; // Return error code
            }
            
            // Write 512 bytes (256 words) to the data port
            outsw(
                bus,
                (buf as *const u8).add(i as usize * 512) as *const u16,
                256,
            );
        }

        // Flush the write cache to ensure data is written to disk
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_CACHE_FLUSH);
        err = ide_polling(channel, false);
        if err != 0 {
            return err as u32; // Return error code from cache flush
        }

        0 // Success
    }
}

/* ============================================================================
 * DEVICE IDENTIFICATION
 * ============================================================================ */

/// Identify and initialize an IDE device
/// 
/// # Arguments
/// * `channel` - IDE channel (0 = primary, 1 = secondary)
/// * `drive` - Drive on the channel (0 = master, 1 = slave)
fn ide_identify(channel: u8, drive: u8) {
    unsafe {
        let mut status: u8;

        let bus = CHANNELS[channel as usize].base;

        // Select the drive
        ide_write(channel, ATA_REG_HDDEVSEL, 0xA0 | (drive << 4));
        sleep(1); // Wait for drive selection

        // Send the IDENTIFY command
        ide_write(channel, ATA_REG_COMMAND, ATA_CMD_IDENTIFY);
        sleep(1); // Wait for command to be processed

        // Check if drive exists
        status = ide_read(channel, ATA_REG_STATUS);
        if status == 0 {
            // No drive present
            IDE_DEVICES[(channel as usize) * 2 + drive as usize].reserved = 0;
            return;
        }

        // Wait for the drive to finish processing
        while {
            status = ide_read(channel, ATA_REG_STATUS);
            (status & ATA_SR_BSY) != 0
        } {}

        // Check for errors and determine device type
        let err = ide_read(channel, ATA_REG_ERROR);
        let type_ = if err != 0 {
            // Error indicates this is likely an ATAPI device
            ide_write(channel, ATA_REG_COMMAND, ATA_CMD_IDENTIFY_PACKET);
            sleep(1);
            IDE_ATAPI
        } else {
            // No error, this is a standard ATA device
            IDE_ATA
        };

        // Initialize device structure
        let device_index = (channel as usize) * 2 + drive as usize;
        IDE_DEVICES[device_index].reserved = 1;
        IDE_DEVICES[device_index].channel = channel;
        IDE_DEVICES[device_index].drive = drive;
        IDE_DEVICES[device_index].drive_type = type_ as u16;

        if type_ == IDE_ATA {
            // Read identification data for ATA device
            let buf_ptr = core::ptr::addr_of_mut!(IDE_BUF).cast::<u8>();
            insw(bus, buf_ptr as *mut u16, 256);

            // Extract device information from identification data
            let buf_ptr_u16 = buf_ptr as *const u16;
            let buf_ptr_u32 = buf_ptr as *const u32;
            
            IDE_DEVICES[device_index].signature = *buf_ptr_u16.add(0);
            IDE_DEVICES[device_index].capabilities = *buf_ptr_u16.add(49);
            IDE_DEVICES[device_index].command_sets = *buf_ptr_u32.add(83);
            IDE_DEVICES[device_index].size = *buf_ptr_u32.add(60);
            
            // Copy model string (40 characters)
            for i in 0..40 {
                IDE_DEVICES[device_index].model[i] = *buf_ptr.add(ATA_IDENT_MODEL as usize * 2 + i);
            }
            IDE_DEVICES[device_index].model[40] = 0; // Null terminator
            
        } else if type_ == IDE_ATAPI {
            // Read identification data for ATAPI device
            let buf_ptr = core::ptr::addr_of_mut!(IDE_BUF).cast::<u8>();
            insw(bus, buf_ptr as *mut u16, 256);
            
            // ATAPI devices have different size calculation
            let buf_ptr_u16 = buf_ptr as *const u16;
            IDE_DEVICES[device_index].size = (*buf_ptr_u16.add(60) as u32) << 16 | (*buf_ptr_u16.add(61) as u32);
            
            // Copy model string (40 characters)
            for i in 0..40 {
                IDE_DEVICES[device_index].model[i] = *buf_ptr.add(ATA_IDENT_MODEL as usize * 2 + i);
            }
            IDE_DEVICES[device_index].model[40] = 0; // Null terminator
        }
    }
}

/* ============================================================================
 * DEVICE POLLING AND STATUS CHECKING
 * ============================================================================ */

/// Poll IDE device status and wait for readiness
/// 
/// # Arguments
/// * `channel` - IDE channel to poll
/// * `advanced_check` - Whether to perform additional error checking
/// 
/// # Returns
/// 0 on success, error code on failure
fn ide_polling(channel: u8, advanced_check: bool) -> u8 {
    // Delay 400ns by reading Alternate Status Register 4 times
    // This is required by the ATA specification
    for _ in 0..4 {
        ide_read(channel, ATA_REG_ALTSTATUS);
    }

    // Wait for the BSY (Busy) bit to clear
    while (ide_read(channel, ATA_REG_STATUS) & ATA_SR_BSY) != 0 {}

    if advanced_check {
        let state = ide_read(channel, ATA_REG_STATUS);

        // Check for error conditions
        if (state & ATA_SR_ERR) != 0 {
            return 2; // Error bit set
        }

        if (state & ATA_SR_DF) != 0 {
            return 1; // Drive fault
        }

        if (state & ATA_SR_DRQ) == 0 {
            return 3; // Data request not ready
        }
    }

    0 // Success - device is ready
}

/* ============================================================================
 * INITIALIZATION AND DEVICE DETECTION
 * ============================================================================ */

/// Initialize the IDE subsystem and detect all connected devices
/// 
/// This function:
/// 1. Disables IRQs for both IDE channels
/// 2. Clears the device table
/// 3. Attempts to identify devices on all 4 possible positions
/// 4. Prints information about detected devices
pub fn ide_initialize() {
    unsafe {
        // Disable IRQs for both IDE channels
        // We use polling mode instead of interrupt-driven I/O
        // Note: We use indexed loops instead of iterators to avoid creating references to mutable statics
        #[allow(clippy::needless_range_loop)]
        for i in 0..2 {
            CHANNELS[i].n_ien = 0x02;
        }

        // Clear all device entries to mark them as empty
        // Note: We use indexed loops instead of iterators to avoid creating references to mutable statics
        #[allow(clippy::needless_range_loop)]
        for i in 0..4 {
            IDE_DEVICES[i].reserved = 0;
        }

        // Attempt to detect devices on all channels and drives
        // 4 possible devices: Primary Master/Slave, Secondary Master/Slave
        for i in 0..4 {
            let channel = (i / 2) as u8;  // 0,1,2,3 -> 0,0,1,1
            let drive = (i % 2) as u8;    // 0,1,2,3 -> 0,1,0,1
            ide_identify(channel, drive);
        }

        let mut size_bytes: u64 = 0;

        // Print information about all detected devices
        // Note: We use indexed loops instead of iterators to avoid creating references to mutable statics
        #[allow(clippy::needless_range_loop)]
        for i in 0..4 {
            if IDE_DEVICES[i].reserved != 0 {
                // Convert model bytes to string, handling potential UTF-8 issues
                let _model_str = core::str::from_utf8(&IDE_DEVICES[i].model)
                    .unwrap_or("[Invalid UTF-8]")
                    .trim_end_matches(char::from(0));

                // Convert size from sectors to GB
                let _size_gb = IDE_DEVICES[i].size as f32 / 2_097_152.0;
            
                // Convert size from sectors to bytes
                size_bytes = IDE_DEVICES[i].size as u64 * 512;
            }
        }
    }
}

pub fn return_ide_size_bytes(drive: u8) -> u64 {
    let size = unsafe { IDE_DEVICES[drive as usize].size as u64 * 512};

    size
}