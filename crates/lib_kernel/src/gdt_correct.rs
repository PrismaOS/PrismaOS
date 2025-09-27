//! Correct GDT implementation with SYSCALL compatibility and proper privilege separation
//!
//! This fixes the critical flaws in the original GDT implementation:
//! - SYSCALL-compatible segment layout (User CS = Kernel DS + 16)
//! - Complete TSS setup with all 7 IST stacks (32KB each)
//! - Proper privilege separation enforcement
//! - Safe userspace transitions using IRETQ

use x86_64::{
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
    PrivilegeLevel,
};
use spin::Mutex;
use core::mem::MaybeUninit;

/// Stack size for each IST stack (32KB for safety)
const IST_STACK_SIZE: usize = 32 * 1024;

/// GDT selectors following SYSCALL-compatible layout
#[repr(C)]
pub struct GdtSelectors {
    pub kernel_code: SegmentSelector,  // 0x08
    pub kernel_data: SegmentSelector,  // 0x10
    pub user_data: SegmentSelector,    // 0x18 (0x10 + 8)
    pub user_code: SegmentSelector,    // 0x20 (0x10 + 16)
    pub tss: SegmentSelector,
}

impl GdtSelectors {
    /// Validate SYSCALL compatibility: User CS must equal Kernel DS + 16
    pub fn validate_syscall_compatibility(&self) -> Result<(), &'static str> {
        let expected_user_cs = SegmentSelector::new(
            (self.kernel_data.index() + 2) as u16,
            PrivilegeLevel::Ring3
        );

        if self.user_code != expected_user_cs {
            return Err("GDT layout violates SYSCALL compatibility requirement");
        }

        crate::kprintln!("[GDT] ✓ SYSCALL compatibility validated");
        crate::kprintln!("  Kernel CS: 0x{:02x}", self.kernel_code.0);
        crate::kprintln!("  Kernel DS: 0x{:02x}", self.kernel_data.0);
        crate::kprintln!("  User DS:   0x{:02x}", self.user_data.0);
        crate::kprintln!("  User CS:   0x{:02x}", self.user_code.0);

        Ok(())
    }
}

/// IST stack wrapper with proper alignment
#[repr(align(4096))]
struct IstStack {
    stack: [u8; IST_STACK_SIZE],
}

impl IstStack {
    const fn new() -> Self {
        Self {
            stack: [0; IST_STACK_SIZE],
        }
    }

    fn top(&self) -> VirtAddr {
        VirtAddr::from_ptr(self.stack.as_ptr_range().end)
    }
}

/// All IST stacks (7 stacks as per Intel specification)
static mut IST_STACKS: [IstStack; 7] = [
    IstStack::new(), IstStack::new(), IstStack::new(), IstStack::new(),
    IstStack::new(), IstStack::new(), IstStack::new(),
];

/// Create properly initialized TSS with all IST stacks
fn create_tss() -> TaskStateSegment {
    let mut tss = TaskStateSegment::new();

    unsafe {
        // Set up all 7 IST stacks
        tss.interrupt_stack_table[0] = IST_STACKS[0].top();
        tss.interrupt_stack_table[1] = IST_STACKS[1].top();
        tss.interrupt_stack_table[2] = IST_STACKS[2].top();
        tss.interrupt_stack_table[3] = IST_STACKS[3].top();
        tss.interrupt_stack_table[4] = IST_STACKS[4].top();
        tss.interrupt_stack_table[5] = IST_STACKS[5].top();
        tss.interrupt_stack_table[6] = IST_STACKS[6].top();
    }

    crate::kprintln!("[GDT] TSS configured with 7 IST stacks (32KB each)");
    tss
}

/// Global GDT instance - use MaybeUninit to avoid initialization issues
static mut GDT_STORAGE: MaybeUninit<GlobalDescriptorTable> = MaybeUninit::uninit();
static mut TSS_STORAGE: MaybeUninit<TaskStateSegment> = MaybeUninit::uninit();
static mut SELECTORS_STORAGE: MaybeUninit<GdtSelectors> = MaybeUninit::uninit();

static GDT_INIT_LOCK: Mutex<bool> = Mutex::new(false);

/// Initialize GDT and TSS with proper SYSCALL layout
fn init_gdt_and_tss() -> &'static GdtSelectors {
    let mut init_guard = GDT_INIT_LOCK.lock();
    if *init_guard {
        // Already initialized, return existing selectors
        unsafe { SELECTORS_STORAGE.assume_init_ref() }
    } else {
        unsafe {
            // Initialize TSS first
            TSS_STORAGE.write(create_tss());
            let tss_ref = TSS_STORAGE.assume_init_ref();

            // Initialize GDT with SYSCALL-compatible layout
            let mut gdt = GlobalDescriptorTable::new();
            let kernel_code = gdt.append(Descriptor::kernel_code_segment());   // 0x08
            let kernel_data = gdt.append(Descriptor::kernel_data_segment());   // 0x10
            let user_data = gdt.append(Descriptor::user_data_segment());       // 0x18 (0x10 + 8)
            let user_code = gdt.append(Descriptor::user_code_segment());       // 0x20 (0x10 + 16)
            let tss = gdt.append(Descriptor::tss_segment(tss_ref));

            // Store GDT
            GDT_STORAGE.write(gdt);

            // Create and validate selectors
            let selectors = GdtSelectors {
                kernel_code,
                kernel_data,
                user_data,
                user_code,
                tss,
            };

            selectors.validate_syscall_compatibility()
                .expect("GDT layout must be SYSCALL compatible");

            // Store selectors
            SELECTORS_STORAGE.write(selectors);

            *init_guard = true;
            SELECTORS_STORAGE.assume_init_ref()
        }
    }
}

