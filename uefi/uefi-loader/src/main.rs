//! # UEFI Loader Main Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code, dead_code)]

mod memory;

extern crate alloc;

use alloc::vec;
use uefi::cstr16;
use uefi::prelude::*;
use uefi::proto::media::file::{File, FileAttribute, FileMode, RegularFile};

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

    let image_handle = boot::image_handle();
    let mut sfs = match boot::get_image_file_system(image_handle) {
        Ok(fs) => fs,
        Err(e) => {
            uefi::println!("Failed to get file system: {e:?}");
            return Status::UNSUPPORTED;
        }
    };

    let mut dir = match sfs.open_volume() {
        Ok(dir) => dir,
        Err(e) => {
            uefi::println!("Failed to open root directory: {e:?}");
            return Status::UNSUPPORTED;
        }
    };

    let path = cstr16!("\\EFI\\Boot\\kernel.elf");
    let handle = match dir.open(path, FileMode::Read, FileAttribute::empty()) {
        Ok(handle) => handle,
        Err(e) => {
            uefi::println!("Failed to read kernel.elf: {e:?}");
            return Status::UNSUPPORTED;
        }
    };

    let Some(mut file) = handle.into_regular_file() else {
        uefi::println!("Failed to read kernel.elf: not a file");
        return Status::UNSUPPORTED;
    };

    // Get file size
    if let Err(e) = file.set_position(RegularFile::END_OF_FILE) {
        uefi::println!("Failed to seek to file end: {e:?}");
        return Status::UNSUPPORTED;
    }

    let size = match file.get_position() {
        Ok(size) => size,
        Err(e) => {
            uefi::println!("Failed to get file size: {e:?}");
            return Status::UNSUPPORTED;
        }
    };

    let Ok(size) = usize::try_from(size) else {
        uefi::println!("Failed to get file size: invalid pointer widths");
        return Status::UNSUPPORTED;
    };

    // Provide a buffer for the file contents
    let mut buf = vec![0u8; size];
    let read = match file.read(&mut buf) {
        Ok(size) => size,
        Err(e) => {
            uefi::println!("Failed to read file contents: {e:?}");
            return Status::UNSUPPORTED;
        }
    };

    if read != size {
        uefi::println!("Mismatch in file size: read {read} bytes, expected {size} bytes");
        return Status::UNSUPPORTED;
    }

    // TODO: Parse ELF64, allocate/load PT_LOAD segments, get `entry_addr` (usize).
    // let entry_addr: usize = ...;

    // Exit boot services (must be last UEFI call)
    // After this returns, do not call any UEFI APIs (incl. println!).
    unsafe {
        // You can pass Some(MemoryType) if you want to tag the map allocation differently.
        let _owned_map = boot::exit_boot_services(None);
    }

    Status::SUCCESS
}
