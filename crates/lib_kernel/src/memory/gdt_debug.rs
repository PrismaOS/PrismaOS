//! Test program to verify GDT selector layout
//! This helps debug the SYSCALL/SYSRET compatibility issue

#[cfg(test)]
mod gdt_layout_test {
    use super::super::unified_gdt::get_selectors;
    
    #[test]
    fn test_gdt_syscall_layout() {
        let selectors = get_selectors();
        
        // Expected layout for SYSCALL/SYSRET compatibility:
        // Kernel CS: 0x08
        // Kernel DS: 0x10
        // User DS:   0x18 (Kernel DS + 8)
        // User CS:   0x20 (Kernel DS + 16)
        
        assert_eq!(selectors.kernel_code.0, 0x08, "Kernel CS should be 0x08");
        assert_eq!(selectors.kernel_data.0, 0x10, "Kernel DS should be 0x10");
        assert_eq!(selectors.user_data.0, 0x18, "User DS should be 0x18");
        assert_eq!(selectors.user_code.0, 0x20, "User CS should be 0x20");
        
        // Verify offsets
        let user_data_offset = (selectors.user_data.0 as i32) - (selectors.kernel_data.0 as i32);
        let user_code_offset = (selectors.user_code.0 as i32) - (selectors.kernel_data.0 as i32);
        
        assert_eq!(user_data_offset, 8, "User DS should be Kernel DS + 8");
        assert_eq!(user_code_offset, 16, "User CS should be Kernel DS + 16");
    }
    
    #[test]
    fn test_gdt_initialization_success() {
        // This test will fail if init() returns an error
        let result = super::super::unified_gdt::init();
        assert!(result.is_ok(), "GDT initialization should succeed: {:?}", result);
    }
}

pub fn debug_gdt_selectors() {
    use super::unified_gdt::get_selectors;
    
    let selectors = get_selectors();
    
    crate::kprintln!("=== GDT Selector Debug Information ===");
    crate::kprintln!("Kernel CS: {:#x}", selectors.kernel_code.0);
    crate::kprintln!("Kernel DS: {:#x}", selectors.kernel_data.0);
    crate::kprintln!("User DS:   {:#x}", selectors.user_data.0);
    crate::kprintln!("User CS:   {:#x}", selectors.user_code.0);
    crate::kprintln!("TSS:       {:#x}", selectors.tss.0);
    
    let user_data_offset = (selectors.user_data.0 as i32) - (selectors.kernel_data.0 as i32);
    let user_code_offset = (selectors.user_code.0 as i32) - (selectors.kernel_data.0 as i32);
    
    crate::kprintln!("User DS offset from Kernel DS: {}", user_data_offset);
    crate::kprintln!("User CS offset from Kernel DS: {}", user_code_offset);
    
    if user_data_offset == 8 && user_code_offset == 16 {
        crate::kprintln!("✅ GDT layout is SYSCALL/SYSRET compatible!");
    } else {
        crate::kprintln!("❌ GDT layout is NOT SYSCALL/SYSRET compatible!");
        crate::kprintln!("   Expected: User DS = Kernel DS + 8, User CS = Kernel DS + 16");
        crate::kprintln!("   Actual: User DS = Kernel DS + {}, User CS = Kernel DS + {}", 
                user_data_offset, user_code_offset);
    }
}