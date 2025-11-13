//! # UEFI Bootloader for Higher-Half x86-64 Kernel
//!
//! This UEFI application serves as the bootloader for the operating system kernel,
//! handling the complex transition from UEFI firmware environment to a fully
//! operational higher-half x86-64 kernel. It orchestrates ELF loading, memory
//! management setup, and the critical handoff to kernel execution.
//!
//! ## Overview
//!
//! Modern operating systems require sophisticated bootloaders that can:
//! - Load and parse complex executable formats
//! - Configure virtual memory management
//! - Gather system information from firmware
//! - Safely transition between execution environments
//!
//! This UEFI loader implements all these capabilities while maintaining compatibility
//! with the UEFI specification and providing a robust foundation for kernel startup.
//!
//! ## Boot Process Architecture
//!
//! The bootloader follows a carefully orchestrated sequence to transition from
//! UEFI firmware control to autonomous kernel operation:
//!
//! ```text
//! UEFI Firmware Boot
//!         ↓
//! ┌─────────────────────────────────────────────┐
//! │              UEFI Loader                    │
//! ├─────────────────────────────────────────────┤
//! │  1. Environment Setup                       │
//! │     • Initialize logging and allocator      │
//! │     • Configure UEFI boot services          │
//! │  2. Kernel Loading                          │
//! │     • Parse kernel.elf file                 │
//! │     • Load PT_LOAD segments to memory       │
//! │     • Resolve higher-half addresses         │
//! │  3. System Discovery                        │
//! │     • Obtain framebuffer configuration      │
//! │     • Locate ACPI RSDP                      │
//! │     • Gather memory map information         │
//! │  4. Virtual Memory Setup                    │
//! │     • Create kernel page tables             │
//! │     • Map kernel segments at higher-half    │
//! │     • Establish HHDM mapping                │
//! │     • Set up trampoline stack               │
//! │  5. Environment Transition                  │
//! │     • Exit UEFI boot services               │
//! │     • Enable memory protection features     │
//! │     • Switch to kernel page tables          │
//! │  6. Kernel Handoff                          │
//! │     • Jump to kernel entry point            │
//! │     • Transfer boot information             │
//! └─────────────────────────────────────────────┘
//!         ↓
//! Kernel Execution (Higher-Half)
//! ```
//!
//! ## Key Components
//!
//! ### ELF Loading and Processing
//! * **File System Access**: Load `kernel.elf` from UEFI ESP filesystem
//! * **ELF64 Parsing**: Parse program headers and extract loadable segments
//! * **Higher-Half Mapping**: Resolve virtual addresses to higher-half locations
//! * **Physical Placement**: Load segments at appropriate physical addresses
//! * **Entry Point Resolution**: Extract kernel entry point for execution transfer
//!
//! ### Memory Management Setup
//! * **Page Table Creation**: Build complete x86-64 page table hierarchy
//! * **Kernel Segment Mapping**: Map all kernel segments with correct permissions
//! * **HHDM Establishment**: Create Higher Half Direct Mapping for physical access
//! * **Identity Mapping**: Maintain low-memory identity map for transition code
//! * **Protection Features**: Enable NX-bit, write protection, and global pages
//!
//! ### System Information Gathering
//! * **UEFI Memory Map**: Capture complete physical memory layout
//! * **Graphics Configuration**: Obtain GOP framebuffer details
//! * **ACPI Discovery**: Locate RSDP for hardware enumeration
//! * **Boot Information**: Package data for kernel consumption
//!
//! ### Transition Management
//! * **Boot Services Exit**: Safely terminate UEFI boot services
//! * **Trampoline Execution**: Execute page table switch code
//! * **Stack Setup**: Provide kernel with initial execution stack
//! * **Control Transfer**: Jump to kernel entry point with proper ABI
//!
//! ## Virtual Memory Layout
//!
//! The loader establishes a sophisticated virtual memory layout for the kernel:
//!
//! ```text
//! Virtual Address Space (Post-Transition):
//!
//! 0x0000_0000_0000_0000 ┌────────────────────────────────┐
//!                       │     Identity Mapped Region     │
//!                       │   (UEFI transition code)       │ 2 MiB
//! 0x0000_0000_0020_0000 ├────────────────────────────────┤
//!                       │        Unmapped Space          │
//!                       │                                │
//! HHDM_BASE             ├────────────────────────────────┤ 0xffff_8880_0000_0000
//!                       │   Higher Half Direct Mapping   │
//!                       │    (All Physical Memory)       │
//! KERNEL_BASE           ├────────────────────────────────┤ 0xffff_ffff_8000_0000
//!                       │       Kernel Text (.text)      │
//!                       │       Kernel Data (.data)      │
//!                       │       Kernel BSS  (.bss)       │
//!                       │       Kernel Stacks            │
//! 0xFFFF_FFFF_FFFF_FFFF └────────────────────────────────┘
//! ```
//!
//! ### Memory Protection Attributes
//! * **Kernel Code**: Read + Execute, Global, Supervisor-only
//! * **Kernel Data**: Read + Write + NX, Global, Supervisor-only
//! * **HHDM Region**: Read + Write + NX, Global, Supervisor-only
//! * **Identity Map**: Read + Execute, Global (temporary)
//!
//! ## ELF Loading Strategy
//!
//! The loader implements sophisticated ELF64 handling:
//!
//! ### Load Address Calculation
//! ```text
//! ELF Virtual Address (VMA) → Physical Load Address (LMA)
//!
//! For higher-half kernel:
//! LMA = VMA - KERNEL_BASE
//! Physical Address = PHYS_LOAD + LMA
//!
//! Example:
//! VMA = 0xffff_ffff_8010_0000 (kernel text)
//! LMA = 0x0010_0000
//! Physical = 0x0010_0000 + 0x0010_0000 = 0x0010_0000
//! ```
//!
//! ### Segment Processing
//! 1. **Parse `PT_LOAD` Headers**: Extract loadable segment information
//! 2. **Calculate Addresses**: Resolve virtual-to-physical mappings
//! 3. **Allocate Memory**: Reserve physical pages for segment content
//! 4. **Copy Content**: Transfer ELF data to allocated memory
//! 5. **Zero BSS**: Initialize uninitialized data sections
//!
//! ## UEFI Integration
//!
//! ### Boot Services Utilization
//! * **Memory Allocation**: Use UEFI allocator for temporary structures
//! * **File Access**: Read kernel image from ESP filesystem
//! * **Graphics Setup**: Configure GOP framebuffer for kernel graphics
//! * **Protocol Access**: Interact with UEFI configuration tables
//!
//! ### Exit Strategy
//! ```rust
//! // Critical sequence for UEFI exit
//! 1. Package all required information
//! 2. Call ExitBootServices()
//! 3. UEFI services now unavailable
//! 4. Switch to kernel page tables
//! 5. Jump to kernel entry point
//! ```
//!
//! ## Trampoline Execution
//!
//! The transition to kernel requires careful stack and page table management:
//!
//! ### Assembly Transition Code
//! ```asm
//! cli                    ; Disable interrupts
//! mov cr3, new_pml4      ; Switch page tables
//! mov rsp, new_stack     ; Set kernel stack
//! jmp kernel_entry       ; Transfer control
//! ```
//!
//! ### Stack Management
//! * **Trampoline Stack**: Temporary stack for page table switch
//! * **Guard Pages**: Protect against stack overflow
//! * **Alignment**: Maintain 16-byte stack alignment for ABI
//! * **Identity Mapping**: Ensure trampoline code remains executable
//!
//! ## Error Handling
//!
//! The loader implements comprehensive error handling:
//! * **ELF Validation**: Verify file format and architecture compatibility
//! * **Memory Allocation**: Handle allocation failures gracefully
//! * **Resource Cleanup**: Ensure proper cleanup on failure paths
//! * **Status Reporting**: Provide meaningful error codes to firmware
//!
//! ## Security Considerations
//!
//! ### Memory Protection
//! * **NX-bit Enforcement**: Prevent execution of data pages
//! * **Write Protection**: Enable supervisor write protection
//! * **Privilege Separation**: Establish kernel/user privilege levels
//!
//! ### Input Validation
//! * **ELF Parsing**: Validate all ELF structures before use
//! * **Address Bounds**: Verify all memory addresses are valid
//! * **Size Limits**: Prevent integer overflow in size calculations
//!
//! ## Development and Debugging
//!
//! The loader integrates with QEMU debugging facilities:
//! * **Debug Logging**: Comprehensive trace output for development
//! * **Memory Layout Tracing**: Detailed memory allocation information
//! * **Boot Information**: Complete system state at kernel handoff
//! * **Error Diagnostics**: Detailed error reporting for troubleshooting
//!
//! ## Standards Compliance
//!
//! * **UEFI Specification**: Full compliance with UEFI 2.x requirements
//! * **ELF64 Standard**: Correct interpretation of ELF file format
//! * **x86-64 ABI**: Proper calling convention and stack management
//! * **PE/COFF Format**: UEFI application binary format compliance

