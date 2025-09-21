// capability registers
pub const CAP_LENGTH: u8 = 0x00;   // 8-bit
pub const RSVD: u8 = 0x01;         // 8-bit
pub const HCIVERSION: u16 = 0x02;  // 16-bit
pub const HCSPARAMS1: u32 = 0x04;  // 32-bit
pub const HCSPARAMS2: u32 = 0x08;  // 32-bit
pub const HCSPARAMS3: u32 = 0x0C;  // 32-bit
pub const HCCPARAMS1: u32 = 0x10;  // 32-bit
pub const DBOFF: u32 = 0x14;       // 32-bit
pub const RTSOFF: u32 = 0x18;      // 32-bit
pub const HCCPARAMS2: u32 = 0x1C;  // 32-bit

// operational registers
pub const USB_CMD: u32 = 0x00;
pub const USB_STS: u32 = 0x04;
pub const PAGESIZE: u32 = 0x08;
pub const DNCTRL: u32 = 0x14;
pub const CRCR: u64 = 0x18;        // 64-bit (command ring pointer)
pub const DCBAAP: u64 = 0x30;      // 64-bit (device context base address)
pub const CONFIG: u32 = 0x38;

// port registers (per port, offset by port number * 0x10)
pub const PORTSC: u32 = 0x00;
pub const PORTPMSC: u32 = 0x04;
pub const PORTLI: u32 = 0x08;
pub const PORTHLPMC: u32 = 0x0C;

// runtime registers
pub const MFINDEX: u32 = 0x00;

// interrupter registers (per interrupter, offset by interrupter number * 0x20)
pub const IMAN: u32 = 0x20;    // 32-bit
pub const IMOD: u32 = 0x24;    // 32-bit
pub const ERSTSZ: u32 = 0x28;  // 32-bit
pub const ERSTBA: u64 = 0x30;  // 64-bit (base address)
pub const ERDP: u64 = 0x38;    // 64-bit (pointer)

// doorbell registers (per slot, offset by slot number * 0x04)
pub const DOORBELL: u32 = 0x00;