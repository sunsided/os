//! # UEFI Loader Main Entry Point

#![cfg_attr(not(test), no_std)]
#![no_main]
#![allow(unsafe_code, dead_code)]
extern crate alloc;

mod elf;
mod file_system;
mod framebuffer;
mod memory;
mod rsdp;
mod tracing;
mod uefi_mmap;

use crate::elf::parser::ElfHeader;
use crate::file_system::load_file;
use crate::framebuffer::get_framebuffer;
use crate::rsdp::find_rsdp_addr;
use crate::tracing::{trace, trace_boot_info};
use crate::uefi_mmap::exit_boot_services;
use alloc::boxed::Box;
use kernel_info::boot::{KernelBootInfo, KernelEntryFn, MemoryMapInfo};
use uefi::cstr16;
use uefi::prelude::*;

#[entry]
#[allow(clippy::too_many_lines)]
fn efi_main() -> Status {
    // Initialize logging and allocator helpers
    if uefi::helpers::init().is_err() {
        return Status::UNSUPPORTED;
    }

    trace("UEFI Loader reporting to QEMU\n");
    uefi::println!("Attempting to load kernel.elf ...");

    let elf_bytes = match load_file(cstr16!("\\EFI\\Boot\\kernel.elf")) {
        Ok(bytes) => bytes,
        Err(status) => {
            uefi::println!("Failed to load kernel.elf. Exiting.");
            return status;
        }
    };

    // Parse ELF64, collect PT_LOAD segments and entry address
    let Ok(parsed) = ElfHeader::parse_elf64(&elf_bytes) else {
        uefi::println!("kernel.elf is not a valid x86_64 ELF64");
        return Status::UNSUPPORTED;
    };

    uefi::println!("Loading kernel segments into memory ...");
    if let Err(e) = elf::loader::load_pt_load_segments_hi(&elf_bytes, &parsed) {
        uefi::println!("Failed to load PT_LOAD segments: {e:?}");
        return e.into();
    }

    uefi::println!(
        "kernel.elf loaded successfully: entry=0x{:x}, segments={}",
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

    let boot_info = Box::new(boot_info);

    // leak it so it stays alive after exit; allocator not usable post-exit anyway
    let boot_info = Box::leak(boot_info);

    // Note: We have not yet loaded PT_LOAD segments; jumping may crash until we implement it.
    // Current step exits boot services and jumps to the kernel entry with GOP BootInfo.
    uefi::println!("Booting kernel ...");
    trace("Booting kernel ...\n");

    boot_info.mmap = match exit_boot_services() {
        Ok(value) => value,
        Err(value) => return value,
    };
    // Off we pop.
    run_kernel(&parsed, boot_info);
}

/// Jump into the kernel code.
fn run_kernel(parsed: &ElfHeader, boot_info: &KernelBootInfo) -> ! {
    trace_boot_info(boot_info);
    trace("UEFI is now jumping into Kernel land. Ciao Kakao ...\n");
    let entry: KernelEntryFn = unsafe { core::mem::transmute(parsed.entry) };
    let bi_ptr: *const KernelBootInfo = boot_info as *const KernelBootInfo;
    entry(bi_ptr)
}

#[allow(clippy::items_after_statements)]
unsafe fn enable_wp_nxe_pge() {
    // CR0.WP = 1 (write-protect in supervisor)
    let mut cr0: u64;
    unsafe {
        core::arch::asm!("mov {}, cr0", out(reg) cr0, options(nomem, preserves_flags));
    }
    cr0 |= 1 << 16;
    unsafe {
        core::arch::asm!("mov cr0, {}", in(reg) cr0, options(nomem, preserves_flags));
    }

    // EFER.NXE = 1
    const MSR_EFER: u32 = 0xC000_0080; // TODO: Document this properly
    let (mut lo, mut hi): (u32, u32);
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") MSR_EFER, out("eax") lo, out("edx") hi, options(nomem, preserves_flags));
    }
    let mut efer = u64::from(hi) << 32 | u64::from(lo);
    efer |= 1 << 11;
    lo = u32::try_from(efer).expect("failed to cast efer to u32"); // TODO: Handle properly
    hi = (efer >> 32) as u32;
    unsafe {
        core::arch::asm!("wrmsr", in("ecx") MSR_EFER, in("eax") lo, in("edx") hi, options(nomem, preserves_flags));
    }

    // CR4.PGE = 1 (global pages)
    let mut cr4: u64;
    unsafe {
        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, preserves_flags));
    }
    cr4 |= 1 << 7;
    unsafe {
        core::arch::asm!("mov cr4, {}", in(reg) cr4, options(nomem, preserves_flags));
    }
}

type PageTablePhysicalAddress = u64;
type KernelVirtualAddress = u64;
type BootInfoVirtualAddress = u64;

/// Switch to new CR3 and immediately jump to the kernel entry at its higher-half VA.
unsafe fn switch_to_kernel(
    new_cr3: PageTablePhysicalAddress,
    kernel_entry_va: KernelVirtualAddress,
    boot_info_ptr_va: BootInfoVirtualAddress,
) -> ! {
    // Load CR3
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) new_cr3, options(nostack, preserves_flags));
    }

    // Tail-call into kernel entry (win64 abi)
    let entry: KernelEntryFn = unsafe { core::mem::transmute(kernel_entry_va) };
    let bi_ptr = boot_info_ptr_va as *const KernelBootInfo;
    entry(bi_ptr)
}
