//! # UEFI Loader Main Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code, dead_code)]

mod file_system;
mod memory;

use crate::file_system::load_file;
use uefi::cstr16;
use uefi::prelude::*;

#[repr(C)]
pub struct BootInfo {
    pub framebuffer_ptr: u64,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_stride: u32,
    pub reserved: u32,
}

// Minimal ELF64 definitions
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const PT_LOAD: u32 = 1;
const EM_X86_64: u16 = 62;

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

    // TODO: Parse ELF64, allocate/load PT_LOAD segments, get `entry_addr` (usize).
    // let entry_addr: usize = ...;

    uefi::println!("UEFI Loader: kernel.elf loaded");
    boot::stall(1_000_000);

    // Exit boot services (must be last UEFI call)
    // After this returns, do not call any UEFI APIs (incl. println!).
    uefi::println!("Exiting boot services.");
    unsafe {
        // You can pass Some(MemoryType) if you want to tag the map allocation differently.
        let _owned_map = boot::exit_boot_services(None);
    }

    Status::SUCCESS
}
