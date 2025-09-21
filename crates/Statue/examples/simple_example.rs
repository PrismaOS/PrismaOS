//! Simple ELF parsing example that compiles without issues

use statue::{ElfFile, ElfError};

/// Simple example demonstrating ELF parsing
fn main() {
    // Example ELF header bytes (minimal)
    let minimal_elf_header = [
        0x7f, 0x45, 0x4c, 0x46,  // ELF magic
        0x02,                     // 64-bit
        0x01,                     // Little endian
        0x01,                     // ELF version
        0x00,                     // System V ABI
        0x00, 0x00, 0x00, 0x00,   // ABI version + padding
        0x00, 0x00, 0x00, 0x00,   // More padding
        // This is incomplete but demonstrates the concept
    ];

    // Try to parse (will fail due to incomplete header, demonstrating error handling)
    match ElfFile::parse(&minimal_elf_header) {
        Ok(elf) => {
            // This won't happen with our minimal data
            let header = elf.header();
            eprintln!("Successfully parsed ELF file");
            eprintln!("Entry point: 0x{:x}", header.entry);
        }
        Err(ElfError::BufferTooSmall) => {
            eprintln!("Expected error: Buffer too small for complete ELF header");
        }
        Err(e) => {
            eprintln!("Parsing failed with error: {}", e.description());
        }
    }

    // Demonstrate error types
    let invalid_magic = [0x00, 0x01, 0x02, 0x03];
    match ElfFile::parse(&invalid_magic) {
        Err(ElfError::InvalidMagic) => {
            eprintln!("Expected error: Invalid ELF magic detected");
        }
        _ => {
            eprintln!("Unexpected result");
        }
    }

    eprintln!("Example completed successfully");
}