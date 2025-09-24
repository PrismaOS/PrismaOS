/// IDE channel register structure containing port addresses
#[derive(Clone, Copy)]
pub struct IDEChannelRegistors {
    pub base: u16,  // IO Base port address
    pub ctrl: u16,  // Control port address
    pub bmide: u16, // Bus Master IDE port address
    pub n_ien: u8,  // nIEN bit (No Interrupt Enable)
}

/// IDE device structure containing device information
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IdeDevice {
    pub reserved: u8,      // 0 = Empty, 1 = Device present
    pub channel: u8,       // Primary (0) or Secondary (1) channel
    pub drive: u8,         // Master (0) or Slave (1) drive
    pub drive_type: u16,   // IDE_ATA or IDE_ATAPI
    pub signature: u16,    // Drive signature from identification
    pub capabilities: u16, // Device capabilities
    pub command_sets: u32, // Supported command sets
    pub size: u32,         // Size in sectors (for ATA) or capacity (for ATAPI)
    pub model: [u8; 41],   // Device model string (40 chars + null terminator)
}

impl IdeDevice {
    /// Create a new empty IDE device structure
    pub const fn new() -> Self {
        IdeDevice {
            reserved: 0,
            channel: 0,
            drive: 0,
            drive_type: 0,
            signature: 0,
            capabilities: 0,
            command_sets: 0,
            size: 0,
            model: [0; 41],
        }
    }
}