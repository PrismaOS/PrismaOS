//! Basic ELF parsing example
//!
//! This example demonstrates how to parse an ELF file and extract basic information
//! about its structure, headers, and sections.

extern crate alloc;

use statue::{ElfFile, ElfError, Result};
use statue::program::ProgramHeaderIter;
use statue::section::SectionHeaderIter;

/// Example ELF data (minimal ELF header for demonstration)
const MINIMAL_ELF: &[u8] = &[
    0x7f, 0x45, 0x4c, 0x46,  // ELF magic
    0x02,                     // 64-bit
    0x01,                     // Little endian
    0x01,                     // ELF version
    0x00,                     // System V ABI
    0x00, 0x00, 0x00, 0x00,   // ABI version + padding
    0x00, 0x00, 0x00, 0x00,   // More padding
    // ... rest would be a complete ELF file
];

/// Parse and analyze an ELF file
fn analyze_elf(data: &[u8]) -> Result<()> {
    // Parse the ELF file
    let elf = ElfFile::parse(data)?;
    let header = elf.header();

    // Print basic information
    println!("=== ELF File Analysis ===");
    println!("Class: {:?}", header.ident.class);
    println!("Data: {:?}", header.ident.data);
    println!("Version: {}", header.ident.version);
    println!("Type: {:?}", header.file_type);
    println!("Machine: {:?}", header.machine);
    println!("Entry point: 0x{:016x}", header.entry);

    // Analyze program headers
    println!("\n=== Program Headers ===");
    let program_headers = ProgramHeaderIter::new(&elf)?;
    println!("Number of program headers: {}", program_headers.len());

    for (i, ph_result) in program_headers.enumerate() {
        let ph = ph_result?;
        println!("Program Header {}:", i);
        println!("  Type: {:?}", ph.segment_type);
        println!("  Virtual Address: 0x{:016x}", ph.vaddr);
        println!("  File Size: {} bytes", ph.filesz);
        println!("  Memory Size: {} bytes", ph.memsz);
        println!("  Flags: R{} W{} X{}",
                 if ph.flags.readable() { "+" } else { "-" },
                 if ph.flags.writable() { "+" } else { "-" },
                 if ph.flags.executable() { "+" } else { "-" });
    }

    // Analyze section headers
    println!("\n=== Section Headers ===");
    let section_headers = SectionHeaderIter::new(&elf)?;
    println!("Number of section headers: {}", section_headers.len());

    for (i, sh_result) in section_headers.enumerate() {
        let sh = sh_result?;
        println!("Section Header {}:", i);
        println!("  Type: {:?}", sh.section_type);
        println!("  Address: 0x{:016x}", sh.addr);
        println!("  Size: {} bytes", sh.size);
        println!("  Flags: A{} W{} X{}",
                 if sh.flags.alloc() { "+" } else { "-" },
                 if sh.flags.writable() { "+" } else { "-" },
                 if sh.flags.executable() { "+" } else { "-" });
    }

    Ok(())
}

/// Example of error handling
fn demonstrate_error_handling() {
    println!("\n=== Error Handling Examples ===");

    // Test with invalid data
    let invalid_data = &[0x00, 0x01, 0x02, 0x03];
    match ElfFile::parse(invalid_data) {
        Err(ElfError::InvalidMagic) => {
            println!("✓ Correctly detected invalid ELF magic");
        }
        Err(e) => {
            println!("Unexpected error: {:?}", e);
        }
        Ok(_) => {
            println!("ERROR: Should have failed with invalid magic");
        }
    }

    // Test with insufficient data
    let short_data = &[0x7f, 0x45, 0x4c];
    match ElfFile::parse(short_data) {
        Err(ElfError::BufferTooSmall) => {
            println!("✓ Correctly detected insufficient data");
        }
        Err(e) => {
            println!("Unexpected error: {:?}", e);
        }
        Ok(_) => {
            println!("ERROR: Should have failed with buffer too small");
        }
    }

    // Test with valid magic but invalid class
    let invalid_class = &[
        0x7f, 0x45, 0x4c, 0x46,  // Valid ELF magic
        0x03,                     // Invalid class
        0x01, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    match ElfFile::parse(invalid_class) {
        Err(ElfError::UnsupportedClass) => {
            println!("✓ Correctly detected unsupported class");
        }
        Err(e) => {
            println!("Unexpected error: {:?}", e);
        }
        Ok(_) => {
            println!("ERROR: Should have failed with unsupported class");
        }
    }
}

/// Demonstrate validation features
fn demonstrate_validation() {
    println!("\n=== Validation Examples ===");

    // Note: In a real implementation, you would have actual ELF files to test
    // This example shows the validation approach

    println!("Statue provides comprehensive validation:");
    println!("- Magic number verification");
    println!("- Architecture compatibility checks");
    println!("- Endianness validation");
    println!("- Header size consistency");
    println!("- Segment and section bounds checking");
    println!("- Address alignment verification");
    println!("- String table integrity");
}

/// Main example function
fn main() {
    println!("Statue ELF Parsing Example");
    println!("=========================");

    // Note: MINIMAL_ELF is not a complete ELF file, so this will fail
    // In a real example, you would use actual ELF file data
    match analyze_elf(MINIMAL_ELF) {
        Ok(()) => {
            println!("Analysis completed successfully!");
        }
        Err(e) => {
            println!("Analysis failed: {}", e.description());
            println!("This is expected with the minimal test data");
        }
    }

    // Demonstrate error handling
    demonstrate_error_handling();

    // Show validation features
    demonstrate_validation();
}

/// Example of working with real ELF data
/// This function would be used with actual ELF files
#[allow(dead_code)]
fn analyze_real_elf_file(file_data: &[u8]) -> Result<()> {
    let elf = ElfFile::parse(file_data)?;

    println!("Real ELF Analysis:");
    println!("Entry point: 0x{:x}", elf.header().entry);

    // Count loadable segments
    let program_headers = ProgramHeaderIter::new(&elf)?;
    let loadable_count = program_headers
        .filter_map(|ph| ph.ok())
        .filter(|ph| ph.is_loadable())
        .count();
    println!("Loadable segments: {}", loadable_count);

    // Find text section
    let section_headers = SectionHeaderIter::new(&elf)?;
    for (i, sh_result) in section_headers.enumerate() {
        let sh = sh_result?;
        if sh.flags.executable() && sh.flags.alloc() {
            println!("Executable section {} at 0x{:x}", i, sh.addr);
        }
    }

    Ok(())
}