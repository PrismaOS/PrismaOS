use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{VirtAddr, PrivilegeLevel};
use lazy_static::lazy_static;
use crate::scheduler::process::ProcessContext;
// use crate::syscall::entry::syscall_entry;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start + STACK_SIZE as u64;
            stack_end
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        
        // FIXED: SYSCALL/SYSRET requires specific segment layout:
        // We need to ensure User DS = Kernel DS + 8 and User CS = Kernel DS + 16
        // Let's build it step by step and verify
        
        let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());    // Should be 0x08
        let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());    // Should be 0x10
        
        // Calculate what we need for SYSCALL compatibility
        let expected_user_data = kernel_data_selector.0 + 8;   // Should be 0x18
        let expected_user_code = kernel_data_selector.0 + 16;  // Should be 0x20
        
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        
        // Debug: Print actual selector values
        crate::kprintln!("    GDT selector values:");
        crate::kprintln!("       Kernel CS: {:#x}", kernel_code_selector.0);
        crate::kprintln!("       Kernel DS: {:#x}", kernel_data_selector.0);
        crate::kprintln!("       User DS:   {:#x}", user_data_selector.0);
        crate::kprintln!("       User CS:   {:#x}", user_code_selector.0);
        crate::kprintln!("       TSS:       {:#x}", tss_selector.0);
        crate::kprintln!("       Expected User DS: {:#x}", expected_user_data);
        crate::kprintln!("       Expected User CS: {:#x}", expected_user_code);
        
        // Check if our layout is SYSCALL compatible
        let user_data_offset = (user_data_selector.0 as i32) - (kernel_data_selector.0 as i32);
        let user_code_offset = (user_code_selector.0 as i32) - (kernel_data_selector.0 as i32);
        
        if user_data_offset != 8 || user_code_offset != 16 {
            crate::kprintln!("    ⚠️  WARNING: GDT layout not SYSCALL compatible!");
            crate::kprintln!("       User DS offset: {} (should be 8)", user_data_offset);
            crate::kprintln!("       User CS offset: {} (should be 16)", user_code_offset);
            crate::kprintln!("       SYSCALL/SYSRET will not work with this layout");
        } else {
            crate::kprintln!("    ✅ GDT layout is SYSCALL compatible");
        }
        
        (gdt, Selectors { 
            kernel_code_selector,
            kernel_data_selector, 
            user_data_selector,
            user_code_selector,
            tss_selector 
        })
    };
}

pub struct Selectors {
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    user_code_selector: SegmentSelector, 
    tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS, DS};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.kernel_code_selector);
        DS::set_reg(GDT.1.kernel_data_selector);
        load_tss(GDT.1.tss_selector);
    }
    
    // crate::kprintln!("    [INFO] GDT initialized");
    // crate::kprintln!("       Kernel CS: {:#x}", GDT.1.kernel_code_selector.0);
    // crate::kprintln!("       Kernel DS: {:#x}", GDT.1.kernel_data_selector.0);
    // crate::kprintln!("       User DS:   {:#x}", GDT.1.user_data_selector.0);
    // crate::kprintln!("       User CS:   {:#x}", GDT.1.user_code_selector.0);
}

/// Get the GDT selectors for SYSCALL/SYSRET setup
pub fn get_selectors() -> &'static Selectors {
    &GDT.1
}

impl Selectors {
    pub fn kernel_code(&self) -> SegmentSelector {
        self.kernel_code_selector
    }
    
    pub fn kernel_data(&self) -> SegmentSelector {
        self.kernel_data_selector
    }
    
    pub fn user_code(&self) -> SegmentSelector {
        // Return with RPL=3 for userspace
        SegmentSelector::new(self.user_code_selector.index(), PrivilegeLevel::Ring3)
    }
    
    pub fn user_data(&self) -> SegmentSelector {
        // Return with RPL=3 for userspace
        SegmentSelector::new(self.user_data_selector.index(), PrivilegeLevel::Ring3)
    }
}