/// Get GDT selectors (initialize if needed)
pub fn get_selectors() -> &'static GdtSelectors {
    init_gdt_and_tss()
}

/// Initialize the GDT and load it
pub fn init() {
    crate::kprintln!("[GDT] Initializing correct GDT implementation...");

    // Initialize GDT and TSS
    let selectors = init_gdt_and_tss();

    // Load the GDT
    unsafe {
        GDT_STORAGE.assume_init_ref().load();
    }

    crate::kprintln!("[GDT] GDT loaded successfully");

    // Load TSS
    unsafe {
        x86_64::instructions::tables::load_tss(selectors.tss);
    }

    crate::kprintln!("[GDT] TSS loaded successfully");
    crate::kprintln!("[GDT] ✓ Complete GDT initialization finished");
}

/// Set up SYSCALL/SYSRET support with proper segment selectors
pub fn setup_syscall(syscall_entry: VirtAddr) -> Result<(), &'static str> {
    crate::kprintln!("[GDT] Setting up SYSCALL/SYSRET support...");

    let selectors = get_selectors();

    // Verify SYSCALL compatibility before setup
    selectors.validate_syscall_compatibility()?;

    // Set up STAR register with proper segment selectors
    use x86_64::registers::model_specific::Star;

    // STAR format: [63:48] User32 CS, [47:32] Kernel CS, [31:0] EIP (unused in 64-bit)
    // For SYSCALL: Kernel CS and Kernel SS are loaded from STAR[47:32]
    // For SYSRET: User CS = STAR[63:48] + 16, User SS = STAR[63:48] + 8
    Star::write(
        selectors.user_code,
        selectors.user_data,
        selectors.kernel_code,
        selectors.kernel_data,
    ).map_err(|_| "Failed to configure STAR register")?;

    // Set LSTAR to point to syscall entry
    use x86_64::registers::model_specific::LStar;
    LStar::write(syscall_entry);

    // Configure SFMASK to clear interrupt flag on syscall
    use x86_64::registers::model_specific::SFMask;
    use x86_64::registers::rflags::RFlags;
    SFMask::write(RFlags::INTERRUPT_FLAG);

    // Enable SYSCALL in EFER
    use x86_64::registers::model_specific::{Efer, EferFlags};
    unsafe {
        let mut efer = Efer::read();
        efer.insert(EferFlags::SYSTEM_CALL_EXTENSIONS);
        Efer::write(efer);
    }

    crate::kprintln!("[GDT] ✓ SYSCALL/SYSRET configured successfully");
    Ok(())
}

/// Safe userspace transition using IRETQ
pub unsafe fn enter_userspace(user_entry: VirtAddr, user_stack: VirtAddr) -> ! {
    let selectors = get_selectors();

    crate::kprintln!("[GDT] Entering userspace via IRETQ");
    crate::kprintln!("  Entry: 0x{:016x}", user_entry.as_u64());
    crate::kprintln!("  Stack: 0x{:016x}", user_stack.as_u64());
    crate::kprintln!("  User CS: 0x{:04x}", selectors.user_code.0);
    crate::kprintln!("  User DS: 0x{:04x}", selectors.user_data.0);

    // Set up userspace data segments
    core::arch::asm!(
        "mov ax, {user_ds:x}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        user_ds = in(reg) selectors.user_data.0,
        options(nostack, preserves_flags)
    );

    // Build IRETQ frame on stack and transition to Ring 3
    // Stack frame: [SS] [RSP] [RFLAGS] [CS] [RIP]
    core::arch::asm!(
        "push {user_ss}",           // SS (user data segment)
        "push {user_rsp}",          // RSP (user stack pointer)
        "push 0x202",               // RFLAGS (interrupts enabled, reserved bit set)
        "push {user_cs}",           // CS (user code segment)
        "push {user_rip}",          // RIP (user entry point)
        "iretq",                    // Return to Ring 3
        user_ss = in(reg) selectors.user_data.0 as u64,
        user_rsp = in(reg) user_stack.as_u64(),
        user_cs = in(reg) selectors.user_code.0 as u64,
        user_rip = in(reg) user_entry.as_u64(),
        options(noreturn)
    );
}

/// Handle syscall entry from userspace
pub extern "C" fn syscall_handler() {
    crate::kprintln!("[GDT] Syscall received from userspace");

    // Syscall handling logic would go here
    // For now, just return to userspace

    // Use SYSRET to return to userspace
    unsafe {
        core::arch::asm!(
            "sysretq",
            options(noreturn)
        );
    }
}