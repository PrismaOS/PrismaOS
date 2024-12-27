use std::path::PathBuf;
use std::env;

fn main() {
    // Get the project root directory
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    
    // Find the kernel binary
    let target_dir = manifest_dir.join("target/x86_64-prisma/debug");
    
    // List all files in the debug directory to find the kernel binary
    let kernel_binary_path = if let Ok(entries) = std::fs::read_dir(&target_dir) {
        let mut kernel_path = None;
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        // Look for a file that starts with prisma_os- and isn't a debug info file
                        if name_str.starts_with("prisma_os-") && !name_str.ends_with(".d") {
                            kernel_path = Some(path);
                            break;
                        }
                    }
                }
            }
        }
        kernel_path
    } else {
        None
    };

    // Check if we found the kernel
    let kernel_binary_path = if let Some(path) = kernel_binary_path {
        println!("cargo:warning=Found kernel at: {}", path.display());
        path
    } else {
        println!("cargo:warning=Could not find kernel binary in {:?}", target_dir);
        return;
    };

    // Create disk images directory
    let disk_dir = target_dir.join("disk_images");
    std::fs::create_dir_all(&disk_dir).unwrap();
    
    let bios_path = disk_dir.join("prisma-os-bios.img");
    
    println!("cargo:warning=Creating disk image at: {}", bios_path.display());
    
    // Create the disk image
    match bootloader::BiosBoot::new(&kernel_binary_path).create_disk_image(&bios_path) {
        Ok(_) => println!("cargo:warning=Successfully created disk image at {}", bios_path.display()),
        Err(e) => println!("cargo:warning=Failed to create disk image: {}", e),
    }

    // Tell cargo to rerun if kernel changes
    println!("cargo:rerun-if-changed={}", kernel_binary_path.display());
}