//! # Virtual Memory Setup for Kernel loading

#![allow(clippy::inline_always)]

use crate::elf::parser::ElfHeader;
use crate::elf::{PF_W, PF_X};
use kernel_info::memory::{HHDM_BASE, KERNEL_BASE, PHYS_LOAD};
use kernel_vmem::{
    AddressSpace, FrameAlloc, MemoryPageFlags, PageSize, PageTable, PhysAddr, PhysMapper, VirtAddr,
    align_down, is_aligned,
};
use uefi::boot;
use uefi::boot::{AllocateType, MemoryType};

const PAGE_4K: u64 = 4 * 1024;
const PAGE_2M: u64 = 2 * 1024 * 1024;
const PAGE_1G: u64 = 1024 * 1024 * 1024;

const PAGE_4K_MASK: u64 = PAGE_4K - 1;
const PAGE_2M_MASK: u64 = PAGE_2M - 1;
const PAGE_1G_MASK: u64 = PAGE_1G - 1;

struct BsFrameAlloc;

impl FrameAlloc for BsFrameAlloc {
    fn alloc_4k(&mut self) -> Option<PhysAddr> {
        let pages = 1usize;
        let mem_type = MemoryType::LOADER_DATA;
        let ptr = boot::allocate_pages(AllocateType::AnyPages, mem_type, pages).ok()?;
        let pa = ptr.as_ptr() as u64;
        // Zero the frame
        unsafe {
            core::ptr::write_bytes(ptr.as_ptr(), 0, 4096);
        }
        Some(PhysAddr(pa))
    }
}

struct LoaderPhysMapper;

impl PhysMapper for LoaderPhysMapper {
    unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T {
        // In the loader we *temporarily* assume identity for page-table frames we allocate.
        // (They’re in low memory and UEFI page tables map them.)
        unsafe { &mut *(pa.0 as *mut T) }
    }
}

/// Derive physical for a kernel VMA per your linker `AT()`
#[inline(always)]
const fn kernel_va_to_pa(va: u64) -> PhysAddr {
    PhysAddr(PHYS_LOAD + (va - KERNEL_BASE))
}

/// The physical address of the Page-Map Level-4 Table (PML4).
type Pml4Phys = PhysAddr;

#[inline(always)]
const fn can_use_2m(va: u64, pa: u64, remaining: u64) -> bool {
    is_aligned(va, PAGE_2M) && is_aligned(pa, PAGE_2M) && remaining >= PAGE_2M
}

pub fn create_kernel_pagetables(elf: &ElfHeader) -> Result<Pml4Phys, &'static str> {
    let mapper = LoaderPhysMapper;
    let mut alloc = BsFrameAlloc;

    // Create the root PML4 (Page-Map Level-4 Table)
    let pml4_phys = alloc.alloc_4k().ok_or("OOM: PML4")?;
    unsafe {
        mapper.phys_to_mut::<PageTable>(pml4_phys).zero();
    }

    let aspace = AddressSpace::new(&mapper, pml4_phys);

    // Map the higher-half kernel segments
    for seg in &elf.segments {
        if seg.memsz == 0 {
            continue;
        }

        let va_start = seg.vaddr;
        let va_end = seg.vaddr.checked_add(seg.memsz).ok_or("overflow")?;

        // Choose flags from ELF p_flags
        let flags = MemoryPageFlags::GLOBAL
            .with_writable_if((seg.flags & PF_W) != 0)
            .with_executable_if((seg.flags & PF_X) != 0);

        // Try 2 MiB chunks when aligned, otherwise 4 KiB
        let mut va = align_down(va_start, PAGE_2M);
        while va < va_end {
            let remaining = va_end - va;
            let pa = kernel_va_to_pa(va).0;

            if can_use_2m(va, pa, remaining) {
                aspace.map_one(
                    &mut alloc,
                    VirtAddr(va),
                    PhysAddr(pa),
                    PageSize::Size2M,
                    flags,
                )?;
                va += PAGE_2M;
            } else {
                aspace.map_one(
                    &mut alloc,
                    VirtAddr(va),
                    PhysAddr(pa),
                    PageSize::Size4K,
                    flags,
                )?;
                va += PAGE_4K;
            }
        }
    }

    // HHDM: map first 1 GiB of physical memory (easy bring-up)
    // VA = HHDM_BASE, PA = 0
    aspace.map_one(
        &mut alloc,
        VirtAddr(HHDM_BASE),
        PhysAddr(0),
        PageSize::Size1G,
        MemoryPageFlags::WRITABLE | MemoryPageFlags::GLOBAL | MemoryPageFlags::NX,
    )?;

    // Identity map first 2 MiB for the trampoline/loader code after CR3 switch.
    aspace.map_one(
        &mut alloc,
        VirtAddr(0),
        PhysAddr(0),
        PageSize::Size2M,
        MemoryPageFlags::WRITABLE | MemoryPageFlags::GLOBAL | MemoryPageFlags::NX,
    )?;

    Ok(pml4_phys)
}
