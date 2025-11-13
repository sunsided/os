//! # Kernel Configuration and Boot Interface
//!
//! This crate defines the core data structures, memory layout constants, and
//! boot interface contracts that govern the kernel's initialization and runtime
//! operation. It serves as the authoritative source for system-wide configuration
//! and provides stable ABI definitions for communication between the bootloader
//! and kernel.
//!
//! ## Overview
//!
//! The kernel requires precise coordination between multiple components during
//! boot and runtime operation. This crate centralizes critical configuration
//! information that must be shared across the bootloader, kernel, and various
//! kernel subsystems, ensuring consistency and preventing configuration drift.
//!
//! ## Architecture
//!
//! The crate is organized into two primary modules:
//!
//! ### Boot Information ([`boot`])
//! Defines the bootloader-to-kernel handoff interface:
//! * **Kernel Entry Point**: Function signature and calling convention
//! * **Boot Data Structures**: Memory map, ACPI information, framebuffer details
//! * **ABI Stability**: C-compatible structures for cross-component communication
//! * **UEFI Integration**: Direct compatibility with UEFI GOP and memory services
//!
//! ### Memory Layout ([`memory`])
//! Establishes the kernel's virtual memory architecture:
//! * **Address Space Layout**: User/kernel space boundaries and reserved regions
//! * **Higher Half Design**: Kernel execution at high virtual addresses
//! * **Physical Memory Mapping**: HHDM (Higher Half Direct Mapping) configuration
//! * **Stack and Code Placement**: Compile-time memory organization
//!
//! ## Virtual Memory Architecture
//!
//! The kernel employs a higher-half design with carefully planned address space layout:
//!
//! ```text
//! Virtual Address Space Layout (64-bit):
//!
//! 0x0000_0000_0000_0000 ┌─────────────────────────────────┐
//!                       │                                 │
//!                       │         User Space              │
//!                       │    (Applications & Libraries)   │
//!                       │                                 │
//! LAST_USERSPACE_ADDRESS├─────────────────────────────────┤ 0xffff_efff_ffff_ffff
//!                       │        Guard Region             │
//! HHDM_BASE             ├─────────────────────────────────┤ 0xffff_8880_0000_0000
//!                       │   Higher Half Direct Mapping    │
//!                       │   (Physical Memory Access)      │
//! KERNEL_BASE           ├─────────────────────────────────┤ 0xffff_ffff_8000_0000
//!                       │       Kernel Text & Data        │
//!                       │     (Kernel Executable)         │
//! 0xFFFF_FFFF_FFFF_FFFF └─────────────────────────────────┘
//! ```
//!
//! ### Design Principles
//! * **Canonical Addressing**: All kernel addresses use the canonical higher half
//! * **Guard Regions**: Large unmapped areas prevent accidental user/kernel overlap
//! * **Direct Mapping**: HHDM enables efficient physical memory access
//! * **Fixed Layout**: Compile-time constants enable static optimization
//!
//! ## Boot Protocol
//!
//! The bootloader-to-kernel handoff follows a well-defined protocol:
//!
//! ### Entry Point Convention
//! ```rust
//! # use kernel_info::boot::KernelBootInfo;
//! pub type KernelEntryFn = extern "win64" fn(*const KernelBootInfo) -> !;
//! ```
//!
//! * **Calling Convention**: Windows x64 ABI for UEFI compatibility
//! * **Parameter**: Single pointer to boot information structure
//! * **No Return**: Kernel assumes control permanently
//!
//! ### Boot Information Structure
//! The [`KernelBootInfo`](boot::KernelBootInfo) structure provides:
//! * **Memory Map**: UEFI memory map for physical memory management
//! * **ACPI Root**: RSDP address for hardware discovery
//! * **Framebuffer**: GOP framebuffer for early graphics output
//!
//! ## Physical Memory Layout
//!
//! The kernel's physical memory placement is coordinated with the linker:
//!
//! ```text
//! Physical Memory Layout:
//! 0x0000_0000 ┌─────────────────────────────────┐
//!             │     Low Memory (< 1MiB)         │
//!             │  (BIOS, VGA, DMA buffers)       │
//! PHYS_LOAD   ├─────────────────────────────────┤ 0x0010_0000 (1 MiB)
//!             │       Kernel Image              │
//!             │   (Text, Data, BSS)             │
//!             ├─────────────────────────────────┤
//!             │    Available RAM                │
//!             │  (Managed by allocator)         │
//!             └─────────────────────────────────┘
//! ```
//!
//! * **Low Memory Avoidance**: Kernel loads at 1 MiB to avoid legacy conflicts
//! * **Identity Mapping**: Small identity region for paging setup
//! * **Linker Integration**: Constants used in kernel linker script
//!
//! ## Configuration Management
//!
//! ### Compile-Time Constants
//! All layout constants are `const` values computed at compile time:
//! * **Memory Safety**: Compile-time assertions prevent invalid configurations
//! * **Performance**: No runtime computation of layout information
//! * **Consistency**: Single source of truth for all kernel components
//!
//! ### Build Integration
//! The constants are consumed by the kernel's `build.rs`:
//! * **Linker Script Generation**: Dynamic linker script creation
//! * **Symbol Definition**: Automatic symbol generation for assembly
//! * **Validation**: Build-time verification of layout constraints
//!
//! ## ABI Compatibility
//!
//! All public structures maintain strict ABI compatibility:
//!
//! ### C Representation
//! * **`#[repr(C)]`**: Predictable memory layout for cross-language use
//! * **Fixed-Size Types**: Explicit integer sizes for platform independence
//! * **No Rust Enums**: Simple discriminated unions for ABI stability
//!
//! ### UEFI Integration
//! * **GOP Compatibility**: Direct mapping from UEFI graphics structures
//! * **Memory Map Format**: Native UEFI memory descriptor compatibility
//! * **Calling Conventions**: UEFI-compatible function signatures
//!
//! ## Safety Guarantees
//!
//! This crate maintains several safety invariants:
//!
//! ### Memory Layout Safety
//! * **No Overlaps**: Compile-time verification of non-overlapping regions
//! * **Canonical Addresses**: All kernel addresses are in canonical form
//! * **Alignment Requirements**: Page and stack alignment guarantees
//!
//! ### ABI Safety
//! * **Stable Layouts**: No unexpected structure reorganization
//! * **Version Compatibility**: Backward-compatible structure evolution
//! * **No Unsafe Code**: Marked `#![deny(unsafe_code)]` for safety assurance
//!
//! ## Usage Patterns
//!
//! ### Build Script Integration
//! ```rust
//! // In build.rs
//! use kernel_info::memory::{KERNEL_BASE, PHYS_LOAD};
//!
//! println!("cargo:rustc-link-arg=--defsym=KERNEL_BASE={:#x}", KERNEL_BASE);
//! println!("cargo:rustc-link-arg=--defsym=PHYS_LOAD={:#x}", PHYS_LOAD);
//! ```
//!
//! ### Bootloader Integration
//! ```rust,ignore
//! use kernel_info::boot::{KernelBootInfo, KernelEntryFn};
//!
//! let boot_info = KernelBootInfo {
//!     mmap: /* memory map info */,
//!     rsdp_addr: /* ACPI root */,
//!     fb: /* framebuffer info */,
//! };
//!
//! let kernel_entry: KernelEntryFn = /* kernel entry point */;
//! kernel_entry(&boot_info); // Transfer control to kernel
//! ```
//!
//! This crate serves as the foundation for all kernel configuration, ensuring
//! consistency across the entire operating system implementation while maintaining
//! clear interfaces between system components.

#![cfg_attr(not(any(test, doctest)), no_std)]
#![deny(unsafe_code)]

pub mod boot;
pub mod memory;
