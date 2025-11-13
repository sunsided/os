//! # ACPI (Advanced Configuration and Power Interface) Support
//!
//! This crate provides foundational support for parsing and interpreting ACPI
//! (Advanced Configuration and Power Interface) structures in the kernel. ACPI
//! is a critical system interface that enables operating systems to discover
//! and configure hardware components, manage power states, and interact with
//! platform-specific features.
//!
//! ## Overview
//!
//! ACPI defines a standardized interface between the operating system and the
//! platform firmware (BIOS/UEFI). This crate focuses on the early discovery
//! phase, parsing the fundamental ACPI data structures needed to bootstrap
//! the full ACPI subsystem.
//!
//! ## Architecture
//!
//! The ACPI specification defines a hierarchical table structure:
//!
//! ```text
//! UEFI/BIOS Firmware
//!     ↓
//! RSDP/XSDP (Root System Description Pointer)
//!     ↓
//! RSDT/XSDT (Root/Extended System Description Table)
//!     ↓
//! Individual ACPI Tables (FADT, MADT, DSDT, SSDT, etc.)
//! ```
//!
//! ## Key Components
//!
//! ### Physical Memory Mapping ([`PhysMapRo`])
//! Abstract trait for mapping physical memory regions into the kernel's
//! virtual address space. This allows the ACPI parser to access firmware
//! tables regardless of the underlying memory management implementation.
//!
//! ### RSDP Discovery and Parsing ([`rsdp`])
//! * **RSDP (ACPI 1.0)**: 20-byte Root System Description Pointer
//! * **XSDP (ACPI 2.0+)**: Extended version supporting 64-bit addresses
//! * **Validation**: Checksum verification and signature validation
//! * **Version Detection**: Automatic handling of ACPI 1.0 vs 2.0+ variants
//!
//! ## ACPI Version Support
//!
//! ### ACPI 1.0 Support
//! * **RSDP Structure**: 20-byte legacy format
//! * **Address Space**: 32-bit physical addresses only
//! * **RSDT**: Root System Description Table with 32-bit pointers
//! * **Checksum**: Simple 8-bit checksum of first 20 bytes
//!
//! ### ACPI 2.0+ Support
//! * **XSDP Structure**: Extended format with length field
//! * **Address Space**: 64-bit physical addresses supported
//! * **XSDT**: Extended System Description Table with 64-bit pointers
//! * **Extended Checksum**: Checksum covers entire table structure
//!
//! ## Discovery Process
//!
//! 1. **Firmware Handoff**: UEFI provides RSDP address in configuration table
//! 2. **Signature Validation**: Verify "RSD PTR " signature at provided address
//! 3. **Checksum Verification**: Validate table integrity via checksum
//! 4. **Version Detection**: Determine ACPI 1.0 vs 2.0+ based on revision field
//! 5. **Address Extraction**: Extract RSDT/XSDT addresses for further parsing
//!
//! ## Memory Management Integration
//!
//! The crate uses the [`PhysMapRo`] trait to abstract physical memory access,
//! enabling integration with various memory management strategies:
//!
//! * **Identity Mapping**: Direct physical-to-virtual mapping
//! * **Higher Half Direct Mapping (HHDM)**: Fixed offset translation
//! * **Temporary Mapping**: Map-use-unmap pattern for limited virtual space
//! * **UEFI Runtime Services**: Use firmware-provided mapping functions
//!
//! ## Safety Considerations
//!
//! ACPI parsing involves extensive unsafe operations due to:
//!
//! ### Firmware Interface
//! * **Untrusted Data**: Firmware-provided addresses and structures
//! * **Memory Layout**: Platform-specific physical memory organization
//! * **Packed Structures**: Direct hardware structure interpretation
//!
//! ### Validation Strategy
//! * **Signature Checking**: Verify expected magic bytes in all structures
//! * **Checksum Validation**: Mathematical integrity verification
//! * **Bounds Checking**: Validate structure sizes and field ranges
//! * **Defensive Programming**: Assume firmware data may be malformed
//!
//! ## Usage Patterns
//!
//! ### Basic RSDP Discovery
//! ```rust,no_run
//! use kernel_acpi::{PhysMapRo, rsdp::AcpiRoots};
//!
//! struct MyMapper;
//! impl PhysMapRo for MyMapper {
//!     unsafe fn map_ro<'a>(&self, paddr: u64, len: usize) -> &'a [u8] {
//!         // Implementation-specific mapping
//!         # unimplemented!()
//!     }
//! }
//!
//! let mapper = MyMapper;
//! let rsdp_addr = 0x12345000; // From UEFI configuration table
//!
//! if let Some(roots) = unsafe { AcpiRoots::parse(&mapper, rsdp_addr) } {
//!     if let Some(xsdt) = roots.xsdt_addr {
//!         println!("ACPI 2.0+ detected, XSDT at 0x{:x}", xsdt);
//!     } else if let Some(rsdt) = roots.rsdt_addr {
//!         println!("ACPI 1.0 detected, RSDT at 0x{:x}", rsdt);
//!     }
//! }
//! ```
//!
//! ## Future Extensions
//!
//! This foundational crate enables future ACPI functionality:
//! * **Table Enumeration**: Parse RSDT/XSDT to discover available tables
//! * **Specific Parsers**: FADT, MADT, MCFG, and other standard tables
//! * **AML Interpreter**: ACPI Machine Language execution engine
//! * **Power Management**: ACPI power state and thermal management
//! * **Device Enumeration**: PCI Express configuration and device discovery
//!
//! ## Standards Compliance
//!
//! Implementation follows the ACPI specification:
//! * **ACPI 1.0**: Legacy 32-bit support for older systems
//! * **ACPI 2.0+**: Modern 64-bit support with extended features
//! * **UEFI Integration**: Proper discovery via UEFI configuration tables
//! * **Checksum Algorithms**: Specification-compliant validation methods

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code)]

pub mod rsdp;

/// Map a physical region and return a *read-only* byte slice for its contents.
/// You provide the implementation (identity map, kmap, etc.).
pub trait PhysMapRo {
    /// # Safety
    /// The implementor must ensure the returned slice is valid for `len` bytes.
    unsafe fn map_ro<'a>(&self, paddr: u64, len: usize) -> &'a [u8];
}

fn sum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |a, &b| a.wrapping_add(b))
}
