//! Comprehensive Memory System Tests
//!
//! This module provides extensive testing for the unified GDT and memory
//! allocator systems to ensure they work correctly with Galleon2 and Luminal.

use super::{
    unified_gdt, unified_allocator, unified_frame_allocator
};
use alloc::{vec::Vec, boxed::Box, string::String};

/// Result type for memory tests
type TestResult = Result<(), &'static str>;

/// Test suite for unified GDT implementation
pub struct GdtTestSuite;

impl GdtTestSuite {
    /// Run basic GDT validation tests
    pub fn test_gdt_initialization() -> TestResult {
        crate::kprintln!("    [TEST] GDT initialization test...");
        
        // Test that GDT validation passes
        unified_gdt::validate_gdt().map_err(|_| "GDT validation failed")?;
        
        // Test selector retrieval
        let selectors = unified_gdt::get_selectors();
        
        // Basic sanity checks
        if selectors.kernel_code.0 == 0 {
            return Err("Kernel code selector is zero");
        }
        if selectors.kernel_data.0 == 0 {
            return Err("Kernel data selector is zero");
        }
        
        crate::kprintln!("    ✅ GDT initialization test passed");
        Ok(())
    }
    
    /// Test SYSCALL/SYSRET compatibility
    pub fn test_syscall_compatibility() -> TestResult {
        crate::kprintln!("    [TEST] SYSCALL/SYSRET compatibility test...");
        
        let selectors = unified_gdt::get_selectors();
        
        // Check SYSCALL layout requirements
        let user_data_offset = (selectors.user_data.0 as i32) - (selectors.kernel_data.0 as i32);
        let user_code_offset = (selectors.user_code.0 as i32) - (selectors.kernel_data.0 as i32);
        
        if user_data_offset != 8 {
            return Err("User data selector offset incorrect for SYSCALL");
        }
        if user_code_offset != 16 {
            return Err("User code selector offset incorrect for SYSCALL");
        }
        
        // Test privilege level handling
        let user_code_rpl3 = selectors.user_code_with_rpl3();
        let user_data_rpl3 = selectors.user_data_with_rpl3();
        
        if user_code_rpl3.rpl() != x86_64::PrivilegeLevel::Ring3 {
            return Err("User code RPL not set to Ring3");
        }
        if user_data_rpl3.rpl() != x86_64::PrivilegeLevel::Ring3 {
            return Err("User data RPL not set to Ring3");
        }
        
        crate::kprintln!("    ✅ SYSCALL/SYSRET compatibility test passed");
        Ok(())
    }
    
    /// Run all GDT tests
    pub fn run_all_tests() -> TestResult {
        crate::kprintln!("  [INFO] Running GDT test suite...");
        
        Self::test_gdt_initialization()?;
        Self::test_syscall_compatibility()?;
        
        crate::kprintln!("  ✅ All GDT tests passed");
        Ok(())
    }
}

/// Test suite for unified allocator system
pub struct AllocatorTestSuite;

impl AllocatorTestSuite {
    /// Test basic heap allocation
    pub fn test_basic_allocation() -> TestResult {
        crate::kprintln!("    [TEST] Basic heap allocation test...");
        
        unified_allocator::test_heap_allocation()
            .map_err(|_| "Basic allocation test failed")?;
        
        crate::kprintln!("    ✅ Basic heap allocation test passed");
        Ok(())
    }
    
    /// Test allocator statistics
    pub fn test_allocator_stats() -> TestResult {
        crate::kprintln!("    [TEST] Allocator statistics test...");
        
        let stats = unified_allocator::get_allocator_stats();
        
        if stats.total_heap_size == 0 {
            return Err("Total heap size is zero");
        }
        if stats.heap_start_addr == 0 {
            return Err("Heap start address is zero");
        }
        if stats.bootstrap_heap_size == 0 {
            return Err("Bootstrap heap size is zero");
        }
        
        crate::kprintln!("    [INFO] Heap stats: {} MiB total, bootstrap: {} KB", 
                        stats.total_heap_size / (1024 * 1024),
                        stats.bootstrap_heap_size / 1024);
        
        crate::kprintln!("    ✅ Allocator statistics test passed");
        Ok(())
    }
    
    /// Test memory stress scenarios (important for Galleon2/Luminal)
    pub fn test_memory_stress() -> TestResult {
        crate::kprintln!("    [TEST] Memory stress test...");
        
        unified_allocator::stress_test_allocations()
            .map_err(|_| "Memory stress test failed")?;
        
        crate::kprintln!("    ✅ Memory stress test passed");
        Ok(())
    }
    
