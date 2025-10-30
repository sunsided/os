//! # Virtual Memory Setup for Kernel loading (new typed API)

use crate::elf::loader::LoadedSegMap;
use crate::elf::{PF_W, PF_X};
use kernel_info::memory::{HHDM_BASE /*KERNEL_BASE,*/ /*PHYS_LOAD*/};

use kernel_vmem::{
    AddressSpace, FrameAlloc, PageEntryBits, PhysMapper,
    addresses::{PhysicalAddress, PhysicalPage, Size1G, Size2M, Size4K, VirtualAddress},
};

use kernel_vmem::address_space::AddressSpaceMapOneError;
use kernel_vmem::addresses::PageSize;
use uefi::boot;
use uefi::boot::{AllocateType, MemoryType};

#[inline]
const fn align_up_u64(x: u64, a: u64) -> u64 {
    (x + (a - 1)) & !(a - 1)
}

/// UEFI-backed frame allocator: hands out zeroed 4 KiB frames.
struct BsFrameAlloc;

impl FrameAlloc for BsFrameAlloc {
    fn alloc_4k(&mut self) -> Option<PhysicalPage<Size4K>> {
        let pages = 1usize;
        let mem_type = MemoryType::LOADER_DATA;
        let ptr = boot::allocate_pages(AllocateType::AnyPages, mem_type, pages).ok()?;
        // Zero the frame (UEFI gives physical RAM identity-mapped in loader)
        unsafe {
            core::ptr::write_bytes(ptr.as_ptr(), 0, 4096);
        }
        let pa = PhysicalAddress::from_nonnull(ptr);
        Some(PhysicalPage::<Size4K>::from_addr(pa))
    }

    fn free_4k(&mut self, pa: PhysicalPage<Size4K>) {
        if let Some(nn) = core::ptr::NonNull::new(pa.base().as_u64() as *mut u8) {
            let _ = unsafe { boot::free_pages(nn, 1) };
        }
    }
}

/// Loader mapper: treat low physical memory frames as directly accessible.
/// Safety: valid only in the UEFI loader context where those frames are mapped.
struct LoaderPhysMapper;

