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
    assert!(BASE_REVISION.is_supported());

    // Perform the heavy lifting in the `init` module so this entrypoint
    // remains compact and easy to audit. We keep behavior identical to the
    // previous implementation: if initialization fails, halt the system.
    match init::init_kernel() {
        Ok(()) => { /* Initialization completed successfully. */ }
        Err(e) => {
            kprintln!("ERROR: Kernel initialization failed: {}", e);
            crate::utils::system::halt_system();
        }
    }

    // Init PCI dirver
    kprintln!("PciAccess {:?}", init_pci());

    // Init IDE driver
    // TODO: Determine if an IDE drive is present before initializing
    kprintln!("Initializing IDE driver...");
    ide_initialize();
    kprintln!("IDE driver initialized.");

    // Initialize the advanced Galleon2 filesystem with comprehensive error handling
    kprintln!("Testing filesystem integration...");

    // First, test basic IDE functionality
    kprintln!("Checking IDE drive accessibility...");
    let drive_accessible = match ide::return_drive_size_bytes(0) {
        Ok(size) => {
            kprintln!("✓ IDE drive 0 detected: {} bytes", size);
            if size == 0 {
                kprintln!("Drive reports 0 bytes - may not be properly configured");
                false
            } else {
                true
            }
        }
        Err(e) => {
            kprintln!("✗ IDE drive 0 not accessible: {:?}", e);
            false
        }
    };

    if !drive_accessible {
        kprintln!("Skipping advanced filesystem (no storage available)");
        kprintln!("Continuing with basic kernel functionality...");

        // Initialize USB subsystem
        init::usb::init_usb();

        #[cfg(test)]
        test_main();

        kprintln!("System idle. Halting CPU...");

        // Enter an infinite HALT loop.
        loop {
            core::arch::asm!("hlt");
        }
    }

    kprintln!("Proceeding with Galleon2 filesystem initialization...");

    // Try to mount existing filesystem, or format a new one
    let mut filesystem = match GalleonFilesystem::mount(0) {
        Ok(fs) => {
            kprintln!("✓ Mounted existing Galleon2 filesystem on drive 0");
            fs
        }
        Err(mount_error) => {
            kprintln!("No existing filesystem found: {:?}", mount_error);
            kprintln!("Attempting to format drive 0...");
            match GalleonFilesystem::format(0) {
                Ok(fs) => {
                    kprintln!("✓ Successfully formatted drive 0 with Galleon2 filesystem");
                    fs
                }
                Err(format_error) => {
                    kprintln!("✗ Failed to format filesystem: {:?}", format_error);
                    kprintln!("Continuing without filesystem (storage may not be available)");
                    // Continue with rest of kernel initialization instead of halting
                    // Initialize USB subsystem
                    init::usb::init_usb();

                    #[cfg(test)]
                    test_main();

                    kprintln!("System idle. Halting CPU...");

                    // Enter an infinite HALT loop.
                    loop {
                        core::arch::asm!("hlt");
                    }
                }
            }
        }
    };

    // Wrap all filesystem operations in error handling to prevent panics
    let filesystem_operations_result = (|| -> Result<(), galleon2::FilesystemError> {
        // Get filesystem statistics
        match filesystem.get_stats() {
            Ok(stats) => {
                kprintln!("Filesystem Statistics:");
                kprintln!("   Total space: {} KB", stats.total_space / 1024);
                kprintln!("   Free space:  {} KB", stats.free_space / 1024); // TODO: Why do we get a hardware fault here? @ghostedgaming
                kprintln!("   Used space:  {} KB", stats.used_space / 1024);
                kprintln!("   Cluster size: {} bytes", stats.cluster_size);
                kprintln!("   Total clusters: {}", stats.total_clusters);
            }
            Err(e) => {
                kprintln!("Could not get filesystem stats: {:?}", e);
                return Err(e);
            }
        }

        // Create sample directory structure
        kprintln!("Creating sample file structure...");

    // Create directories
    let home_dir = match filesystem.create_directory(5, "home".to_string()) {
        Ok(dir) => {
            kprintln!("✓ Created directory: /home");
            dir
        }
        Err(e) => {
            kprintln!("✗ Failed to create /home: {:?}", e);
            5 // fallback to root
        }
    };

    let docs_dir = match filesystem.create_directory(home_dir, "documents".to_string()) {
        Ok(dir) => {
            kprintln!("✓ Created directory: /home/documents");
            dir
        }
        Err(e) => {
            kprintln!("✗ Failed to create /home/documents: {:?}", e);
            home_dir
        }
    };

    let projects_dir = match filesystem.create_directory(home_dir, "projects".to_string()) {
        Ok(dir) => {
            kprintln!("✓ Created directory: /home/projects");
            dir
        }
        Err(e) => {
            kprintln!("✗ Failed to create /home/projects: {:?}", e);
            home_dir
        }
    };

    // Create sample files
    kprintln!("Creating sample files...");

    // Create a simple text file
    let readme_content = b"Welcome to PrismaOS!\n\nThis is a demonstration of the advanced Galleon2 filesystem.\nFeatures:\n- NTFS-like architecture\n- Transaction journaling\n- B+ tree indexing\n- Extent-based allocation\n- Crash recovery\n\nBuilt with Rust for maximum safety and performance.".to_vec();
    match filesystem.create_file(5, "README.txt".to_string(), Some(readme_content)) {
        Ok(_) => kprintln!("✓ Created file: /README.txt"),
        Err(e) => kprintln!("✗ Failed to create README.txt: {:?}", e),
    }

    // Create a config file in documents
    let config_content = b"[system]\nversion=1.0\nkernel=PrismaOS\nfilesystem=Galleon2\n\n[features]\njournaling=enabled\ncompression=disabled\nencryption=disabled".to_vec();
    match filesystem.create_file(docs_dir, "config.ini".to_string(), Some(config_content)) {
        Ok(_) => kprintln!("✓ Created file: /home/documents/config.ini"),
        Err(e) => kprintln!("✗ Failed to create config.ini: {:?}", e),
    }

    // Create a source code file in projects
    let source_content = b"// PrismaOS Kernel Module\n// Advanced filesystem demonstration\n\nuse galleon2::GalleonFilesystem;\n\nfn main() {\n    println!(\"Hello from PrismaOS!\");\n    // Demonstrate filesystem operations\n    let fs = GalleonFilesystem::mount(0).unwrap();\n    println!(\"Filesystem mounted successfully!\");\n}".to_vec();
    match filesystem.create_file(projects_dir, "demo.rs".to_string(), Some(source_content)) {
        Ok(_) => kprintln!("✓ Created file: /home/projects/demo.rs"),
        Err(e) => kprintln!("✗ Failed to create demo.rs: {:?}", e),
    }

    // Create a large file to demonstrate extent allocation
    let large_content = alloc::vec![0x42u8; 8192]; // 8KB file to test multi-cluster allocation
    match filesystem.create_file(projects_dir, "largefile.bin".to_string(), Some(large_content)) {
        Ok(_) => kprintln!("✓ Created file: /home/projects/largefile.bin (8KB)"),
        Err(e) => kprintln!("✗ Failed to create largefile.bin: {:?}", e),
    }

    // List root directory contents
    kprintln!("\n📋 Root directory listing:");
    match filesystem.list_directory() {
        Ok(entries) => {
            for (name, record_num) in entries {
                kprintln!("   {} (record #{})", name, record_num);
            }
        }
        Err(e) => kprintln!("✗ Failed to list directory: {:?}", e),
    }

    // Demonstrate file reading
    kprintln!("Reading back created files...");

    // Read and display README.txt
    if let Ok(Some(readme_record)) = filesystem.find_file("README.txt") {
        match filesystem.read_file(readme_record) {
            Ok(content) => {
                if let Ok(text) = alloc::string::String::from_utf8(content) {
                    kprintln!("Contents of README.txt:");
                    kprintln!("─────────────────────────────");
                    // Print first few lines
                    for (i, line) in text.lines().enumerate() {
                        if i < 5 {
                            kprintln!("   {}", line);
                        } else if i == 5 {
                            kprintln!("   ... ({} more lines)", text.lines().count() - 5);
                            break;
                        }
                    }
                    kprintln!("─────────────────────────────");
                } else {
                    kprintln!("✗ README.txt contains invalid UTF-8");
                }
            }
            Err(e) => kprintln!("✗ Failed to read README.txt: {:?}", e),
        }
    }

    // Demonstrate file search
    kprintln!("\n🔍 File search demonstration:");
    let search_files = ["README.txt", "config.ini", "demo.rs", "nonexistent.txt"];
    for filename in &search_files {
        match filesystem.find_file(filename) {
            Ok(Some(record)) => kprintln!("✓ Found '{}' at record #{}", filename, record),
            Ok(None) => kprintln!("✗ File '{}' not found", filename),
            Err(e) => kprintln!("✗ Error searching for '{}': {:?}", filename, e),
        }
    }

    // Final filesystem sync
    match filesystem.sync() {
        Ok(()) => kprintln!("Filesystem synchronized to disk"),
        Err(e) => kprintln!("Filesystem sync warning: {:?}", e),
    }

        kprintln!("\n🎉 Advanced Galleon2 filesystem demonstration completed!");

        Ok(())
    })();

    // Handle filesystem operation results
    match filesystem_operations_result {
        Ok(()) => kprintln!("All filesystem operations completed successfully"),
        Err(e) => kprintln!("Filesystem operations failed: {:?}", e),
    }

    // Initialize USB subsystem
    init::usb::init_usb();

    #[cfg(test)]
    test_main();

    kprintln!("System idle. Halting CPU...");

    // Enter an infinite HALT loop.
    loop {
        core::arch::asm!("hlt");
    }
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