/// FIXED: Switch to userspace using runtime selector values
#[no_mangle]
pub unsafe extern "C" fn switch_to_userspace(context: *const ProcessContext) -> ! {
    // Get the actual selectors from our GDT
    let selectors = get_selectors();
    let user_data_with_rpl3 = selectors.user_data().0;
    let user_code_with_rpl3 = selectors.user_code().0;
    
    core::arch::asm!(
        // Load new page table
        "mov rax, [rdi + 0x90]",
        "mov cr3, rax",
        
        // Set up user data segments using runtime values
        "mov ax, {user_data_sel:x}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax", 
        "mov gs, ax",
        
        // Push stack frame for iretq
        "push {user_data_sel:x}",    // SS (user stack segment)
        "push [rdi + 0x38]",         // RSP
        "push [rdi + 0x88]",         // RFLAGS 
        "push {user_code_sel:x}",    // CS (user code segment)
        "push [rdi + 0x80]",         // RIP
        
        // Load general purpose registers
        "mov rax, [rdi + 0x00]",
        "mov rbx, [rdi + 0x08]",
        "mov rcx, [rdi + 0x10]",
        "mov rdx, [rdi + 0x18]",
        "mov rsi, [rdi + 0x20]",
        "mov rbp, [rdi + 0x30]",
        "mov r8,  [rdi + 0x40]",
        "mov r9,  [rdi + 0x48]",
        "mov r10, [rdi + 0x50]",
        "mov r11, [rdi + 0x58]",
        "mov r12, [rdi + 0x60]",
        "mov r13, [rdi + 0x68]",
        "mov r14, [rdi + 0x70]",
        "mov r15, [rdi + 0x78]",
        
        // Load RDI last
        "mov rdi, [rdi + 0x28]",
        
        // Switch to userspace
        "iretq",
        
        in("rdi") context,
        user_data_sel = in(reg) user_data_with_rpl3 as u64,
        user_code_sel = in(reg) user_code_with_rpl3 as u64,
        options(noreturn)
    );
}

/// FIXED: SYSCALL MSR setup with correct parameter order
pub fn setup_syscall_msrs() {
    use x86_64::registers::model_specific::{Star, LStar, SFMask, Efer, EferFlags};
    use x86_64::VirtAddr;
    
    unsafe {
        // Enable SYSCALL/SYSRET in EFER
        let mut efer = Efer::read();
        efer |= EferFlags::SYSTEM_CALL_EXTENSIONS;
        Efer::write(efer);

        // Get selectors with corrected GDT layout
        let selectors = get_selectors();
        
        // crate::kprintln!("    Setting up SYSCALL MSRs:");
        // crate::kprintln!("       Kernel CS: {:#x}", selectors.kernel_code().0);
        // crate::kprintln!("       Kernel DS: {:#x}", selectors.kernel_data().0);
        // crate::kprintln!("       User DS:   {:#x}", selectors.user_data().0);
        // crate::kprintln!("       User CS:   {:#x}", selectors.user_code().0);
        
        // FIXED: Correct parameter order for Star::write
        // Star::write(cs_sysret, ss_sysret, cs_syscall, ss_syscall)
        let result = Star::write(
            selectors.user_code(),   // cs_sysret (User CS for SYSRET)
            selectors.user_data(),   // ss_sysret (User SS for SYSRET) 
            selectors.kernel_code(), // cs_syscall (Kernel CS for SYSCALL)
            selectors.kernel_data(), // ss_syscall (Kernel SS for SYSCALL)
        );
        
        if let Err(e) = result {
        //     crate::kprintln!("    ❌ STAR MSR write failed: {:?}", e);
        //     crate::kprintln!("    This indicates GDT layout doesn't meet SYSCALL requirements");
        //     crate::kprintln!("    Expected: User CS = Kernel DS + 16, User DS = Kernel DS + 8");
            return;
        }

        // Set up LSTAR register (syscall entry point) - DISABLED FOR NOW
        // LStar::write(VirtAddr::new(syscall_entry as u64));

        // Set up SFMASK register (flags to clear on syscall)
        SFMask::write(x86_64::registers::rflags::RFlags::INTERRUPT_FLAG);
    }

    // crate::kprintln!("    ✅ SYSCALL MSRs configured successfully");
}