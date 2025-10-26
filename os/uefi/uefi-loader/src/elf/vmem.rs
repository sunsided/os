//! # Virtual Memory Setup for Kernel loading

#![allow(clippy::inline_always)]

use crate::elf::loader::LoadedSegMap;
use crate::elf::parser::ElfHeader;
use crate::elf::{PF_W, PF_X};
use kernel_info::memory::{HHDM_BASE, KERNEL_BASE, PHYS_LOAD};
use kernel_qemu::qemu_trace;
use kernel_vmem::{
    AddressSpace, FrameAlloc, MemoryAddress, MemoryPageFlags, PageSize, PageTable, PhysAddr,
    PhysMapper, VirtAddr, align_down, align_up, is_aligned,
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
        Some(PhysAddr::new(MemoryAddress::new(pa)))
    }
}

struct LoaderPhysMapper;

impl PhysMapper for LoaderPhysMapper {
    unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T {
        // In the loader we *temporarily* assume identity for page-table frames we allocate.
        // (They’re in low memory and UEFI page tables map them.)
        unsafe { &mut *(pa.as_u64() as *mut T) }
    }
}

/// Derive physical for a kernel VMA per your linker `AT()`
#[inline(always)]
const fn kernel_va_to_pa(va: VirtAddr) -> PhysAddr {
    PhysAddr::new(MemoryAddress::new(PHYS_LOAD + (va.as_u64() - KERNEL_BASE)))
}

/// The physical address of the Page-Map Level-4 Table (PML4).
type Pml4Phys = PhysAddr;

#[inline(always)]
const fn can_use_2m(va: VirtAddr, pa: PhysAddr, remaining: u64) -> bool {
    is_aligned(va.as_addr(), PAGE_2M) && is_aligned(pa.as_addr(), PAGE_2M) && remaining >= PAGE_2M
}

pub fn create_kernel_pagetables(
    kernel_maps: &[LoadedSegMap],
    tramp_code_va: VirtAddr,
    tramp_code_len: usize,
    tramp_stack_base_phys: PhysAddr,
    tramp_stack_top_va: VirtAddr,
    tramp_stack_size_bytes: usize,
) -> Result<Pml4Phys, &'static str> {
    let mapper = LoaderPhysMapper;
    let mut alloc = BsFrameAlloc;

    // Create the root PML4 (Page-Map Level-4 Table)
    let pml4_phys = alloc.alloc_4k().ok_or("OOM: PML4")?;
    unsafe {
        mapper.phys_to_mut::<PageTable>(pml4_phys).zero();
    }

    let aspace = AddressSpace::new(&mapper, pml4_phys);

    // Map each loaded segment using (phys_page + (va - vaddr_page))
    for m in kernel_maps {
        let mut va = m.vaddr_page;
        let end = m.vaddr_page.as_u64() + m.map_len;

        let mut flags = MemoryPageFlags::GLOBAL;
        if (m.flags & PF_W) != 0 {
            flags |= MemoryPageFlags::WRITABLE;
        }
        if (m.flags & PF_X) == 0 {
            flags |= MemoryPageFlags::NX;
        } // leave NX *clear* iff PF_X set

        while va.as_u64() < end {
            let off = va - m.vaddr_page;
            let pa = m.phys_page.as_u64() + off;

            let can2m = (va.as_u64() & (PAGE_2M - 1) == 0)
                && (pa & (PAGE_2M - 1) == 0)
                && (end - va.as_u64()) >= PAGE_2M;

            if can2m {
                aspace.map_one(
                    &mut alloc,
                    VirtAddr::new(va),
                    PhysAddr::from_u64(pa),
                    PageSize::Size2M,
                    flags,
                )?;
                va += PAGE_2M;
            } else {
                aspace.map_one(
                    &mut alloc,
                    VirtAddr::new(va),
                    PhysAddr::from_u64(pa),
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
        VirtAddr::from_u64(HHDM_BASE),
        PhysAddr::from_u64(0),
        PageSize::Size1G,
        MemoryPageFlags::WRITABLE | MemoryPageFlags::GLOBAL | MemoryPageFlags::NX,
    )?;

    // Identity map first 2 MiB for the trampoline/loader code after CR3 switch.
    aspace.map_one(
        &mut alloc,
        VirtAddr::from_u64(0),
        PhysAddr::from_u64(0),
        PageSize::Size2M,
        MemoryPageFlags::WRITABLE | MemoryPageFlags::GLOBAL | MemoryPageFlags::NX,
    )?;

    // Identity map the trampoline stack (4 KiB leaves)
    {
        let start = align_down(tramp_stack_base_phys.as_addr(), PAGE_4K);
        let end = align_up(
            tramp_stack_base_phys
                .as_u64()
                .checked_add(tramp_stack_size_bytes as u64)
                .ok_or("stack range overflow")?
                .into(),
            PAGE_4K,
        );

        let mut pa = start;
        while pa < end {
            // Identity: VA == PA
            aspace.map_one(
                &mut alloc,
                VirtAddr::new(pa),
                PhysAddr::new(pa),
                PageSize::Size4K,
                MemoryPageFlags::WRITABLE | MemoryPageFlags::GLOBAL | MemoryPageFlags::NX,
            )?;
            pa += PAGE_4K;
        }
    }

    // Identity map the trampoline code
    {
        let start = align_down(tramp_code_va.as_addr(), PAGE_4K);
        let end = align_up(
            tramp_code_va
                .as_u64()
                .checked_add(tramp_code_len as u64)
                .ok_or("tramp code overflow")?
                .into(),
            PAGE_4K,
        );
        let mut pa = start;
        while pa < end {
            aspace.map_one(
                &mut alloc,
                VirtAddr(pa), // identity
                PhysAddr(pa),
                PageSize::Size4K,
                // Executable! keep NX clear; writable not needed:
                MemoryPageFlags::GLOBAL,
            )?;
            pa += PAGE_4K;
        }
    }

    Ok(pml4_phys)
}