#![cfg_attr(not(any(test, doctest)), no_std)]
#![no_main]
#![allow(unsafe_code, dead_code)]
extern crate alloc;

mod elf;
mod file_system;
mod framebuffer;
mod logger;
mod memory;
mod rsdp;
mod tracing;
mod uefi_mmap;
mod vmem;

use crate::elf::parser::ElfHeader;
use crate::file_system::load_file;
use crate::framebuffer::get_framebuffer;
use crate::logger::UefiLogger;
use crate::memory::alloc_trampoline_stack;
use crate::rsdp::find_rsdp_addr;
use crate::tracing::trace_boot_info;
use crate::uefi_mmap::exit_boot_services;
use crate::vmem::create_kernel_pagetables;
use alloc::boxed::Box;
use kernel_info::boot::{KernelBootInfo, MemoryMapInfo};
use kernel_registers::cr4::Cr4;
use kernel_registers::{LoadRegister, StoreRegister, efer::Efer};
use kernel_vmem::addresses::{PhysicalAddress, VirtualAddress};
use log::{LevelFilter, debug, info};
use uefi::boot::PAGE_SIZE;
use uefi::cstr16;
use uefi::prelude::*;

// TODO: Add proper documentation.
const TRAMPOLINE_STACK_SIZE_BYTES: usize = 64 * 1024;

