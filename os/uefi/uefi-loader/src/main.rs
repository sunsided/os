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
use crate::elf::vmem::create_kernel_pagetables;
use crate::file_system::load_file;
use crate::framebuffer::get_framebuffer;
use crate::rsdp::find_rsdp_addr;
use crate::tracing::trace_boot_info;
use crate::uefi_mmap::exit_boot_services;
use alloc::boxed::Box;
use kernel_info::boot::{KernelBootInfo, KernelEntryFn, MemoryMapInfo};
use kernel_qemu::qemu_trace;
use uefi::cstr16;
use uefi::prelude::*;

#[entry]
#[allow(clippy::too_many_lines)]
fn efi_main() -> Status {
    // Initialize logging and allocator helpers
    if uefi::helpers::init().is_err() {
        return Status::UNSUPPORTED;
    }

    qemu_trace!("UEFI Loader reporting to QEMU\n");
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

    // Heap-allocate and leak the boot info.
    let boot_info = Box::leak(Box::new(boot_info));
    uefi::println!("Kernel boot info: {:#?}", core::ptr::from_ref(boot_info));

    // Build page tables
    let Ok(pml4_phys) = create_kernel_pagetables(&parsed) else {
        uefi::println!("Failed to create kernel page tables");
        return Status::OUT_OF_RESOURCES;
    };

    // Choose which BootInfo pointer to pass:
    //    (a) identity-mapped low pointer (we kept a 2 MiB identity mapping)
    let bi_ptr_va = core::ptr::from_ref::<KernelBootInfo>(boot_info) as u64;

    boot_info.mmap = match exit_boot_services() {
        Ok(value) => value,
        Err(value) => return value,
    };

    // Off we pop.
    unsafe {
        trace_boot_info(boot_info, bi_ptr_va, parsed.entry);
        enable_wp_nxe_pge();

        // Activate our CR3 and jump to kernel entry (higher-half VA)
        switch_to_kernel(
            pml4_phys.0,
            parsed.entry, // this is the higher-half VMA from ELF header
            bi_ptr_va,
        )
    }
}

#[allow(clippy::items_after_statements)]
unsafe fn enable_wp_nxe_pge() {
    // CR0.WP = 1 (write-protect in supervisor)
    qemu_trace!("Enabling supervisor write protection ...\n");
    let mut cr0: u64;
    unsafe {
        core::arch::asm!("mov {}, cr0", out(reg) cr0, options(nomem, preserves_flags));
    }
    cr0 |= 1 << 16;
    unsafe {
        core::arch::asm!("mov cr0, {}", in(reg) cr0, options(nomem, preserves_flags));
    }

    // EFER.NXE = 1
    qemu_trace!("Setting EFER.NXE ...\n");
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
    qemu_trace!("Enabling global pages ...\n");
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
    qemu_trace!("Loading CR3 with the Page Table Root ...\n");
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) new_cr3, options(nostack, preserves_flags));
    }

    // Tail-call into kernel entry (win64 abi)
    qemu_trace!("UEFI is now jumping into Kernel land. Ciao Kakao ...\n");
    let entry: KernelEntryFn = unsafe { core::mem::transmute(kernel_entry_va) };
    let bi_ptr = boot_info_ptr_va as *const KernelBootInfo;
    entry(bi_ptr)
}