impl PhysMapper for LoaderPhysMapper {
    unsafe fn phys_to_mut<T>(&self, at: PhysicalAddress) -> &mut T {
        unsafe { &mut *(at.as_u64() as *mut T) }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::similar_names
)]
pub fn create_kernel_pagetables(
    kernel_maps: &[LoadedSegMap],
    tramp_code_va: VirtualAddress,
    tramp_code_len: usize,
    tramp_stack_base_phys: PhysicalAddress,
    tramp_stack_size_bytes: usize,
    boot_info_ptr_va: VirtualAddress,
) -> Result<PhysicalAddress, KernelPageTableError> {
    let mapper = LoaderPhysMapper;
    let mut alloc = BsFrameAlloc;

    // Root PML4
    let pml4_phys = alloc
        .alloc_4k()
        .ok_or(KernelPageTableError::OutOfMemoryPml4)?;
    mapper.zero_pml4(pml4_phys);

    let aspace = AddressSpace::from_root(&mapper, pml4_phys);
    let pml4_phys = aspace.root_page().base();

    // Common flags
    // Non-leaf: present + writable (no NX on non-leaves)
    let nonleaf_flags: PageEntryBits = PageEntryBits::new().with_present(true).with_writable(true);

    // Map each PT_LOAD segment
    for m in kernel_maps {
        let mut cur_va = m.vaddr_page.base(); // VirtualAddress (page-aligned)
        let end_u64 = m
            .vaddr_page
            .base()
            .as_u64()
            .checked_add(m.map_len)
            .ok_or(KernelPageTableError::SegmentLengthOverflow)?;

        while cur_va.as_u64() < end_u64 {
            // Compute PA = phys_page.base + (cur_va - vaddr_page.base)
            let off = cur_va.as_u64() - m.vaddr_page.base().as_u64();
            let cur_pa = PhysicalAddress::new(m.phys_page.base().as_u64() + off);

            // Leaf flags from ELF PF_*:
            // start with present + global; add writable if PF_W; add NX if !PF_X
            let leaf_flags = PageEntryBits::new()
                .with_present(true)
                .with_global_translation(true)
                .with_writable((m.flags & PF_W) != 0)
                .with_no_execute((m.flags & PF_X) == 0);

            // Try 2 MiB leaf where legal
            let remaining = end_u64 - cur_va.as_u64();
            let can_2m = (cur_va.as_u64() & (Size2M::SIZE - 1) == 0)
                && (cur_pa.as_u64() & (Size2M::SIZE - 1) == 0)
                && remaining >= Size2M::SIZE;

            if can_2m {
                aspace.map_one::<_, Size2M>(
                    &mut alloc,
                    cur_va,
                    cur_pa,
                    nonleaf_flags,
                    leaf_flags,
                )?;
                cur_va = VirtualAddress::new(cur_va.as_u64() + Size2M::SIZE);
            } else {
                aspace.map_one::<_, Size4K>(
                    &mut alloc,
                    cur_va,
                    cur_pa,
                    nonleaf_flags,
                    leaf_flags,
                )?;
                cur_va = VirtualAddress::new(cur_va.as_u64() + Size4K::SIZE);
            }
        }
    }

    // HHDM: map first 1 GiB VA = HHDM_BASE → PA = 0, NX + writable + global
    {
        let hhdm_va = VirtualAddress::new(HHDM_BASE);
        let zero_pa = PhysicalAddress::new(0);
        let leaf = PageEntryBits::new()
            .with_present(true)
            .with_writable(true)
            .with_global_translation(true)
            .with_no_execute(true);
        aspace.map_one::<_, Size1G>(&mut alloc, hhdm_va, zero_pa, nonleaf_flags, leaf)?;
    }

    // Identity map first 2 MiB of low VA so the trampoline keeps executing after mov cr3.
    // Executable (i.e., NX not set), global, writable.
    {
        let va0 = VirtualAddress::new(0);
        let pa0 = PhysicalAddress::new(0);
        let leaf = PageEntryBits::new()
            .with_present(true)
            .with_writable(true)
            .with_global_translation(true);
        aspace.map_one::<_, Size2M>(&mut alloc, va0, pa0, nonleaf_flags, leaf)?;
    }

    // Identity map the trampoline stack (4 KiB, NX)
    {
        let start = tramp_stack_base_phys.as_u64() & !(Size4K::SIZE - 1);
        let end = align_up_u64(
            tramp_stack_base_phys
                .as_u64()
                .checked_add(tramp_stack_size_bytes as u64)
                .ok_or(KernelPageTableError::TrampolineStackRangeOverflow)?,
            Size4K::SIZE,
        );
        let leaf = PageEntryBits::new()
            .with_present(true)
            .with_writable(true)
            .with_global_translation(true)
            .with_no_execute(true);

        let mut pa = start;
        while pa < end {
            let va = VirtualAddress::new(pa); // identity
            let phys = PhysicalAddress::new(pa);
            aspace.map_one::<_, Size4K>(&mut alloc, va, phys, nonleaf_flags, leaf)?;
            pa += Size4K::SIZE;
        }
    }

    // Identity map the trampoline code (4 KiB, executable)
    {
        let start = tramp_code_va.page::<Size4K>().base().as_u64();
        let end = align_up_u64(
            tramp_code_va
                .as_u64()
                .checked_add(tramp_code_len as u64)
                .ok_or(KernelPageTableError::TrampolineCodeRangeOverflow)?,
            Size4K::SIZE,
        );
        let leaf = PageEntryBits::new()
            .with_present(true)
            .with_global_translation(true)
            .with_no_execute(false) // executable (no NX)
            .with_writable(false);

        let mut addr = start;
        while addr < end {
            let va = VirtualAddress::new(addr);
            let pa = PhysicalAddress::new(addr); // identity
            aspace.map_one::<_, Size4K>(&mut alloc, va, pa, nonleaf_flags, leaf)?;
            addr += Size4K::SIZE;
        }
    }

    // Identity map just the BootInfo pointer page (4 KiB, NX)
    {
        let bi_page = boot_info_ptr_va.page::<Size4K>().base();
        let leaf = PageEntryBits::new()
            .with_present(true)
            .with_writable(true)
            .with_global_translation(true)
            .with_no_execute(true);
        aspace.map_one::<_, Size4K>(
            &mut alloc,
            bi_page,
            PhysicalAddress::new(bi_page.as_u64()),
            nonleaf_flags,
            leaf,
        )?;
    }

    Ok(pml4_phys)
}

#[derive(Debug, thiserror::Error)]
pub enum KernelPageTableError {
    #[error("out of memory in PML4")]
    OutOfMemoryPml4,
    #[error("PT_LOAD segment length overflow")]
    SegmentLengthOverflow,
    /// Address arithmetic overflow while mapping trampoline stack memory range.
    #[error("stack range overflow")]
    TrampolineStackRangeOverflow,
    /// Address arithmetic overflow while mapping trampoline code memory range.
    #[error("trampoline code overflow")]
    TrampolineCodeRangeOverflow,
    #[error(transparent)]
    OutOfMemoryPageTable(#[from] AddressSpaceMapOneError),
}
