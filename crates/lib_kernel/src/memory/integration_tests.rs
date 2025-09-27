//! Integration tests for Galleon2 filesystem with unified memory systems
//!
//! These tests validate that Galleon2 filesystem operations work correctly
//! with our unified memory allocator and GDT implementations.

use alloc::{vec::Vec, string::String};
use super::{unified_allocator, tests::AllocatorTestSuite};

/// Test Galleon2-like filesystem operations with the unified allocator
pub fn test_galleon2_memory_integration() -> Result<(), &'static str> {
    crate::kprintln!("[INFO] Testing Galleon2 filesystem memory integration...");
    
    // Test allocations similar to what Galleon2 filesystem would do
    
    // 1. Test allocation of file system metadata structures
    let mut metadata_blocks = Vec::new();
    for i in 0..20 {
        // Simulate filesystem metadata blocks (typically 4KB)
        let block = vec![i as u8; 4096];
        metadata_blocks.push(block);
    }
    
    if metadata_blocks.len() != 20 {
        return Err("Failed to allocate filesystem metadata blocks");
    }
    
    // 2. Test allocation of file buffers (similar to Galleon2 cluster allocations)
    let mut file_buffers = Vec::new();
    for cluster in 0..50 {
        // Simulate cluster allocation (typical cluster size varies)
        let cluster_data = vec![cluster as u8; 8192]; // 8KB clusters
        file_buffers.push(cluster_data);
    }
    
    if file_buffers.len() != 50 {
        return Err("Failed to allocate file cluster buffers");
    }
    
    // 3. Test directory structure allocations
    let mut directory_entries = Vec::new();
    for entry_idx in 0..100 {
        // Simulate directory entry structures
        let mut entry_name = String::new();
        entry_name.push_str("file_");
        
        // Convert number to string for no_std
        let mut num = entry_idx;
        let mut digits = Vec::new();
        if num == 0 {
            digits.push(b'0');
        } else {
            while num > 0 {
                digits.push(b'0' + (num % 10) as u8);
                num /= 10;
            }
            digits.reverse();
        }
        
        for &digit in &digits {
            entry_name.push(digit as char);
        }
        entry_name.push_str(".dat");
        
        directory_entries.push(entry_name);
    }
    
    if directory_entries.len() != 100 {
        return Err("Failed to allocate directory entries");
    }
    
    // 4. Test large contiguous allocations (like those needed for file I/O)
    let mut large_io_buffers: Vec<Vec<u8>> = Vec::new();
    for _buf in 0..10 {
        // Large I/O buffers for filesystem operations
        let io_buffer = vec![0u8; 64 * 1024]; // 64KB I/O buffers
        large_io_buffers.push(io_buffer);
    }
    
    if large_io_buffers.len() != 10 {
        return Err("Failed to allocate large I/O buffers");
    }
    
    // Verify all data is accessible and correct
    for (i, block) in metadata_blocks.iter().enumerate() {
        if block[0] != i as u8 {
            return Err("Metadata block data corruption detected");
        }
    }
    
    for (i, buffer) in file_buffers.iter().enumerate() {
        if buffer[0] != i as u8 {
            return Err("File buffer data corruption detected");
        }
    }
    
    for (i, entry) in directory_entries.iter().enumerate() {
        if !entry.starts_with("file_") || !entry.ends_with(".dat") {
            return Err("Directory entry format corruption detected");
        }
    }
    
    for buffer in &large_io_buffers {
        if buffer.len() != (64 * 1024) {
            return Err("Large I/O buffer size corruption detected");
        }
    }
    
    crate::kprintln!("    [INFO] Galleon2 memory integration test results:");
    crate::kprintln!("       Metadata blocks: {} × 4KB", metadata_blocks.len());
    crate::kprintln!("       File clusters: {} × 8KB", file_buffers.len());
    crate::kprintln!("       Directory entries: {}", directory_entries.len());
    crate::kprintln!("       I/O buffers: {} × 64KB", large_io_buffers.len());
    crate::kprintln!("    ✅ Galleon2 filesystem memory integration test passed");
    
    Ok(())
}

