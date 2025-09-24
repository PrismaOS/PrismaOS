#![no_std]

use core::arch::asm;
use core::mem::size_of;
use core::ptr;

use crate::serial::write_serial;

/// Number of descriptor slots in the GDT. The TSS descriptor consumes two slots.
const GDT_ENTRIES: usize = 7;

/// Selectors used elsewhere in the kernel.
pub const KERNEL_CODE_SELECTOR: u16 = 0x08;
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;
pub const TSS_SELECTOR: u16 = 0x28;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

impl GdtEntry {
    const fn null() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            base_middle: 0,
            access: 0,
            granularity: 0,
            base_high: 0,
        }
    }
}

#[repr(C, packed)]
struct GdtPtr {
    limit: u16,
    base: u64,
}

/// 64-bit Task State Segment layout (packed to match CPU expectations).
#[repr(C, packed)]
struct Tss {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iopb_offset: u16,
}

impl Tss {
    const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            reserved3: 0,
            iopb_offset: 0,
        }
    }
}

static mut GDT: [GdtEntry; GDT_ENTRIES] = [GdtEntry::null(); GDT_ENTRIES];
static mut GDT_PTR: GdtPtr = GdtPtr { limit: 0, base: 0 };
static mut KERNEL_TSS: Tss = Tss::new();

unsafe fn gdt_set_entry(index: usize, base: u64, limit: u32, access: u8, gran: u8) {
    // Only the low 32-bit base fields are stored in the 8-byte entry.
    // For TSS (16-byte) we'll write a special descriptor.
    GDT[index].base_low = (base & 0xFFFF) as u16;
    GDT[index].base_middle = ((base >> 16) & 0xFF) as u8;
    GDT[index].base_high = ((base >> 24) & 0xFF) as u8;

    GDT[index].limit_low = (limit & 0xFFFF) as u16;
    // granularity low nibble holds top 4 bits of limit
    GDT[index].granularity = (((limit >> 16) & 0x0F) as u8) | (gran & 0xF0);
    GDT[index].access = access;
}

/// Write a 16-byte TSS descriptor occupying entries index and index+1.
unsafe fn gdt_set_tss(index: usize, tss_addr: u64, limit: u32, access: u8, gran: u8) {
    // First 8-byte descriptor (descriptor low)
    GDT[index].limit_low = (limit & 0xFFFF) as u16;
    GDT[index].base_low = (tss_addr & 0xFFFF) as u16;
    GDT[index].base_middle = ((tss_addr >> 16) & 0xFF) as u8;
    GDT[index].access = access;
    GDT[index].granularity = (((limit >> 16) & 0x0F) as u8) | (gran & 0xF0);
    GDT[index].base_high = ((tss_addr >> 24) & 0xFF) as u8;

    // Second 8-byte descriptor (descriptor high) lives at index+1 and must contain
    // the high 32 bits of base and reserved fields. We'll cast the GDT slice into u64 words.
    let entries_bytes = &mut *(GDT.as_mut_ptr() as *mut u8);
    // offset of second entry in bytes:
    let off = (index + 1) * size_of::<GdtEntry>();
    // write the high dword of base into the appropriate location:
    let high_base: u32 = ((tss_addr >> 32) & 0xFFFFFFFF) as u32;
    // write high_base into bytes at offset + 0..4
    let dst = entries_bytes.as_mut_ptr().add(off) as *mut u32;
    ptr::write_unaligned(dst, high_base);
    // zero the remaining bytes of the second descriptor (safest)
    let rest = entries_bytes.as_mut_ptr().add(off + 4) as *mut u32;
    ptr::write_unaligned(rest, 0u32);
}

/// Initialize the GDT entries and the TSS structure.
/// Safe wrapper will perform internal unsafe writes.
pub fn gdt_init() {
    unsafe {
        GDT_PTR.limit = (size_of::<GdtEntry>() * GDT_ENTRIES - 1) as u16;
        GDT_PTR.base = &GDT as *const _ as u64;

        // Null descriptor
        gdt_set_entry(0, 0, 0, 0, 0);

        // Kernel code segment (64-bit), access 0x9A, flags 0xAF (0xA in high nibble plus available bit)
        gdt_set_entry(1, 0, 0xFFFFF, 0x9A, 0xAF);

        // Kernel data segment, access 0x92, flags 0xCF
        gdt_set_entry(2, 0, 0xFFFFF, 0x92, 0xCF);

        // User code segment, access 0xFA, flags 0xAF
        gdt_set_entry(3, 0, 0xFFFFF, 0xFA, 0xAF);

        // User data segment, access 0xF2, flags 0xCF
        gdt_set_entry(4, 0, 0xFFFFF, 0xF2, 0xCF);

        // Task State Segment (TSS)
        // zero the TSS
        let tss_ptr = &mut KERNEL_TSS as *mut Tss as *mut u8;
        ptr::write_bytes(tss_ptr, 0, core::mem::size_of::<Tss>());
        // set minimal fields
        KERNEL_TSS.rsp0 = 0;
        KERNEL_TSS.iopb_offset = core::mem::size_of::<Tss>() as u16;

        // place TSS descriptor at index 5 (uses entries 5 and 6)
        gdt_set_tss(5, &KERNEL_TSS as *const _ as u64, (size_of::<Tss>() - 1) as u32, 0x89, 0x00);
    }
}

/// Load the GDT and update segment registers, then load the TSS.
/// This function uses inline assembly and is unsafe.
pub unsafe fn gdt_load() {
    // Load GDT and update data segment registers, then do far jump to reload CS.
    asm!(
        "lgdt [{gdt_ptr}]",
        // set DS/ES/FS/GS/SS to kernel data selector (0x10)
        "mov ax, {data_sel:x}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",
        // far return to reload CS: push code selector then rip, lretq
        "push {code_sel}",
        "lea rax, [rip + 1f]",
        "push rax",
        "lretq",
        "1:",
        gdt_ptr = in(reg) &GDT_PTR,
        code_sel = const KERNEL_CODE_SELECTOR,
        data_sel = const KERNEL_DATA_SELECTOR,
        out("rax") _,
        options(nostack)
    );

    write_serial("Complete\n");
    write_serial("TSS: loading TSS\n");

    // Load the TSS (selector 0x28)
    asm!(
        "ltr {sel:x}",
        sel = in(reg) TSS_SELECTOR,
        options(nostack)
    );

    write_serial("Complete\n");
}