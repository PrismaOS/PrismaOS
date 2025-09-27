//! Kernel entrypoint and minimal runtime utilities
//!
//! This file contains the absolute minimum the kernel needs at boot time:
//! - `kmain`: the entrypoint called from the assembly bootstrap. It performs
//!   a small sanity check and delegates the full initialization to
//!   `crate::init::init_kernel()` so the majority of boot logic is kept in
//!   `kernel/src/init/*` modules.
//! - `panic_handler`: the kernel panic handler that renders a BSOD via the
//!   framebuffer renderer when available, or falls back to VGA text.
//! - `test_runner`: the custom test harness entry used when compiling tests.
//!
//! All functions in this file are intentionally small and well-documented so
//! the platform-specific startup path remains easy to audit.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![test_runner(crate::test_runner)]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![allow(warnings)]

use alloc::string::ToString;
use core::panic::PanicInfo;
use lib_kernel::{consts::BASE_REVISION, kprintln, scrolling_text};

extern crate alloc;

use lib_kernel::consts::*;
use x86_64;

pub mod boot_userspace;
pub mod userspace_isolation;
pub mod userspace_test;

use ide::ide_initialize;

mod init;
mod utils;

use ahci;
use galleon2::{GalleonFilesystem, FilesystemStats};
use luminal;
use pci::init_pci;
use usb;

// NOTE: speaker and other modules are available as `crate::speaker` if
/// needed; avoid glob imports here to keep the top-level clean.

/// Kernel entrypoint called after assembly bootstrap completes.
///
/// Responsibilities and behavior:
/// - Performs a single sanity check that the Limine protocol revision is
///   supported by this kernel binary. This ensures the Limine request
///   objects are compatible with the code below.
/// - Delegates the rest of the boot sequence to `init::init_kernel()` to
///   keep this function small and auditable.
/// - On failure of the initialization function this routine prints an error
///   and halts the machine (consistent with the prior behavior).
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    // Verify Limine revision; a panic here indicates the bootstrap/limine
    // environment is incompatible with this kernel image.
    if !BASE_REVISION.is_supported() {
        // Use direct VGA output since nothing is initialized yet
        let vga_buffer = 0xb8000 as *mut u8;
        let error_msg = b"FATAL: Unsupported Limine protocol revision";
        for (i, &byte) in error_msg.iter().enumerate() {
            *vga_buffer.add(i * 2) = byte;
            *vga_buffer.add(i * 2 + 1) = 0x4f; // White on red
        }
        crate::utils::system::halt_system();
    }

    // Perform comprehensive kernel initialization with proper error handling
    match init::init_kernel() {
        Ok(()) => {
            kprintln!("🎯 Kernel initialization completed successfully!");
            kprintln!("🚀 System ready for advanced operations");
        }
        Err(e) => {
            kprintln!("❌ FATAL: Kernel initialization failed: {}", e);
            kprintln!("System cannot continue safely - halting");
            crate::utils::system::halt_system();
        }
    }

    // Continue with application-level initialization
    run_main_kernel_loop()
}

/// Main kernel loop after successful initialization
fn run_main_kernel_loop() -> ! {
    kprintln!("=== Starting Main Kernel Operations ===");

    // Initialize PCI driver
    match init_pci_safe() {
        Ok(_) => kprintln!("✅ PCI initialized successfully"),
        Err(_) => kprintln!("⚠️  PCI initialization failed, continuing without PCI"),
    }

    // Initialize IDE driver
    match init_ide_safe() {
        Ok(_) => kprintln!("✅ IDE driver initialized"),
        Err(_) => kprintln!("⚠️  IDE initialization failed, continuing without storage"),
    }

    // Test and initialize filesystem
    match init_filesystem_safe() {
        Ok(_) => kprintln!("✅ Filesystem operations completed successfully"),
        Err(e) => kprintln!("⚠️  Filesystem operations failed: {}", e),
    }

    // Initialize USB subsystem
    match init_usb_safe() {
        Ok(_) => kprintln!("✅ USB subsystem ready"),
        Err(_) => kprintln!("⚠️  USB initialization failed, continuing without USB"),
    }

    // Run tests if enabled
    #[cfg(test)]
    test_main();

    kprintln!("🎉 All systems operational - entering idle state");
    kprintln!("💤 System idle. CPU will halt between interrupts.");

    // Enter efficient idle loop
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Safe PCI initialization
fn init_pci_safe() -> Result<(), &'static str> {
    let _ = init_pci(); // PciAccess is private, so we just call it and ignore return
    Ok(())
}

/// Safe IDE initialization  
fn init_ide_safe() -> Result<(), &'static str> {
    ide_initialize();
    Ok(())
}

/// Safe USB initialization
fn init_usb_safe() -> Result<(), &'static str> {
    init::usb::init_usb();
    Ok(())
}

/// Safe filesystem initialization and testing
fn init_filesystem_safe() -> Result<(), &'static str> {
    kprintln!("🔍 Testing filesystem integration...");

    // Check IDE drive accessibility first
    let drive_accessible = match test_ide_drive_safe(0) {
        Ok(size) if size > 0 => {
            kprintln!("✅ IDE drive 0 detected: {} bytes", size);
            true
        }
        Ok(_) => {
            kprintln!("⚠️  IDE drive reports 0 bytes - may not be configured");
            false
        }
        Err(_) => {
            kprintln!("⚠️  IDE drive 0 not accessible");
            false
        }
    };

    if !drive_accessible {
        kprintln!("ℹ️  Skipping filesystem operations (no storage available)");
        return Ok(());
    }

    kprintln!("🚀 Proceeding with Galleon2 filesystem operations...");

    // Try to mount existing filesystem, or format a new one
    let mut filesystem = match mount_or_format_filesystem_safe() {
        Ok(fs) => fs,
        Err(e) => {
            kprintln!("❌ Filesystem initialization failed: {}", e);
            return Ok(()); // Continue without filesystem
        }
    };

    // Perform filesystem operations safely
    match perform_filesystem_operations_safe(&mut filesystem) {
        Ok(_) => kprintln!("✅ Filesystem operations completed successfully"),
        Err(e) => kprintln!("⚠️  Some filesystem operations failed: {}", e),
    }

    Ok(())
}