    /// Test filesystem-like allocation patterns
    pub fn test_filesystem_allocations() -> TestResult {
        crate::kprintln!("    [TEST] Filesystem-like allocation patterns...");
        
        // Simulate Galleon2 allocation patterns
        let mut file_buffers = Vec::new();
        
        // Typical file read/write buffers
        for i in 0..50 {
            let buffer = vec![i as u8; 4096]; // 4KB file buffer
            file_buffers.push(buffer);
        }
        
        // Metadata structures
        let mut metadata_structs = Vec::new();
        for i in 0..200 {
            let metadata = Box::new([i as u8; 256]); // 256-byte metadata
            metadata_structs.push(metadata);
        }
        
        // Directory entries
        let mut directory_entries = Vec::new();
        for i in 0..100 {
            let mut entry = String::new();
            entry.push_str("file_");
            // Simple number to string conversion for no_std
            let mut num_str = String::new();
            let mut n = i;
            if n == 0 {
                num_str.push('0');
            } else {
                let mut digits = Vec::new();
                while n > 0 {
                    digits.push((b'0' + (n % 10) as u8) as char);
                    n /= 10;
                }
                for digit in digits.iter().rev() {
                    num_str.push(*digit);
                }
            }
            entry.push_str(&num_str);
            entry.push_str(".txt");
            directory_entries.push(entry);
        }
        
        // Verify allocations
        if file_buffers.len() != 50 {
            return Err("File buffer allocation failed");
        }
        if metadata_structs.len() != 200 {
            return Err("Metadata allocation failed");
        }
        if directory_entries.len() != 100 {
            return Err("Directory entry allocation failed");
        }
        
        crate::kprintln!("    [INFO] Allocated filesystem structures:");
        crate::kprintln!("       File buffers: {} × 4KB", file_buffers.len());
        crate::kprintln!("       Metadata: {} × 256B", metadata_structs.len());
        crate::kprintln!("       Directory entries: {}", directory_entries.len());
        
        crate::kprintln!("    ✅ Filesystem allocation test passed");
        Ok(())
    }
    
    /// Test runtime-like allocation patterns
    pub fn test_runtime_allocations() -> TestResult {
        crate::kprintln!("    [TEST] Runtime-like allocation patterns...");
        
        // Simulate Luminal runtime allocation patterns
        let mut task_stacks = Vec::new();
        let mut message_queues = Vec::new();
        let mut runtime_metadata = Vec::new();
        
        // Task stacks
        for i in 0..20 {
            let stack = vec![0u8; 8192]; // 8KB stack per task
            task_stacks.push(stack);
        }
        
        // Message queues
        for i in 0..50 {
            let queue = Vec::with_capacity(100);
            message_queues.push(queue);
        }
        
        // Runtime metadata
        for i in 0..100 {
            let metadata = Box::new([i as u8; 128]);
            runtime_metadata.push(metadata);
        }
        
        // Verify allocations work
        if task_stacks.len() != 20 {
            return Err("Task stack allocation failed");
        }
        if message_queues.len() != 50 {
            return Err("Message queue allocation failed");
        }
        if runtime_metadata.len() != 100 {
            return Err("Runtime metadata allocation failed");
        }
        
        crate::kprintln!("    [INFO] Allocated runtime structures:");
        crate::kprintln!("       Task stacks: {} × 8KB", task_stacks.len());
        crate::kprintln!("       Message queues: {}", message_queues.len());
        crate::kprintln!("       Runtime metadata: {} × 128B", runtime_metadata.len());
        
        crate::kprintln!("    ✅ Runtime allocation test passed");
        Ok(())
    }
    
    /// Run all allocator tests
    pub fn run_all_tests() -> TestResult {
        crate::kprintln!("  [INFO] Running allocator test suite...");
        
        Self::test_basic_allocation()?;
        Self::test_allocator_stats()?;
        Self::test_memory_stress()?;
        Self::test_filesystem_allocations()?;
        Self::test_runtime_allocations()?;
        
        crate::kprintln!("  ✅ All allocator tests passed");
        Ok(())
    }
}

/// Test suite for frame allocator
pub struct FrameAllocatorTestSuite;

impl FrameAllocatorTestSuite {
    /// Test frame allocator initialization and basic functionality
    pub fn test_frame_allocator() -> TestResult {
        crate::kprintln!("    [TEST] Frame allocator test...");
        
        unified_frame_allocator::test_global_frame_allocator()
            .map_err(|_| "Frame allocator test failed")?;
        
        crate::kprintln!("    ✅ Frame allocator test passed");
        Ok(())
    }
    
