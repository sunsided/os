//! # UEFI Loader Main Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code, dead_code)]
extern crate alloc;

mod elf;
mod file_system;
mod memory;

use crate::elf::ElfHeader;
use crate::file_system::load_file;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use kernel_info::{KernelBootInfo, KernelEntry};
use uefi::boot::{MemoryType, ScopedProtocol};
use uefi::cstr16;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

fn trace<S>(message: S)
where
    S: AsRef<[u8]>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print(message);
    }
}

fn trace_num<N>(number: N)
where
    N: Into<usize>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print_usize(number);
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
    boot::stall(500_000);

    uefi::println!("Obtaining Graphics Output Protocol (GOP)");
    let mut gop = match get_gop() {
        Ok(gop) => gop,
        Err(e) => {
            uefi::println!("Failed to get GOP: {e:?}");
            return Status::UNSUPPORTED;
        }
    };

    let mode = gop.current_mode_info();
    let (framebuffer_width, framebuffer_height) = mode.resolution();

    let (framebuffer_format, framebuffer_masks) = match mode.pixel_format() {
        PixelFormat::Rgb => (
            kernel_info::BootPixelFormat::Rgb,
            kernel_info::BootPixelMasks {
                red_mask: 0,
                green_mask: 0,
                blue_mask: 0,
                alpha_mask: 0,
            },
        ),
        PixelFormat::Bgr => (
            kernel_info::BootPixelFormat::Bgr,
            kernel_info::BootPixelMasks {
                red_mask: 0,
                green_mask: 0,
                blue_mask: 0,
                alpha_mask: 0,
            },
        ),
        PixelFormat::Bitmask if mode.pixel_bitmask().is_some() => {
            let mask = mode.pixel_bitmask().unwrap();
            (
                kernel_info::BootPixelFormat::Bitmask,
                kernel_info::BootPixelMasks {
                    red_mask: mask.red,
                    green_mask: mask.green,
                    blue_mask: mask.blue,
                    alpha_mask: mask.reserved,
                },
            )
        }
        PixelFormat::BltOnly | PixelFormat::Bitmask => {
            uefi::println!("Unsupported pixel format: Bitmask");
            return Status::UNSUPPORTED;
        }
    };

    let mut fb = gop.frame_buffer();
    let framebuffer_ptr = fb.as_mut_ptr();
    let framebuffer_size = fb.size();
    let framebuffer_stride = mode.stride();

    // (Optional) locate RSDP before exiting boot services; if not found, set 0.
    let rsdp_addr: u64 = /* find via config tables, else 0 */ 0;

    let boot_info = KernelBootInfo {
        framebuffer_ptr: framebuffer_ptr as usize,
        framebuffer_size,
        framebuffer_width,
        framebuffer_height,
        framebuffer_stride,
        framebuffer_format,
        framebuffer_masks,
        // Memory map fields — fill right after exit_boot_services returns the owned map:
        mmap_ptr: 0,
        mmap_len: 0,
        mmap_desc_size: 0,
        mmap_desc_version: 0,
        rsdp_addr,
    };

    let boot_info = Box::new(boot_info);

    // leak it so it stays alive after exit; allocator not usable post-exit anyway
    let boot_info = Box::leak(boot_info);

    // Note: We have not yet loaded PT_LOAD segments; jumping may crash until we implement it.
    // Current step exits boot services and jumps to the kernel entry with GOP BootInfo.
    uefi::println!("Booting kernel ...");
    trace("Booting kernel ...\n");

    // Ensure all UEFI protocol guards are dropped before exiting boot services.
    drop(gop); // TODO: Not sure if this is correct!

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
        trace_num(mmap_copy.len());
        trace(", got ");
        trace_num(mmap_length);
        return Status::BUFFER_TOO_SMALL;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(src, mmap_copy_ptr, mmap_length);
    }

    // Fill BootInfo with the copy.
    boot_info.mmap_ptr = mmap_copy_ptr as usize;
    boot_info.mmap_len = mmap_length;
    boot_info.mmap_desc_size = owned_map.meta().desc_size;
    boot_info.mmap_desc_version = owned_map.meta().desc_version;

    // Ensure the memory map copy continues to exist.
    core::mem::forget(mmap_copy);

    // Off we pop.
    run_kernel(&parsed, boot_info);
}

/// Jump into the kernel code.
fn run_kernel(parsed: &ElfHeader, boot_info: &KernelBootInfo) -> ! {
    trace("UEFI is now jumping into Kernel land. Bye, bye ...\n");
    let entry: KernelEntry = unsafe { core::mem::transmute(parsed.entry) };
    let bi_ptr: *const KernelBootInfo = boot_info as *const KernelBootInfo;
    entry(bi_ptr)
}

/// Fetch the Graphics Output Protocol (GOP).
fn get_gop() -> Result<ScopedProtocol<GraphicsOutput>, uefi::Error> {
    let handle = boot::get_handle_for_protocol::<GraphicsOutput>().map_err(|e| {
        uefi::println!("Failed to get GOP handle: {e:?}");
        uefi::Error::new(Status::ABORTED, ())
    })?;

    let gop = boot::open_protocol_exclusive::<GraphicsOutput>(handle).map_err(|e| {
        uefi::println!("Failed to open GOP exlusively: {e:?}");
        uefi::Error::new(Status::ABORTED, ())
    })?;
    Ok(gop)
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
