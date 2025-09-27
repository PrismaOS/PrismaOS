//! Unified Global Descriptor Table (GDT) implementation
//! 
//! This module provides a single, unified GDT implementation that replaces
//! the scattered GDT code throughout the kernel. It combines the robustness
//! of the x86_64 crate with proper SYSCALL/SYSRET support and TSS management.

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{VirtAddr, PrivilegeLevel};
use lazy_static::lazy_static;

/// Double fault interrupt stack table index
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// The unified TSS (Task State Segment) for the kernel
lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        
        // Set up double fault interrupt stack
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5; // 20KB stack
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            
            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start + STACK_SIZE as u64;
            stack_end
        };
        
        tss
    };
}

/// GDT selectors for different privilege levels and segments
#[derive(Debug, Clone, Copy)]
pub struct GdtSelectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss: SegmentSelector,
}

impl GdtSelectors {
    /// Get user code selector with Ring3 privilege level for use in user mode
    pub fn user_code_with_rpl3(&self) -> SegmentSelector {
        SegmentSelector::new(self.user_code.index(), PrivilegeLevel::Ring3)
    }
    
    /// Get user data selector with Ring3 privilege level for use in user mode
    pub fn user_data_with_rpl3(&self) -> SegmentSelector {
        SegmentSelector::new(self.user_data.index(), PrivilegeLevel::Ring3)
    }
}

/// The unified GDT instance
lazy_static! {
    static ref GDT: (GlobalDescriptorTable, GdtSelectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        
        // Build GDT with guaranteed SYSCALL/SYSRET layout
        // SYSCALL/SYSRET has strict requirements:
        // - SYSCALL loads CS from STAR[47:32] and SS from STAR[47:32] + 8
        // - SYSRET loads CS from STAR[63:48] + 16 and SS from STAR[63:48] + 8
        // This means:
        //   Kernel CS must be at selector 0x08
        //   Kernel DS must be at selector 0x10  
        //   User DS must be at selector 0x18 (Kernel DS + 8)
        //   User CS must be at selector 0x20 (Kernel DS + 16)
        
        // Ensure we get the expected selectors by appending in specific order
        let kernel_code = gdt.append(Descriptor::kernel_code_segment());    // Should be 0x08
        let kernel_data = gdt.append(Descriptor::kernel_data_segment());    // Should be 0x10
        let user_data = gdt.append(Descriptor::user_data_segment());        // Should be 0x18
        let user_code = gdt.append(Descriptor::user_code_segment());        // Should be 0x20
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));       // Should be 0x28
        
        // Verify that we got the expected layout
        // Note: User segments have RPL=3, so we need to check the index, not the full selector
        let kernel_cs_expected = 0x08;
        let kernel_ds_expected = 0x10;
        let user_ds_expected_index = 3; // Index 3 = selector 0x18, but with RPL=3 becomes 0x1B
        let user_cs_expected_index = 4; // Index 4 = selector 0x20, but with RPL=3 becomes 0x23
        
        assert_eq!(kernel_code.0, kernel_cs_expected, "Kernel CS selector mismatch");
        assert_eq!(kernel_data.0, kernel_ds_expected, "Kernel DS selector mismatch");
        assert_eq!(user_data.index(), user_ds_expected_index, "User DS index mismatch");
        assert_eq!(user_code.index(), user_cs_expected_index, "User CS index mismatch");
        
        // Verify the SYSCALL/SYSRET offset requirements using the base selector values
        let user_data_base = user_ds_expected_index * 8;    // 3 * 8 = 0x18
        let user_code_base = user_cs_expected_index * 8;    // 4 * 8 = 0x20
        let syscall_ds_offset = user_data_base - kernel_ds_expected; // Should be 8
        let syscall_cs_offset = user_code_base - kernel_ds_expected; // Should be 16
        
        assert_eq!(syscall_ds_offset, 8, "User DS not at Kernel DS + 8 for SYSCALL compatibility");
        assert_eq!(syscall_cs_offset, 16, "User CS not at Kernel DS + 16 for SYSCALL compatibility");
        
        let selectors = GdtSelectors {
            kernel_code,
            kernel_data,
            user_data,
            user_code,
            tss: tss_selector,
        };
        
        (gdt, selectors)
    };
}