#[entry]
#[allow(clippy::too_many_lines)]
fn efi_main() -> Status {
    // Initialize logging and allocator helpers
    if uefi::helpers::init().is_err() {
        return Status::UNSUPPORTED;
    }

    let logger = UefiLogger::new(LevelFilter::Debug);
    let logger = logger.init().expect("logger init");

    info!("UEFI Loader reporting to QEMU");
    info!("Attempting to load kernel.elf ...");

    let elf_bytes = match load_file(cstr16!("\\EFI\\Boot\\kernel.elf")) {
        Ok(bytes) => bytes,
        Err(status) => {
            info!("Failed to load kernel.elf. Exiting.");
            return status;
        }
    };

    // Parse ELF64, collect PT_LOAD segments and entry address
    let Ok(parsed) = ElfHeader::parse_elf64(&elf_bytes) else {
        info!("kernel.elf is not a valid x86_64 ELF64");
        return Status::UNSUPPORTED;
    };

    info!("Loading kernel segments into memory ...");
    let kernel_segments = match elf::loader::load_pt_load_segments_hi(&elf_bytes, &parsed) {
        Ok(segments) => segments,
        Err(e) => {
            info!("Failed to load PT_LOAD segments: {e:?}");
            return e.into();
        }
    };

    info!(
        "kernel.elf loaded successfully: entry={}, segments={}",
        parsed.entry,
        parsed.segments.len()
    );

    let fb = match get_framebuffer() {
        Ok(fb) => fb,
        Err(status) => {
            return status;
        }
    };

    // Locate RSDP before exiting boot services; if not found, set 0.
    let rsdp_addr: u64 = find_rsdp_addr();

    let boot_info = KernelBootInfo {
        // Memory map fields are filled right after exit_boot_services returns the owned map:
        mmap: MemoryMapInfo {
            mmap_ptr: 0,
            mmap_len: 0,
            mmap_desc_size: 0,
            mmap_desc_version: 0,
        },
        rsdp_addr,
        fb,
    };

    // Heap-allocate and leak the boot info.
    let boot_info = Box::leak(Box::new(boot_info));
    info!("Kernel boot info: {:#?}", core::ptr::from_ref(boot_info));

    // The trampoline code must also be mapped, otherwise we won't be able to execute it
    // when switching the CR3 page tables.
    let tramp_code_va = VirtualAddress::new(switch_to_kernel as usize as u64);
    let tramp_code_len: usize = PAGE_SIZE; // should be enough

    // Allocate a trampoline stack (with guard page)
    debug!(
        "Allocating trampoline stack for Kernel ({tramp_code_va}, {TRAMPOLINE_STACK_SIZE_BYTES} bytes)"
    );
    let (tramp_stack_base_phys, tramp_stack_top_va) =
        alloc_trampoline_stack(TRAMPOLINE_STACK_SIZE_BYTES, true);

    // Pass identity-mapped low pointer
    let bi_ptr_va = VirtualAddress::from_ptr(core::ptr::from_ref::<KernelBootInfo>(boot_info));

    // Build page tables
    info!("Creating initial kernel page tables ...");
    let Ok(pml4_phys) = create_kernel_pagetables(
        &kernel_segments,
        tramp_code_va,
        tramp_code_len,
        tramp_stack_base_phys,
        TRAMPOLINE_STACK_SIZE_BYTES,
        bi_ptr_va,
    ) else {
        uefi::println!("Failed to create kernel page tables");
        return Status::OUT_OF_RESOURCES;
    };

    logger.exit_boot_services();
    boot_info.mmap = match exit_boot_services() {
        Ok(value) => value,
        Err(value) => return value,
    };

    // Off we pop.
    unsafe {
        trace_boot_info(boot_info, bi_ptr_va, parsed.entry, tramp_stack_top_va);
        enable_wp_nxe_pge();

        // Activate our CR3 and jump to kernel entry (higher-half VA)
        switch_to_kernel(
            pml4_phys,
            parsed.entry, // this is the higher-half VMA from ELF header
            bi_ptr_va,
            tramp_stack_top_va,
        )
    }
}

