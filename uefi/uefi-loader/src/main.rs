//! # UEFI Loader Main Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code, dead_code)]

mod elf;
mod file_system;
mod memory;

use crate::elf::ElfHeader;
use crate::file_system::load_file;
use uefi::boot::ScopedProtocol;
use uefi::cstr16;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;

#[repr(C)]
pub struct BootInfo {
    pub framebuffer_ptr: u64,
    pub framebuffer_width: usize,
    pub framebuffer_height: usize,
    pub framebuffer_stride: usize,
    pub reserved: u32,
}

#[entry]
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
    let mut fb = gop.frame_buffer();
    let boot_info = BootInfo {
        framebuffer_ptr: fb.as_mut_ptr() as u64,
        framebuffer_width: mode.resolution().0,
        framebuffer_height: mode.resolution().1,
        framebuffer_stride: mode.stride(),
        reserved: 0,
    };

    // Note: We have not yet loaded segments nor jumped to the kernel.
    // Next step will allocate memory, copy PT_LOAD segments, zero BSS, exit boot services and jump.

    // Exit boot services (must be last UEFI call)
    // After this returns, do not call any UEFI APIs (incl. println!).
    uefi::println!("Exiting boot services.");

    unsafe {
        // You can pass Some(MemoryType) if you want to tag the map allocation differently.
        let _owned_map = boot::exit_boot_services(None);
    }

    // For now, we just return success; jumping will be implemented in the next step.
    let _ = boot_info; // suppress unused for now
    Status::SUCCESS
}

fn get_gop() -> Result<ScopedProtocol<GraphicsOutput>, uefi::Error> {
    let handle = boot::get_handle_for_protocol::<GraphicsOutput>()
        .map_err(|_| uefi::Error::new(Status::ABORTED, ()))?;
    let gop = boot::open_protocol_exclusive::<GraphicsOutput>(handle)
        .map_err(|_| uefi::Error::new(Status::ABORTED, ()))?;
    Ok(gop)
}