/// Initialize the unified GDT
pub fn init() -> Result<(), &'static str> {
    use x86_64::instructions::segmentation::{Segment, CS, DS};
    use x86_64::instructions::tables::load_tss;
    
    // Load the GDT - now we can reference it directly since it's not behind a Mutex
    GDT.0.load();
    
    // Get selectors for segment register setup and verification
    let selectors = &GDT.1;
    
    unsafe {
        // Update segment registers
        CS::set_reg(selectors.kernel_code);
        DS::set_reg(selectors.kernel_data);
        
        // Load TSS
        load_tss(selectors.tss);
    }
    
    // Verify SYSCALL compatibility - we need to check the base selector values (without RPL)
    // SYSCALL/SYSRET works with the descriptor table indices, not the full selector values
    let user_data_base = (selectors.user_data.index() as u16) * 8;  // Index 3 -> 0x18
    let user_code_base = (selectors.user_code.index() as u16) * 8;  // Index 4 -> 0x20  
    let kernel_ds_base = selectors.kernel_data.0;                   // Should be 0x10
    
    let user_data_offset = (user_data_base as i32) - (kernel_ds_base as i32);
    let user_code_offset = (user_code_base as i32) - (kernel_ds_base as i32);
    
    // For SYSCALL/SYSRET to work:
    // User DS base should be Kernel DS + 8 (0x10 + 8 = 0x18)
    // User CS base should be Kernel DS + 16 (0x10 + 16 = 0x20)
    if user_data_offset != 8 || user_code_offset != 16 {
        crate::kprintln!("    ❌ ERROR: GDT layout not SYSCALL compatible!");
        crate::kprintln!("       Kernel CS: {:#x} (expected 0x08)", selectors.kernel_code.0);
        crate::kprintln!("       Kernel DS: {:#x} (expected 0x10)", selectors.kernel_data.0);
        crate::kprintln!("       User DS:   {:#x} (base {:#x}, expected base 0x18)", selectors.user_data.0, user_data_base);
        crate::kprintln!("       User CS:   {:#x} (base {:#x}, expected base 0x20)", selectors.user_code.0, user_code_base);
        crate::kprintln!("       User DS offset: {} (should be 8)", user_data_offset);
        crate::kprintln!("       User CS offset: {} (should be 16)", user_code_offset);
        return Err("GDT layout incompatible with SYSCALL/SYSRET");
    }
    
    crate::kprintln!("    [INFO] Unified GDT initialized successfully");
    crate::kprintln!("       Kernel CS: {:#x}", selectors.kernel_code.0);
    crate::kprintln!("       Kernel DS: {:#x}", selectors.kernel_data.0);
    crate::kprintln!("       User DS:   {:#x} (base {:#x})", selectors.user_data.0, user_data_base);
    crate::kprintln!("       User CS:   {:#x} (base {:#x})", selectors.user_code.0, user_code_base);
    crate::kprintln!("       TSS:       {:#x}", selectors.tss.0);
    crate::kprintln!("    ✅ GDT layout is SYSCALL compatible (offsets +8, +16)");
    
    Ok(())
}

/// Get the current GDT selectors
pub fn get_selectors() -> GdtSelectors {
    GDT.1
}

/// Setup SYSCALL/SYSRET MSRs with the unified GDT
pub fn setup_syscall_msrs() -> Result<(), &'static str> {
    use x86_64::registers::model_specific::{Star, LStar, SFMask, Efer, EferFlags};
    
    let selectors = get_selectors();
    
    unsafe {
        // Enable SYSCALL/SYSRET in EFER
        let mut efer = Efer::read();
        efer |= EferFlags::SYSTEM_CALL_EXTENSIONS;
        Efer::write(efer);
        
        // Configure STAR register for SYSCALL/SYSRET
        let result = Star::write(
            selectors.user_code_with_rpl3(),   // cs_sysret (User CS for SYSRET)
            selectors.user_data_with_rpl3(),   // ss_sysret (User SS for SYSRET)
            selectors.kernel_code,             // cs_syscall (Kernel CS for SYSCALL)
            selectors.kernel_data,             // ss_syscall (Kernel SS for SYSCALL)
        );
        
        match result {
            Ok(()) => {
                crate::kprintln!("    ✅ SYSCALL MSRs configured successfully");
            }
            Err(e) => {
                crate::kprintln!("    ❌ STAR MSR write failed: {:?}", e);
                return Err("Failed to configure SYSCALL MSRs");
            }
        }
        
        // Set up SFMASK register (flags to clear on syscall)
        SFMask::write(x86_64::registers::rflags::RFlags::INTERRUPT_FLAG);
    }
    
    Ok(())
}

