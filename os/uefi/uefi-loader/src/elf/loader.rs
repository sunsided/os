//! # ELF Segment Loader

extern crate alloc;

use crate::elf::parser::ElfHeader;
use crate::elf::{PAGE_SIZE, PF_X};
use core::ptr;
use kernel_info::memory::{KERNEL_BASE, PHYS_LOAD};
use kernel_vmem::{align_down, align_up};
use uefi::Status;
use uefi::boot::{self, AllocateType, MemoryType};

#[derive(Debug, thiserror::Error)]
pub enum ElfLoaderError {
    #[error("A pointer arithmetic operation failed due to an underflow or overflow")]
    PointerArithmetic,
    #[error("A provided memory address is out of bounds for the architecture")]
    AddressOutOfBounds,
    #[error("An physical memory allocation failed")]
    PhysicalAllocationFailed(#[source] uefi::Error),
    #[error("The ELF segment size does not match the program size")]
    ElfSizeMismatch,
}

impl From<ElfLoaderError> for Status {
    fn from(value: ElfLoaderError) -> Self {
        match value {
            ElfLoaderError::PhysicalAllocationFailed(_) => Self::BUFFER_TOO_SMALL,
            ElfLoaderError::PointerArithmetic
            | ElfLoaderError::AddressOutOfBounds
            | ElfLoaderError::ElfSizeMismatch => Self::BAD_BUFFER_SIZE,
        }
    }
}

/// Load all `PT_LOAD` segments at their **physical LMA** derived from the linker `AT()`.
/// `Vaddr = high-hal`f; `LMA = vaddr - KERNEL_BASE`; final `phys = PHYS_LOAD + LMA`.
pub fn load_pt_load_segments_hi(elf_bytes: &[u8], hdr: &ElfHeader) -> Result<(), ElfLoaderError> {
    for seg in &hdr.segments {
        if seg.memsz == 0 {
            continue;
        }

        // LMA inside the kernel image
        let lma = seg
            .vaddr
            .checked_sub(KERNEL_BASE)
            .ok_or(ElfLoaderError::PointerArithmetic)?;
        let phys_start = PHYS_LOAD
            .checked_add(lma)
            .ok_or(ElfLoaderError::PointerArithmetic)?;
        let phys_end = phys_start
            .checked_add(seg.memsz)
            .ok_or(ElfLoaderError::PointerArithmetic)?;

        // Page-aligned reservation in physical memory
        let alloc_start = align_down(phys_start, PAGE_SIZE);
        let alloc_end = align_up(phys_end, PAGE_SIZE);
        let pages = ((alloc_end - alloc_start) / PAGE_SIZE) as usize;

        let mem_type = if (seg.flags & PF_X) != 0 {
            MemoryType::LOADER_CODE
        } else {
            MemoryType::LOADER_DATA
        };

        // Reserve the physical range for the segment
        let ptr = boot::allocate_pages(AllocateType::Address(alloc_start), mem_type, pages)
            .map_err(ElfLoaderError::PhysicalAllocationFailed)?;
        let base = ptr.as_ptr() as u64;

        // Zero full in-memory size
        let mem_len = usize::try_from(seg.memsz).map_err(|_| ElfLoaderError::AddressOutOfBounds)?;
        let in_seg_off = phys_start - alloc_start;
        let dst = (base + in_seg_off) as *mut u8;
        unsafe {
            ptr::write_bytes(dst, 0, mem_len);
        }

        // Copy file payload (if any)
        if seg.filesz != 0 {
            let src_off =
                usize::try_from(seg.offset).map_err(|_| ElfLoaderError::AddressOutOfBounds)?;
            let file_len =
                usize::try_from(seg.filesz).map_err(|_| ElfLoaderError::AddressOutOfBounds)?;
            let src_end = src_off
                .checked_add(file_len)
                .ok_or(ElfLoaderError::PointerArithmetic)?;
            if src_end > elf_bytes.len() {
                return Err(ElfLoaderError::ElfSizeMismatch);
            }
            unsafe {
                ptr::copy_nonoverlapping(elf_bytes.as_ptr().add(src_off), dst, file_len);
            }
        }
    }
    Ok(())
}
