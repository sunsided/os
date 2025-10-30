//! # ELF Segment Loader

extern crate alloc;

use crate::elf::parser::{ElfHeader, PFlags};
use alloc::vec::Vec;
use core::ptr;
use kernel_info::memory::{KERNEL_BASE, PHYS_LOAD};
use kernel_vmem::addresses::{
    MemoryAddress, PageSize, PhysicalAddress, PhysicalPage, Size4K, VirtualPage,
};
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
pub fn load_pt_load_segments_hi(
    elf_bytes: &[u8],
    hdr: &ElfHeader,
) -> Result<Vec<LoadedSegMap>, ElfLoaderError> {
    let mut maps = Vec::new();

    for seg in &hdr.segments {
        if seg.memsz == 0 {
            continue;
        }

        // LMA math
        let seg_vaddr = seg.vaddr;
        let lma = seg_vaddr
            .as_u64()
            .checked_sub(KERNEL_BASE)
            .ok_or(ElfLoaderError::PointerArithmetic)?;

        let phys_start = PHYS_LOAD
            .checked_add(lma)
            .ok_or(ElfLoaderError::PointerArithmetic)?;
        let phys_end = phys_start
            .checked_add(seg.memsz)
            .ok_or(ElfLoaderError::PointerArithmetic)?;

        // Page-rounded allocation window (physical)
        let alloc_start = MemoryAddress::new(phys_start)
            .align_down::<Size4K>()
            .as_u64();
        let alloc_end = align_up_u64(phys_end, Size4K::SIZE);
        let pages = ((alloc_end - alloc_start) / Size4K::SIZE) as usize;

        let mem_type = if seg.flags.execute() {
            MemoryType::LOADER_CODE
        } else {
            MemoryType::LOADER_DATA
        };

        // Reserve at the *physical address* we computed (UEFI AllocatePages at address)
        let ptr = boot::allocate_pages(AllocateType::Address(alloc_start), mem_type, pages)
            .map_err(ElfLoaderError::PhysicalAllocationFailed)?;
        // This is a physical address returned by UEFI:
        let phys_base = PhysicalAddress::from_nonnull(ptr);

        // Zero full in-memory size (BSS tail)
        let mem_len = usize::try_from(seg.memsz).map_err(|_| ElfLoaderError::AddressOutOfBounds)?;
        let in_seg_off = phys_start - alloc_start; // offset *within first page* to seg start
        let dst = (phys_base.as_u64() + in_seg_off) as *mut u8;
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

        // Build mapping info from the *VMA* perspective (page-rounded)
        let vaddr_page = seg_vaddr.page::<Size4K>(); // page-aligned VA base
        let vaddr_end_u64 = seg_vaddr
            .as_u64()
            .checked_add(seg.memsz)
            .ok_or(ElfLoaderError::PointerArithmetic)?;
        let vaddr_end_aligned = align_up_u64(vaddr_end_u64, Size4K::SIZE);
        let map_len = vaddr_end_aligned - vaddr_page.base().as_u64();

        maps.push(LoadedSegMap {
            vaddr_page,
            phys_page: PhysicalPage::<Size4K>::from_addr(phys_base),
            map_len,
            flags: seg.flags,
        });
    }

    Ok(maps)
}

#[derive(Clone, Copy)]
pub struct LoadedSegMap {
    /// page-aligned VMA start used for mapping
    pub vaddr_page: VirtualPage<Size4K>,
    /// page-aligned physical base actually allocated
    pub phys_page: PhysicalPage<Size4K>,
    /// bytes to map from `vaddr_page` (page-rounded)
    pub map_len: u64,
    /// ELF `p_flags` (`PF_X`, `PF_W`)
    pub flags: PFlags,
}

#[inline]
const fn align_up_u64(x: u64, a: u64) -> u64 {
    // 'a' must be a power of two
    (x + (a - 1)) & !(a - 1)
}
