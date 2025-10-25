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
use alloc::vec;
use kernel_info::{KernelBootInfo, KernelEntry};
use uefi::boot::{MemoryType, ScopedProtocol};
use uefi::cstr16;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

#[entry]
#[allow(clippy::too_many_lines)]
fn efi_main() -> Status {
    // Initialize logging and allocator helpers
    if uefi::helpers::init().is_err() {
        return Status::UNSUPPORTED;
    }

    uefi::println!("UEFI Loader: starting up");

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

    uefi::println!(
        "UEFI Loader: kernel.elf loaded: entry=0x{:x}, segments={}",
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
        PixelFormat::Bitmask if mode.pixel_bitmask().is_none() => {
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

    let mut boot_info = KernelBootInfo {
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

    // Note: We have not yet loaded PT_LOAD segments; jumping may crash until we implement it.
    // Current step exits boot services and jumps to the kernel entry with GOP BootInfo.
    uefi::println!("Booting kernel ...");

    // Ensure all UEFI protocol guards are dropped before exiting boot services.
    drop(gop); // TODO: Not sure if this is correct!

    // Introspect the memory map.
    let memory_map_size = match boot::memory_map(MemoryType::LOADER_DATA) {
        Ok(map) => {
            let size = map.meta().map_size;
            assert!(size < 64 * 1024, "memory map too large");
            uefi::println!("Memory map: {:#?} (will allocate {size} bytes)", map);
            size
        }
        Err(e) => {
            uefi::println!("Failed to get memory map: {e:?}");
            return Status::UNSUPPORTED;
        }
    };

    // Pre-allocate a buffer while UEFI allocator is still alive.
    let mut mmap_copy = vec![0u8; memory_map_size];
    let mmap_copy_ptr = mmap_copy.as_mut_ptr();
    let mmap_copy_cap = mmap_copy.len();

    // Exit boot services — after this, the UEFI allocator must not be used anymore.
    let owned_map = unsafe { boot::exit_boot_services(None) };

    // Copy the returned descriptors into our preallocated buffer.
    let src = owned_map.buffer().as_ptr();
    let mmap_length = owned_map.buffer().len();

    // Safety: ensure the buffer is large enough (or bail/panic in dev builds).
    assert!(
        mmap_length <= mmap_copy_cap,
        "preallocated mmap buffer too small"
    );
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
    run_kernel(&parsed, &boot_info);
}

fn run_kernel(parsed: &ElfHeader, boot_info: &KernelBootInfo) -> ! {
    let entry: KernelEntry = unsafe { core::mem::transmute(parsed.entry) };
    let bi_ptr: *const KernelBootInfo = boot_info as *const KernelBootInfo;
    entry(bi_ptr)
}

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