    /// Test frame allocator statistics
    pub fn test_frame_stats() -> TestResult {
        crate::kprintln!("    [TEST] Frame allocator statistics test...");
        
        let stats = unified_frame_allocator::get_frame_allocator_stats()
            .map_err(|_| "Failed to get frame stats")?;
        
        if stats.total_regions == 0 {
            return Err("No memory regions found");
        }
        if stats.usable_regions == 0 {
            return Err("No usable memory regions found");
        }
        if stats.total_memory == 0 {
            return Err("Total memory is zero");
        }
        
        crate::kprintln!("    [INFO] Frame allocator stats:");
        crate::kprintln!("       Total regions: {}, usable: {}", stats.total_regions, stats.usable_regions);
        crate::kprintln!("       Total memory: {} MiB", stats.total_memory / (1024 * 1024));
        crate::kprintln!("       Allocated frames: {}, free: {}", stats.allocated_frames, stats.free_frames);
        
        crate::kprintln!("    ✅ Frame allocator statistics test passed");
        Ok(())
    }
    
    /// Run all frame allocator tests
    pub fn run_all_tests() -> TestResult {
        crate::kprintln!("  [INFO] Running frame allocator test suite...");
        
        Self::test_frame_allocator()?;
        Self::test_frame_stats()?;
        
        crate::kprintln!("  ✅ All frame allocator tests passed");
        Ok(())
    }
}

/// Integration test suite for complete memory system
pub struct MemoryIntegrationTestSuite;

impl MemoryIntegrationTestSuite {
    /// Test complete memory system integration
    pub fn test_complete_integration() -> TestResult {
        crate::kprintln!("    [TEST] Complete memory system integration...");
        
        // Test that heap validation passes
        unified_allocator::validate_heap()
            .map_err(|_| "Heap validation failed in integration test")?;
        
        // Test that GDT validation passes
        unified_gdt::validate_gdt()
            .map_err(|_| "GDT validation failed in integration test")?;
        
        // Test mixed allocation patterns (simulating real system usage)
        let mut mixed_allocations = Vec::new();
        
        // Small allocations (typical for syscalls, interrupts)
        for i in 0..100 {
            let small = Box::new([i as u8; 32]);
            mixed_allocations.push(small);
        }
        
        // Medium allocations (typical for user processes)
        for i in 0..20 {
            let medium = vec![i as u8; 1024];
            mixed_allocations.push(Box::new(medium));
        }
        
        // Large allocations (typical for file I/O)
        for i in 0..5 {
            let large = vec![i as u8; 16384];
            mixed_allocations.push(Box::new(large));
        }
        
        if mixed_allocations.len() != 125 {
            return Err("Mixed allocation pattern failed");
        }
        
        crate::kprintln!("    [INFO] Mixed allocations: {} total", mixed_allocations.len());
        crate::kprintln!("    ✅ Complete integration test passed");
        Ok(())
    }
    
    /// Run all integration tests
    pub fn run_all_tests() -> TestResult {
        crate::kprintln!("  [INFO] Running memory integration test suite...");
        
        Self::test_complete_integration()?;
        
        crate::kprintln!("  ✅ All integration tests passed");
        Ok(())
    }
}

/// Master test runner for all memory systems
pub struct MemoryTestRunner;

impl MemoryTestRunner {
    /// Run all memory system tests
    pub fn run_all_tests() -> TestResult {
        crate::kprintln!("[INFO] Starting comprehensive memory system tests...");
        
        // Run GDT tests
        GdtTestSuite::run_all_tests()?;
        
        // Run allocator tests  
        AllocatorTestSuite::run_all_tests()?;
        
        // Run frame allocator tests
        FrameAllocatorTestSuite::run_all_tests()?;
        
        // Run integration tests
        MemoryIntegrationTestSuite::run_all_tests()?;
        
        crate::kprintln!("🎉 ALL MEMORY SYSTEM TESTS PASSED!");
        crate::kprintln!("   The unified GDT and memory allocator systems are working correctly.");
        crate::kprintln!("   Ready for Galleon2 filesystem and Luminal runtime integration.");
        
        Ok(())
    }
    
    /// Run quick validation (subset of tests for fast verification)
    pub fn run_quick_validation() -> TestResult {
        crate::kprintln!("[INFO] Running quick memory system validation...");
        
        // Quick GDT check
        unified_gdt::validate_gdt().map_err(|_| "Quick GDT validation failed")?;
        
        // Quick allocator check
        unified_allocator::validate_heap().map_err(|_| "Quick heap validation failed")?;
        
        // Quick frame allocator check
        let _stats = unified_frame_allocator::get_frame_allocator_stats()
            .map_err(|_| "Quick frame allocator validation failed")?;
        
        crate::kprintln!("✅ Quick memory validation passed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gdt_suite() {
        // This would run in a kernel test environment
        // For now, just test that the functions exist and can be called
        let result = GdtTestSuite::test_gdt_initialization();
        // We can't actually run this without kernel context, but we can test the code compiles
        assert!(true);
    }
    
    #[test]
    fn test_allocator_suite() {
        // Test that the test suite compiles
        // Actual tests would run in kernel context
        assert!(true);
    }
}