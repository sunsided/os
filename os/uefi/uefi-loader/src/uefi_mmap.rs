//! # UEFI Memory Map Utilities
//!
//! Helper functions for dealing with the UEFI memory map after exiting boot services.

use alloc::vec;
use alloc::vec::Vec;
use kernel_info::boot::MemoryMapInfo;
use log::info;
use uefi::boot::MemoryType;
use uefi::mem::memory_map::MemoryMap;
use uefi::{Status, boot};

/// Exist the UEFI boot services and retain a copy of the UEFI memory map.
pub fn exit_boot_services() -> Result<MemoryMapInfo, Status> {
    uefi::println!("Exiting boot services ...");
    info!("Exiting boot services ...");

    // Pre-allocate a buffer while UEFI allocator is still alive.
    let mut mmap_copy = match allocate_mmap_buffer() {
        Ok(buf) => buf,
        Err(status) => {
            return Err(status);
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
        info!(
            "Memory map size assertion failed: Expected {}, got {}",
            mmap_copy.len(),
            mmap_length
        );
        return Err(Status::BUFFER_TOO_SMALL);
    }
    unsafe {
        core::ptr::copy_nonoverlapping(src, mmap_copy_ptr, mmap_length);
    }

    let mmap = MemoryMapInfo {
        mmap_ptr: mmap_copy_ptr as u64,
        mmap_len: mmap_length as u64,
        mmap_desc_size: owned_map.meta().desc_size as u64,
        mmap_desc_version: owned_map.meta().desc_version,
    };

    // Ensure the memory map copy continues to exist.
    core::mem::forget(mmap_copy);

    info!("Boot services exited, we're now flying by instruments.");
    Ok(mmap)
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
