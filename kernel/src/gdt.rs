use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use lazy_static::lazy_static;

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
        
        // SYSCALL/SYSRET requires specific segment layout:
        // Index 0: NULL (automatically added)
        // Index 1: Kernel Code
        // Index 2: Kernel Data  
        // Index 3: User Code (for SYSCALL: kernel_cs + 16)
        // Index 4: User Data (for SYSCALL: kernel_cs + 8, but we put it after user code)
        // Index 5: TSS
        
        let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment()); 
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        
        (gdt, Selectors { 
            kernel_code_selector,
            kernel_data_selector, 
            user_code_selector,
            user_data_selector,
            tss_selector 
        })
    };
}

pub struct Selectors {
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    user_code_selector: SegmentSelector, 
    user_data_selector: SegmentSelector,
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
        self.user_code_selector
    }
    
    pub fn user_data(&self) -> SegmentSelector {
        self.user_data_selector
    }
}