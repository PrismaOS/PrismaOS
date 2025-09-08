#![no_std]
#![feature(asm_const)]
use core::arch::asm;
use core::mem::{size_of, zeroed};

/// ----- Segment Selectors (GDT indices * 8) -----
pub const KERNEL_CS: u16 = 0x08; // index 1
pub const KERNEL_DS: u16 = 0x10; // index 2
pub const USER_CS:   u16 = 0x18; // index 3
pub const USER_DS:   u16 = 0x20; // index 4
pub const TSS_SEL:   u16 = 0x28; // index 5 (spans two entries: 5/6)

/// ----- 64-bit TSS Layout -----
#[repr(C, packed)]
pub struct Tss {
    pub reserved0:  u32,
    pub rsp0:       u64,
    pub rsp1:       u64,
    pub rsp2:       u64,
    pub reserved1:  u64,
    pub ist1:       u64,
    pub ist2:       u64,
    pub ist3:       u64,
    pub ist4:       u64,
    pub ist5:       u64,
    pub ist6:       u64,
    pub ist7:       u64,
    pub reserved2:  u64,
    pub reserved3:  u16,
    pub iopb_offset:u16,
}

/// Standard 8-byte code/data descriptor
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct GdtEntry {
    limit_low:     u16,
    base_low:      u16,
    base_mid:      u8,
    access:        u8,
    granularity:   u8,
    base_high:     u8,
}

/// 16-byte TSS descriptor
#[repr(C, packed)]
struct TssDescriptor {
    limit_low:   u16,
    base_low:    u16,
    base_mid:    u8,
    access:      u8,
    granularity: u8,
    base_high:   u8,
    base_upper:  u32,
    reserved:    u32,
}

/// GDTR pointer
#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base:  u64,
}

/// Our final GDT layout: 5 x 8-byte entries + 16-byte TSS
#[repr(C, packed)]
struct Gdt {
    null:        GdtEntry,     // 0
    kcode:       GdtEntry,     // 1
    kdata:       GdtEntry,     // 2
    ucode:       GdtEntry,     // 3
    udata:       GdtEntry,     // 4
    tss:         TssDescriptor // 5/6
}

// -------------------- Static storage (no allocator) --------------------

// 16-byte alignment is nice-to-have for stacks/structures
#[repr(align(16))]
struct Aligned<T>(T);

static mut TSS: Aligned<Tss> = Aligned(Tss {
    reserved0: 0,
    rsp0: 0, rsp1: 0, rsp2: 0,
    reserved1: 0,
    ist1: 0, ist2: 0, ist3: 0, ist4: 0, ist5: 0, ist6: 0, ist7: 0,
    reserved2: 0,
    reserved3: 0,
    iopb_offset: 0,
});

// Kernel RSP0 stack (for syscalls/interrupts returning to ring 0)
static mut KERNEL_STACK: Aligned<[u8; 32 * 1024]> = Aligned([0; 32 * 1024]);
// IST1 stack (good default for double-fault handler)
static mut IST1_STACK:  Aligned<[u8; 8 * 1024]>  = Aligned([0; 8 * 1024]);

static mut GDT: Aligned<Gdt> = Aligned(Gdt {
    null:  GdtEntry { limit_low:0, base_low:0, base_mid:0, access:0, granularity:0, base_high:0 },
    kcode: GdtEntry { limit_low:0, base_low:0, base_mid:0, access:0, granularity:0, base_high:0 },
    kdata: GdtEntry { limit_low:0, base_low:0, base_mid:0, access:0, granularity:0, base_high:0 },
    ucode: GdtEntry { limit_low:0, base_low:0, base_mid:0, access:0, granularity:0, base_high:0 },
    udata: GdtEntry { limit_low:0, base_low:0, base_mid:0, access:0, granularity:0, base_high:0 },
    tss: TssDescriptor {
        limit_low:0, base_low:0, base_mid:0, access:0, granularity:0, base_high:0,
        base_upper:0, reserved:0
    },
});

static mut GDTR: Gdtr = Gdtr { limit: 0, base: 0 };

// -------------------- Helpers to build descriptors --------------------

/// Build a standard 64-bit code/data descriptor.
/// Note: In long mode, base/limit are largely ignored for code/data; we still fill canonical values.
const fn make_seg_descriptor(access: u8, flags: u8) -> GdtEntry {
    // We use base=0, limit=0xFFFFF (for historical consistency), granularity=4K
    let limit: u32 = 0x000F_FFFF; // 20-bit max (with granularity)
    let base:  u32 = 0;

    GdtEntry {
        limit_low:   (limit & 0xFFFF) as u16,
        base_low:    (base & 0xFFFF) as u16,
        base_mid:    ((base >> 16) & 0xFF) as u8,
        access, // includes present, type, DPL
        granularity: (((limit >> 16) & 0x0F) as u8) | (flags & 0xF0),
        base_high:   ((base >> 24) & 0xFF) as u8,
    }
}