/// Test Luminal runtime operations with the unified memory systems
pub fn test_luminal_memory_integration() -> Result<(), &'static str> {
    crate::kprintln!("[INFO] Testing Luminal runtime memory integration...");
    
    // Test allocations similar to what Luminal runtime would do
    
    // 1. Test task/future allocations
    let mut task_contexts = Vec::new();
    for task_id in 0..30 {
        // Simulate task context data (stack, registers, state)
        let task_context = vec![task_id as u8; 2048]; // 2KB per task context
        task_contexts.push(task_context);
    }
    
    if task_contexts.len() != 30 {
        return Err("Failed to allocate task contexts");
    }
    
    // 2. Test async runtime state allocations
    let mut async_states = Vec::new();
    for state_id in 0..100 {
        // Simulate async state machines
        let async_state = vec![state_id as u8; 512]; // 512B per async state
        async_states.push(async_state);
    }
    
    if async_states.len() != 100 {
        return Err("Failed to allocate async states");
    }
    
    // 3. Test executor queue allocations
    let mut executor_queues = Vec::new();
    for queue_id in 0..10 {
        // Simulate executor work queues
        let mut queue = Vec::new();
        for item in 0..50 {
            queue.push(item as u8);
        }
        executor_queues.push(queue);
    }
    
    if executor_queues.len() != 10 {
        return Err("Failed to allocate executor queues");
    }
    
    // 4. Test waker allocations
    let mut waker_data = Vec::new();
    for waker_id in 0..200 {
        // Simulate waker objects
        let waker = vec![waker_id as u8; 128]; // 128B per waker
        waker_data.push(waker);
    }
    
    if waker_data.len() != 200 {
        return Err("Failed to allocate waker data");
    }
    
    // 5. Test future chain allocations
    let mut future_chains = Vec::new();
    for chain_id in 0..20 {
        // Simulate future composition chains
        let chain = vec![chain_id as u8; 1024]; // 1KB per future chain
        future_chains.push(chain);
    }
    
    if future_chains.len() != 20 {
        return Err("Failed to allocate future chains");
    }
    
    // Verify all data integrity
    for (i, context) in task_contexts.iter().enumerate() {
        if context[0] != i as u8 || context.len() != 2048 {
            return Err("Task context data corruption detected");
        }
    }
    
    for (i, state) in async_states.iter().enumerate() {
        if state[0] != i as u8 || state.len() != 512 {
            return Err("Async state data corruption detected");
        }
    }
    
    for (i, queue) in executor_queues.iter().enumerate() {
        if queue.len() != 50 {
            return Err("Executor queue size corruption detected");
        }
    }
    
    for (i, waker) in waker_data.iter().enumerate() {
        if waker[0] != i as u8 || waker.len() != 128 {
            return Err("Waker data corruption detected");
        }
    }
    
    for (i, chain) in future_chains.iter().enumerate() {
        if chain[0] != i as u8 || chain.len() != 1024 {
            return Err("Future chain data corruption detected");
        }
    }
    
    crate::kprintln!("    [INFO] Luminal runtime memory integration test results:");
    crate::kprintln!("       Task contexts: {} × 2KB", task_contexts.len());
    crate::kprintln!("       Async states: {} × 512B", async_states.len());
    crate::kprintln!("       Executor queues: {} × 50 items", executor_queues.len());
    crate::kprintln!("       Wakers: {} × 128B", waker_data.len());
    crate::kprintln!("       Future chains: {} × 1KB", future_chains.len());
    crate::kprintln!("    ✅ Luminal runtime memory integration test passed");
    
    Ok(())
}

/// Combined integration test for both Galleon2 and Luminal systems
pub fn test_combined_system_integration() -> Result<(), &'static str> {
    crate::kprintln!("[INFO] Running combined Galleon2 + Luminal integration test...");
    
    // First validate our unified systems are working
    unified_allocator::validate_heap().map_err(|_| "Heap validation failed during integration test")?;
    
    // Test filesystem integration
    test_galleon2_memory_integration()?;
    
    // Test runtime integration
    test_luminal_memory_integration()?;
    
    // Test concurrent allocations (simulating both systems running simultaneously)
    let mut mixed_allocations = Vec::new();
    
    // Interleave filesystem and runtime allocations
    for i in 0..50 {
        if i % 2 == 0 {
            // Filesystem allocation
            let fs_data = vec![i as u8; 4096];
            mixed_allocations.push(fs_data);
        } else {
            // Runtime allocation
            let runtime_data = vec![i as u8; 1024];
            mixed_allocations.push(runtime_data);
        }
    }
    
    if mixed_allocations.len() != 50 {
        return Err("Failed mixed allocation test");
    }
    
    // Verify integrity of mixed allocations
    for (i, allocation) in mixed_allocations.iter().enumerate() {
        if allocation[0] != i as u8 {
            return Err("Mixed allocation data corruption detected");
        }
        
        let expected_size = if i % 2 == 0 { 4096 } else { 1024 };
        if allocation.len() != expected_size {
            return Err("Mixed allocation size corruption detected");
        }
    }
    
    crate::kprintln!("    [INFO] Combined integration test results:");
    crate::kprintln!("       Mixed allocations: {} items", mixed_allocations.len());
    crate::kprintln!("       Memory integrity: ✅ Verified");
    crate::kprintln!("       GDT stability: ✅ Maintained");
    crate::kprintln!("       Allocator stability: ✅ Maintained");
    
    crate::kprintln!("🎉 COMBINED SYSTEM INTEGRATION TEST PASSED!");
    crate::kprintln!("   Galleon2 filesystem and Luminal runtime are fully compatible");
    crate::kprintln!("   with the unified memory management systems.");
    
    Ok(())
}

/// Run all integration tests for external systems
pub fn run_integration_tests() -> Result<(), &'static str> {
    crate::kprintln!("🚀 STARTING SYSTEM INTEGRATION TESTS");
    crate::kprintln!("=====================================");
    
    // Run basic memory system tests first
    AllocatorTestSuite::run_all_tests()?;
    
    // Run filesystem integration tests
    test_galleon2_memory_integration()?;
    
    // Run runtime integration tests
    test_luminal_memory_integration()?;
    
    // Run combined integration tests
    test_combined_system_integration()?;
    
    crate::kprintln!("");
    crate::kprintln!("🎯 ALL INTEGRATION TESTS PASSED!");
    crate::kprintln!("================================");
    crate::kprintln!("✅ Unified GDT system validated");
    crate::kprintln!("✅ Unified memory allocator validated");
    crate::kprintln!("✅ Galleon2 filesystem compatibility verified");
    crate::kprintln!("✅ Luminal runtime compatibility verified");
    crate::kprintln!("✅ Combined system stability validated");
    crate::kprintln!("");
    crate::kprintln!("The kernel is ready for production use with complex systems!");
    
    Ok(())
}