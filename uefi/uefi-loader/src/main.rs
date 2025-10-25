//! # UEFI Loader Main Entry Point

#![cfg_attr(not(test), no_std)]
#![no_main]
#![allow(unsafe_code, dead_code)]
extern crate alloc;

mod elf;
mod file_system;
mod framebuffer;
mod memory;
mod memory_mapper;
mod rsdp;

use crate::elf::ElfHeader;
use crate::file_system::load_file;
use crate::framebuffer::get_framebuffer;
use crate::memory_mapper::UefiIdentityMapper;
use crate::rsdp::find_rsdp_addr;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use kernel_acpi::rsdp::AcpiRoots;
use kernel_info::{KernelBootInfo, KernelEntry, MemoryMapInfo};
use uefi::boot::MemoryType;
use uefi::cstr16;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;

fn trace<S>(message: S)
where
    S: AsRef<[u8]>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print(message);
    }
}

fn trace_usize<N>(number: N)
where
    N: Into<usize>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print_usize(number);
    }
}

fn trace_u64<N>(number: N)
where
    N: Into<u64>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print_u64(number);
    }
}

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
    if let Err(e) = elf::load_pt_load_segments(&elf_bytes, &parsed) {
        uefi::println!("Failed to load PT_LOAD segments: {e:?}");
        return Status::UNSUPPORTED;
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

    let mapper = UefiIdentityMapper;

    // Locate RSDP before exiting boot services; if not found, set 0.
    let rsdp_addr: u64 = find_rsdp_addr();

    #[cfg(feature = "qemu")]
    {
        if let Some(roots) = unsafe { AcpiRoots::parse(&mapper, rsdp_addr) } {
            if let Some(addr) = roots.rsdt_addr {
                trace("Found RSDT for ACPI 1.0 at ");
                trace_u64(addr);
                trace("\n");
            } else if let Some(addr) = roots.xsdt_addr {
                trace("Found XSDT for ACPI 2.0 at ");
                trace_u64(addr);
                trace("\n");
            } else {
                trace("Found unknown ACPI variant\n");
            }
        } else {
            trace("No ACPI RSDP found in UEFI configuration table\n");
        }
    }

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

    // Pre-allocate a buffer while UEFI allocator is still alive.
    let mut mmap_copy = match allocate_mmap_buffer() {
        Ok(buf) => buf,
        Err(status) => {
            return status;
        }
    };
    let mmap_copy_ptr = mmap_copy.as_mut_ptr();

    // Exit boot services — after this, the UEFI allocator must not be used anymore.
    let owned_map = unsafe { boot::exit_boot_services(None) };

    // Copy the returned descriptors into our preallocated buffer.
    let src = owned_map.buffer().as_ptr();
    let mmap_length = owned_map.buffer().len();

    // Safety: ensure the buffer is large enough (or bail/panic in dev builds).
    if mmap_length > mmap_copy.len() {
        trace("Memory map size assertion failed: Expected ");
        trace_usize(mmap_copy.len());
        trace(", got ");
        trace_usize(mmap_length);
        return Status::BUFFER_TOO_SMALL;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(src, mmap_copy_ptr, mmap_length);
    }

    // Fill BootInfo with the copy.
    boot_info.mmap.mmap_ptr = mmap_copy_ptr as u64;
    boot_info.mmap.mmap_len = mmap_length as u64;
    boot_info.mmap.mmap_desc_size = owned_map.meta().desc_size as u64;
    boot_info.mmap.mmap_desc_version = owned_map.meta().desc_version;

    // Ensure the memory map copy continues to exist.
    core::mem::forget(mmap_copy);

    // Off we pop.
    run_kernel(&parsed, boot_info);
}

/// Jump into the kernel code.
fn run_kernel(parsed: &ElfHeader, boot_info: &KernelBootInfo) -> ! {
    trace_boot_info(boot_info);
    trace("UEFI is now jumping into Kernel land. Ciao Kakao ...\n");
    let entry: KernelEntry = unsafe { core::mem::transmute(parsed.entry) };
    let bi_ptr: *const KernelBootInfo = boot_info as *const KernelBootInfo;
    entry(bi_ptr)
}

/// Allocate a buffer to hold a copy of the memory map returned from `ExitBootServices`.
///
/// This seems to be the opposite of an exact science:
/// * After boot services were exited, allocation is impossible.
/// * The number of descriptors changes over time.
///
/// As a result, we now overallocate to hopefully have enough headroom
/// to contain the memory map _after_ exiting.
fn allocate_mmap_buffer() -> Result<Vec<u8>, Status> {
    const EXTRA_DESCS: usize = 32;

    // Introspect the memory map.
    let probe = match boot::memory_map(MemoryType::LOADER_DATA) {
        Ok(probe) => probe,
        Err(e) => {
            uefi::println!("Failed to get memory map: {e:?}");
            return Err(Status::UNSUPPORTED);
        }
    };

    let desc_size = probe.meta().desc_size;
    let mut needed_size = probe.meta().map_size;

    // We won't use `probe`'s buffer; drop it now to reduce churn.
    drop(probe);

    // Pre-allocate our own buffer with slack for extra descriptors.
    // Rule of thumb: + N * desc_size; N=16..64 is usually plenty in QEMU/OVMF.
    needed_size += EXTRA_DESCS * desc_size;

    // Pre-allocate a buffer while UEFI allocator is still alive.
    let buf = vec![0u8; needed_size];
    Ok(buf)
}

fn trace_boot_info(boot_info: &KernelBootInfo) {
    trace("Boot Info in UEFI Loader:\n");
    trace("   BI ptr = ");
    trace_usize(core::ptr::from_ref(boot_info) as usize);
    trace("\n");
    trace(" MMAP ptr = ");
    trace_u64(boot_info.mmap.mmap_ptr);
    trace(", MMAP len = ");
    trace_u64(boot_info.mmap.mmap_len);
    trace(", MMAP desc size = ");
    trace_u64(boot_info.mmap.mmap_desc_size);
    trace(", MMAP desc version = ");
    trace_usize(usize::try_from(boot_info.mmap.mmap_desc_version).unwrap_or_default());
    trace(", rsdp addr = ");
    trace_usize(usize::try_from(boot_info.rsdp_addr).unwrap_or_default());
    trace("\n");
    trace("   FB ptr = ");
    trace_u64(boot_info.fb.framebuffer_ptr);
    trace(", FB size = ");
    trace_u64(boot_info.fb.framebuffer_size);
    trace(", FB width = ");
    trace_u64(boot_info.fb.framebuffer_width);
    trace(", FB height = ");
    trace_u64(boot_info.fb.framebuffer_height);
    trace(", FB stride = ");
    trace_u64(boot_info.fb.framebuffer_stride);
    trace(", FB format = ");
    match boot_info.fb.framebuffer_format {
        kernel_info::BootPixelFormat::Rgb => trace("RGB"),
        kernel_info::BootPixelFormat::Bgr => trace("BGR"),
        kernel_info::BootPixelFormat::Bitmask => trace("Bitmask"),
        kernel_info::BootPixelFormat::BltOnly => trace("BltOnly"),
    }
    trace("\n");
}
