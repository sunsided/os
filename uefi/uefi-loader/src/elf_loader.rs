//! UEFI kernel segment loader (higher-half VMA, physical LMA)
//!
use crate::elf_parser::{ElfHeader, LoadSegment};
use core::{cmp, ptr};
use uefi::boot::{AllocateType, MemoryType};
use uefi::{Status, boot};

const KERNEL_BASE: u64 = 0xffffffff80000000;
const PAGE_SIZE: u64 = 4096;

// ELF PF_* flags (standard)
const PF_X: u32 = 0x1;
const PF_W: u32 = 0x2;
const PF_R: u32 = 0x4;

// ELF segment types
const PT_LOAD: u32 = 1;

#[inline(always)]
fn align_down(x: u64, a: u64) -> u64 {
    x & !(a - 1)
}
#[inline(always)]
fn align_up(x: u64, a: u64) -> u64 {
    (x + a - 1) & !(a - 1)
}

#[derive(Debug, Clone, Copy)]
pub struct LoadedSeg {
    /// VMA start (where it executes)
    pub vaddr: u64,
    /// LMA physical start (where bytes reside in RAM)
    pub paddr: u64,
    /// File payload size
    pub filesz: u64,
    /// In-memory size (includes .bss tail)
    pub memsz: u64,
    /// ELF PF_* flags
    pub flags: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct LoadedKernel {
    /// Physical range [phys_start, phys_end) that holds all loaded segments.
    pub phys_start: u64,
    pub phys_end: u64,
    /// Virtual range [vma_start, vma_end) spanned by all PT_LOADs (rounded to 2 MiB).
    pub vma_start: u64,
    pub vma_end: u64,
    /// ELF entry point (VMA) to jump to after paging is set up.
    pub entry_vma: u64,
}

pub struct LoadedKernelWithSegs {
    pub meta: LoadedKernel,
    /// Per-segment mapping info (useful for setting page permissions)
    pub segs: alloc::vec::Vec<LoadedSeg>,
}

/// Load all PT_LOAD segments to their **physical** LMAs (`paddr` from ELF).
/// Returns spans needed to build page tables (map VMA→phys) plus per-segment info.
pub fn load_pt_load_segments(
    elf_bytes: &[u8],
    hdr: &ElfHeader,
) -> Result<LoadedKernelWithSegs, Status> {
    if hdr.segments.is_empty() {
        return Err(Status::UNSUPPORTED);
    }

    // Keep only PT_LOAD segments with nonzero memsz
    let mut segs: alloc::vec::Vec<LoadedSeg> = hdr
        .segments
        .iter()
        .filter(|s| s.p_type == PT_LOAD && s.memsz != 0)
        .map(|s| LoadedSeg {
            vaddr: s.vaddr,
            paddr: s.paddr,
            filesz: s.filesz,
            memsz: s.memsz,
            flags: s.flags,
        })
        .collect();

    if segs.is_empty() {
        return Err(Status::UNSUPPORTED);
    }

    // Basic validations + compute VMA span
    let mut vma_min = u64::MAX;
    let mut vma_max = 0u64;

    for (i, seg) in segs.iter().enumerate() {
        // filesz <= memsz
        if seg.filesz > seg.memsz {
            return Err(Status::UNSUPPORTED);
        }

        // paddr must match vaddr - KERNEL_BASE (consistency with your linker script)
        if seg.vaddr < KERNEL_BASE || seg.paddr != seg.vaddr - KERNEL_BASE {
            return Err(Status::UNSUPPORTED);
        }

        // Segment alignment expectations: p_align power-of-two and >= page
        // (We can’t see p_align here because we dropped it; if you want, keep it in LoadedSeg and validate.)
        // For safety we enforce page alignment on start addresses we actually allocate.
        if (seg.vaddr & (PAGE_SIZE - 1)) != 0 || (seg.paddr & (PAGE_SIZE - 1)) != 0 {
            // Not strictly required by ELF for vaddr/paddr, but simplifies exact-address allocation.
            // If this triggers, round alloc_start down and keep the in-segment offset (we do this anyway).
        }

        vma_min = cmp::min(vma_min, seg.vaddr);
        vma_max = cmp::max(
            vma_max,
            seg.vaddr
                .checked_add(seg.memsz)
                .ok_or(Status::UNSUPPORTED)?,
        );

        // Optional: ensure segments don't overlap in VMA (helps catch bad link scripts)
        for prev in &segs[..i] {
            let a0 = prev.vaddr;
            let a1 = prev.vaddr + prev.memsz;
            let b0 = seg.vaddr;
            let b1 = seg.vaddr + seg.memsz;
            if a0 < b1 && b0 < a1 {
                return Err(Status::UNSUPPORTED);
            }
        }
    }

    // Round VMA span to 2 MiB boundaries (handy for 2 MiB mapping later)
    let vma_start_2m = align_down(vma_min, 2 * 1024 * 1024);
    let vma_end_2m = align_up(vma_max, 2 * 1024 * 1024);

    // Load each segment at its LMA (physical)
    let mut phys_min = u64::MAX;
    let mut phys_max = 0u64;

    for seg in &segs {
        let lma = seg.paddr;
        let phys_end = lma.checked_add(seg.memsz).ok_or(Status::UNSUPPORTED)?;
        let alloc_start = align_down(lma, PAGE_SIZE);
        let alloc_end = align_up(phys_end, PAGE_SIZE);
        let pages = ((alloc_end - alloc_start) / PAGE_SIZE) as usize;

        // Pick a sensible UEFI memory type
        let mem_type = if (seg.flags & PF_X) != 0 {
            MemoryType::LOADER_CODE
        } else {
            MemoryType::LOADER_DATA
        };

        // Allocate at the **exact physical address** (LMA).
        let ptr = boot::allocate_pages(AllocateType::Address(alloc_start), mem_type, pages)
            .map_err(|_| Status::UNSUPPORTED)?;
        let base = ptr.as_ptr() as u64;

        // In-segment destination pointer for the first byte of this segment
        let in_seg_off = (lma - alloc_start) as usize;
        let dst_seg_ptr = (base as usize + in_seg_off) as *mut u8;

        // Zero entire memsz (covers .bss tail)
        let mem_len = usize::try_from(seg.memsz).map_err(|_| Status::UNSUPPORTED)?;
        unsafe { ptr::write_bytes(dst_seg_ptr, 0, mem_len) };

        // Copy file payload (filesz bytes from file offset)
        if seg.filesz != 0 {
            let file_len = usize::try_from(seg.filesz).map_err(|_| Status::UNSUPPORTED)?;
            let src_off = usize::try_from(
                hdr.segments
                    .iter()
                    .find(|s| s.vaddr == seg.vaddr && s.paddr == seg.paddr)
                    .ok_or(Status::UNSUPPORTED)?
                    .offset,
            )
            .map_err(|_| Status::UNSUPPORTED)?;
            let src_end = src_off.checked_add(file_len).ok_or(Status::UNSUPPORTED)?;
            if src_end > elf_bytes.len() {
                return Err(Status::UNSUPPORTED);
            }
            unsafe {
                ptr::copy_nonoverlapping(elf_bytes.as_ptr().add(src_off), dst_seg_ptr, file_len);
            }
        }

        phys_min = cmp::min(phys_min, alloc_start);
        phys_max = cmp::max(phys_max, alloc_end);
    }

    if phys_min >= phys_max {
        return Err(Status::UNSUPPORTED);
    }

    Ok(LoadedKernelWithSegs {
        meta: LoadedKernel {
            phys_start: phys_min,
            phys_end: phys_max,
            vma_start: vma_start_2m,
            vma_end: vma_end_2m,
            entry_vma: hdr.entry, // jump to this (after paging switch)
        },
        segs,
    })
}
