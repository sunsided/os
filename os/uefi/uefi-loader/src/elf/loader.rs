//! # ELF Segment Loader

extern crate alloc;

use crate::elf::parser::ElfHeader;
use crate::elf::{PAGE_SIZE, PF_X};
use core::ptr::{self};
use uefi::Status;
use uefi::boot::{self, AllocateType, MemoryType};

#[allow(clippy::inline_always)]
#[inline(always)]
fn align_down(x: u64, a: u64) -> u64 {
    debug_assert!(a.is_power_of_two());
    x & !(a - 1)
}

#[allow(clippy::inline_always)]
#[inline(always)]
fn align_up(x: u64, a: u64) -> u64 {
    debug_assert!(a.is_power_of_two());
    (x + a - 1) & !(a - 1)
}

/// Load all `PT_LOAD` segments into memory at their virtual addresses.
/// Returns `UNSUPPORTED` if exact address allocation fails or bounds are invalid.
pub fn load_pt_load_segments(elf_bytes: &[u8], hdr: &ElfHeader) -> Result<(), Status> {
    for seg in &hdr.segments {
        if seg.memsz == 0 {
            continue;
        }

        // Calculate page-aligned allocation range that fully covers the segment
        let seg_start = seg.vaddr;
        let seg_end = seg
            .vaddr
            .checked_add(seg.memsz)
            .ok_or(Status::UNSUPPORTED)?;
        let alloc_start = align_down(seg_start, PAGE_SIZE);
        let alloc_end = align_up(seg_end, PAGE_SIZE);
        let pages = (alloc_end - alloc_start) / PAGE_SIZE;

        // Executable segments in LOADER_CODE, others in LOADER_DATA
        let mem_type = if (seg.flags & PF_X) != 0 {
            MemoryType::LOADER_CODE
        } else {
            MemoryType::LOADER_DATA
        };

        // Reserve the exact address range
        // UEFI allocate_pages returns a pointer to the allocated region.
        let ptr =
            boot::allocate_pages(AllocateType::Address(alloc_start), mem_type, pages as usize)
                .map_err(|_| Status::UNSUPPORTED)?;
        // Compute the in-segment base pointer based on what firmware returned.
        // We requested [alloc_start, alloc_end), so seg_start may be inside that range.
        let base = ptr.as_ptr() as u64;
        let in_seg_off = seg_start - alloc_start; // safe: seg_start >= alloc_start by construction
        let dst_seg_ptr = (base + in_seg_off) as *mut u8;

        // Zero initialize the in-memory size for this segment (.bss etc.)
        let mem_len = usize::try_from(seg.memsz).map_err(|_| Status::UNSUPPORTED)?;
        unsafe {
            ptr::write_bytes(dst_seg_ptr, 0, mem_len);
        }

        // Copy file payload if any
        if seg.filesz != 0 {
            let src_off = usize::try_from(seg.offset).map_err(|_| Status::UNSUPPORTED)?;
            let file_len = usize::try_from(seg.filesz).map_err(|_| Status::UNSUPPORTED)?;
            let src_end = src_off.checked_add(file_len).ok_or(Status::UNSUPPORTED)?;
            if src_end > elf_bytes.len() {
                return Err(Status::UNSUPPORTED);
            }
            unsafe {
                ptr::copy_nonoverlapping(elf_bytes.as_ptr().add(src_off), dst_seg_ptr, file_len);
            }
        }
    }

    Ok(())
}