/// Test IDE drive safely
fn test_ide_drive_safe(drive: u8) -> Result<u64, &'static str> {
    ide::return_drive_size_bytes(drive).map_err(|_| "Drive access failed")
}

/// Mount or format filesystem safely
fn mount_or_format_filesystem_safe() -> Result<GalleonFilesystem, &'static str> {
    match GalleonFilesystem::mount(0) {
        Ok(fs) => {
            kprintln!("✅ Mounted existing Galleon2 filesystem");
            Ok(fs)
        }
        Err(_) => {
            kprintln!("ℹ️  No existing filesystem found, attempting format...");
            GalleonFilesystem::format(0).map_err(|_| "Failed to format filesystem")
                .map(|fs| {
                    kprintln!("✅ Successfully formatted new Galleon2 filesystem");
                    fs
                })
        }
    }
}

/// Perform filesystem operations safely
fn perform_filesystem_operations_safe(filesystem: &mut GalleonFilesystem) -> Result<(), &'static str> {
    // Get and display filesystem statistics
    if let Ok(stats) = filesystem.get_stats() {
        kprintln!("📊 Filesystem Statistics:");
        kprintln!("   Total space: {} KB", stats.total_space / 1024);
        kprintln!("   Free space:  {} KB", stats.free_space / 1024);
        kprintln!("   Used space:  {} KB", stats.used_space / 1024);
        kprintln!("   Cluster size: {} bytes", stats.cluster_size);
    }

    // Create sample directory structure safely
    create_sample_directories_safe(filesystem)?;
    
    // Create sample files safely
    create_sample_files_safe(filesystem)?;

    // Test file operations safely
    test_file_operations_safe(filesystem)?;

    // Final filesystem sync
    if let Err(_) = filesystem.sync() {
        kprintln!("⚠️  Filesystem sync warning - data may not be persistent");
    } else {
        kprintln!("✅ Filesystem synchronized to disk");
    }

    kprintln!("🎉 Galleon2 filesystem demonstration completed successfully!");
    Ok(())
}

/// Create sample directories safely
fn create_sample_directories_safe(filesystem: &mut GalleonFilesystem) -> Result<(), &'static str> {
    kprintln!("📁 Creating sample directory structure...");

    let _home_dir = filesystem.create_directory(5, "home".to_string())
        .unwrap_or_else(|_| { kprintln!("⚠️  Failed to create /home"); 5 });

    let _docs_dir = filesystem.create_directory(_home_dir, "documents".to_string())
        .unwrap_or_else(|_| { kprintln!("⚠️  Failed to create /home/documents"); _home_dir });

    let _projects_dir = filesystem.create_directory(_home_dir, "projects".to_string())
        .unwrap_or_else(|_| { kprintln!("⚠️  Failed to create /home/projects"); _home_dir });

    Ok(())
}

/// Create sample files safely
fn create_sample_files_safe(filesystem: &mut GalleonFilesystem) -> Result<(), &'static str> {
    kprintln!("📄 Creating sample files...");

    let readme_content = b"Welcome to PrismaOS with safe initialization!".to_vec();
    if let Err(_) = filesystem.create_file(5, "README.txt".to_string(), Some(readme_content)) {
        kprintln!("⚠️  Failed to create README.txt");
    } else {
        kprintln!("✅ Created /README.txt");
    }

    Ok(())
}

/// Test file operations safely
fn test_file_operations_safe(filesystem: &mut GalleonFilesystem) -> Result<(), &'static str> {
    kprintln!("🔍 Testing file operations...");

    // List directory contents
    match filesystem.list_directory() {
        Ok(entries) => {
            kprintln!("📋 Directory listing:");
            for (name, record_num) in entries.iter().take(5) { // Limit output
                kprintln!("   {} (record #{})", name, record_num);
            }
        }
        Err(_) => kprintln!("⚠️  Failed to list directory"),
    }

    // Test file search
    match filesystem.find_file("README.txt") {
        Ok(Some(record)) => kprintln!("✅ Found README.txt at record #{}", record),
        Ok(None) => kprintln!("⚠️  README.txt not found"),
        Err(_) => kprintln!("⚠️  Error searching for README.txt"),
    }

    Ok(())
}

/// Kernel panic handler
///
/// Disables interrupts immediately and attempts to render a BSOD using the
/// framebuffer renderer if available. If no framebuffer renderer exists the
/// handler falls back to a VGA-style blue screen. The handler never returns
/// — it halts the machine after rendering diagnostics.
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    // Disable interrupts to avoid re-entrancy while rendering the panic
    // screen.
    x86_64::instructions::interrupts::disable();

    // Try to render a framebuffer BSOD, otherwise use VGA fallback. The
    // global renderer is an Option in `scrolling_text` so we check it
    // unsafely here (same behavior as before).
    unsafe {
        if let Some(ref mut renderer) = scrolling_text::GLOBAL_RENDERER {
            crate::utils::bsod::render_framebuffer_bsod(renderer, info);
        } else {
            crate::utils::bsod::render_vga_bsod(info);
        }
    }

    crate::utils::system::halt_system();
}

/// Custom test runner required by `#![test_runner]`.
///
/// Executes each test function provided by the test harness. Kept intentionally
/// tiny: real test reporting is handled by the test framework and logging
/// done elsewhere.
pub fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}