/// Fill the 16-byte TSS descriptor from a TSS pointer
unsafe fn fill_tss_descriptor(desc: &mut TssDescriptor, tss_ptr: *const Tss) {
    let base = tss_ptr as u64;
    let limit = (size_of::<Tss>() - 1) as u32;

    desc.limit_low   = (limit & 0xFFFF) as u16;
    desc.base_low    = (base & 0xFFFF) as u16;
    desc.base_mid    = ((base >> 16) & 0xFF) as u8;
    desc.access      = 0x89; // Present | Type=0b1001 (Available 64-bit TSS)
    desc.granularity = ((limit >> 16) & 0x0F) as u8;
    desc.base_high   = ((base >> 24) & 0xFF) as u8;
    desc.base_upper  = (base >> 32) as u32;
    desc.reserved    = 0;
}

// -------------------- Public init --------------------

pub unsafe fn init() {
    // 1) Prepare TSS (zero + stacks)
    // Zero is already done via static init; ensure IOPB after TSS (disabled)
    TSS.0.iopb_offset = size_of::<Tss>() as u16;

    // RSP0 = top of kernel stack
    let kstack_top = (KERNEL_STACK.0.as_ptr().add(KERNEL_STACK.0.len())) as u64;
    TSS.0.rsp0 = kstack_top;

    // IST1 = top of ist1 stack (e.g., use for double-fault)
    let ist1_top = (IST1_STACK.0.as_ptr().add(IST1_STACK.0.len())) as u64;
    TSS.0.ist1 = ist1_top;

    // 2) Build GDT entries
    // Access bytes:
    //  Code: 0x9A = P|S|Executable|Readable (DPL=0)
    //  Data: 0x92 = P|S|Writable (DPL=0)
    //  User Code: 0xFA = P|S|Executable|Readable (DPL=3)
    //  User Data: 0xF2 = P|S|Writable (DPL=3)
    //
    // Flags: set Granularity=1 (4K), Long=1 for code, Size=0 in long mode
    //   0xA0 = L bit; 0x80 = G bit -> 0xA0 | 0x80 = 0xA0 + 0x80 but mask is high nibble (0xF0)
    // For data, L must be 0. We still keep G=1 (0xC0 in granularity's high nibble).
    GDT.0.null  = make_seg_descriptor(0x00, 0x00);

    // Kernel code: Long=1, G=1
    GDT.0.kcode = make_seg_descriptor(0x9A, 0xA0 | 0x80); // 0xA0 (L) + 0x80 (G) -> high nibble 0xE0 maps properly via mask
    // Kernel data: L=0, G=1
    GDT.0.kdata = make_seg_descriptor(0x92, 0x80);

    // User code: DPL=3, Long=1, G=1
    GDT.0.ucode = make_seg_descriptor(0xFA, 0xA0 | 0x80);
    // User data: DPL=3, L=0, G=1
    GDT.0.udata = make_seg_descriptor(0xF2, 0x80);

    // TSS descriptor (16 bytes)
    fill_tss_descriptor(&mut GDT.0.tss, &TSS.0 as *const Tss);

    // 3) Load GDTR
    GDTR.limit = (size_of::<Gdt>() - 1) as u16;
    GDTR.base  = (&GDT.0 as *const Gdt) as u64;

    asm!(
        "lgdt [{gdtr}]",
        gdtr = in(reg) &GDTR,
        options(nostack, preserves_flags)
    );

    // 4) Reload segments: data first, then far-jump to reload CS
    load_segments_and_cs();

    // 5) Load TSS
    asm!(
        "ltr {sel:x}",
        sel = in(reg) TSS_SEL,
        options(nostack, preserves_flags)
    );
}

/// Separate tiny function just to keep `init()` readable.
unsafe fn load_segments_and_cs() {
    // Write data segments = kernel data selector
    let ds = KERNEL_DS;
    asm!(
        "mov ds, {0:x}",
        "mov es, {0:x}",
        "mov fs, {0:x}",
        "mov gs, {0:x}",
        "mov ss, {0:x}",
        in(reg) ds,
        options(nostack, preserves_flags)
    );

    // Far return to load CS = kernel code selector
    asm!(
        "push {cs}",
        "lea rax, [rip + 1f]",
        "push rax",
        "lretq",
        "1:",
        cs = const KERNEL_CS as u64,
        out("rax") _,
        options(nostack, preserves_flags)
    );
}

#[allow(dead_code)]
pub fn kernel_stack_top() -> u64 {
    unsafe { (KERNEL_STACK.0.as_ptr().add(KERNEL_STACK.0.len())) as u64 }
}

#[allow(dead_code)]
pub fn ist1_top() -> u64 {
    unsafe { (IST1_STACK.0.as_ptr().add(IST1_STACK.0.len())) as u64 }
}