#[allow(clippy::items_after_statements)]
unsafe fn enable_wp_nxe_pge() {
    // CR0.WP = 1 (write-protect in supervisor)
    info!("Enabling supervisor write protection ...");
    let mut cr0: u64;
    unsafe {
        core::arch::asm!("mov {}, cr0", out(reg) cr0, options(nomem, preserves_flags));
    }
    cr0 |= 1 << 16;
    unsafe {
        core::arch::asm!("mov cr0, {}", in(reg) cr0, options(nomem, preserves_flags));
    }

    // EFER.NXE = 1
    info!("Setting EFER.NXE ...");
    unsafe {
        Efer::load().with_nxe(true).store();
    }

    // CR4.PGE = 1 (global pages)
    info!("Enabling global pages ...");
    unsafe {
        Cr4::load().with_pge(true).store();
    }
}

type PageTablePhysicalAddress = PhysicalAddress;
type KernelVirtualAddress = VirtualAddress;
type BootInfoVirtualAddress = VirtualAddress;
type TrampolineStackVirtualAddress = VirtualAddress;

/// Enter the kernel via a tiny trampoline.
/// - `new_cr3`: phys addr of PML4 (4KiB aligned)
/// - `kernel_entry`: higher-half VA (your `extern "win64" fn(*const BootInfo) -> !`)
/// - `boot_info`: higher-half VA (or low VA if you pass identity-mapped pointer)
/// - `tramp_stack_top`: VA of the top of the trampoline stack (identity-mapped in both maps)
#[inline(never)]
unsafe fn switch_to_kernel(
    pml4_phys: PageTablePhysicalAddress,
    kernel_entry_va: KernelVirtualAddress,
    boot_info_ptr_va: BootInfoVirtualAddress,
    tramp_stack_top_va: TrampolineStackVirtualAddress,
) -> ! {
    info!("UEFI is about to jump into Kernel land. Ciao Kakao ...");
    unsafe {
        core::arch::asm!(
            "cli",
            // Set up stack pointer
            "mov    rsp, rdx",
            // Set up page tables
            "mov    cr3, rdi",
            // Set up arguments for sysv64: rdi = boot_info, rsi = (unused), rdx = (unused)
            "mov    rdi, r8",
            // Set up kernel entry address in rax
            "mov    rax, rsi",
            // Align RSP down to 16-byte boundary
            "and    rsp, -16",
            // Emulate a CALL by pushing a dummy return address (kernel entry never returns)
            "push   0",
            "jmp    rax",
            in("rdi") pml4_phys.as_u64(),
            in("rsi") kernel_entry_va.as_u64(),
            in("rdx") tramp_stack_top_va.as_u64(),
            in("r8")  boot_info_ptr_va.as_u64(),
            options(noreturn)
        )
    }
}