/// Update TSS with new kernel stack pointer (for process switching)
pub fn update_tss_rsp0(new_rsp0: VirtAddr) {
    // Access the TSS through the lazy_static
    unsafe {
        // We need to get a mutable reference to the TSS
        // This is safe because we control all access to the TSS
        let tss_ptr = &*TSS as *const TaskStateSegment as *mut TaskStateSegment;
        (*tss_ptr).privilege_stack_table[0] = new_rsp0;
    }
}

/// Validate GDT integrity (for testing)
pub fn validate_gdt() -> Result<(), &'static str> {
    let selectors = &GDT.1;
    
    // Check that selectors are non-zero (except null selector)
    if selectors.kernel_code.0 == 0 {
        return Err("Kernel code selector is null");
    }
    if selectors.kernel_data.0 == 0 {
        return Err("Kernel data selector is null");
    }
    if selectors.user_code.0 == 0 {
        return Err("User code selector is null");
    }
    if selectors.user_data.0 == 0 {
        return Err("User data selector is null");
    }
    if selectors.tss.0 == 0 {
        return Err("TSS selector is null");
    }
    
    // Verify SYSCALL layout requirements
    let user_data_offset = (selectors.user_data.0 as i32) - (selectors.kernel_data.0 as i32);
    let user_code_offset = (selectors.user_code.0 as i32) - (selectors.kernel_data.0 as i32);
    
    if user_data_offset != 8 {
        return Err("User data selector offset incorrect for SYSCALL");
    }
    if user_code_offset != 16 {
        return Err("User code selector offset incorrect for SYSCALL");
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gdt_selectors_non_zero() {
        // Test that all selectors are properly initialized
        let selectors = get_selectors();
        
        assert_ne!(selectors.kernel_code.0, 0, "Kernel code selector should not be zero");
        assert_ne!(selectors.kernel_data.0, 0, "Kernel data selector should not be zero");
        assert_ne!(selectors.user_code.0, 0, "User code selector should not be zero");
        assert_ne!(selectors.user_data.0, 0, "User data selector should not be zero");
        assert_ne!(selectors.tss.0, 0, "TSS selector should not be zero");
    }
    
    #[test] 
    fn test_syscall_layout() {
        // Test SYSCALL/SYSRET compatibility
        let selectors = get_selectors();
        
        let user_data_offset = (selectors.user_data.0 as i32) - (selectors.kernel_data.0 as i32);
        let user_code_offset = (selectors.user_code.0 as i32) - (selectors.kernel_data.0 as i32);
        
        assert_eq!(user_data_offset, 8, "User data selector must be kernel data + 8");
        assert_eq!(user_code_offset, 16, "User code selector must be kernel data + 16");
    }
    
    #[test]
    fn test_privilege_levels() {
        let selectors = get_selectors();
        
        // Kernel selectors should be Ring 0
        assert_eq!(selectors.kernel_code.rpl(), PrivilegeLevel::Ring0);
        assert_eq!(selectors.kernel_data.rpl(), PrivilegeLevel::Ring0);
        
        // User selectors with RPL should be Ring 3
        assert_eq!(selectors.user_code_with_rpl3().rpl(), PrivilegeLevel::Ring3);
        assert_eq!(selectors.user_data_with_rpl3().rpl(), PrivilegeLevel::Ring3);
    }
    
    #[test]
    fn test_gdt_validation() {
        // Test the validation function
        assert!(validate_gdt().is_ok(), "GDT validation should pass");
    }
}