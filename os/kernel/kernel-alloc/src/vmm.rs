//! Minimal Virtual Memory Manager (VMM) for the kernel.
//!
//! This VMM provides basic map/unmap/query operations for a single address space.
//! It uses the `AddressSpace` abstraction from kernel-vmem, and FrameAlloc/PhysMapper
//! implementations from kernel-alloc.
//!
//! # Example
//! ```ignore
//! use kernel_alloc::{frame_alloc::BitmapFrameAlloc, phys_mapper::HhdmPhysMapper, vmm::Vmm};
//! let mut pmm = BitmapFrameAlloc::new();
//! let mapper = HhdmPhysMapper;
//! let mut vmm = Vmm::new(&mapper, &mut pmm);
//! // Map, unmap, query...
//! ```

use core::ptr::copy_nonoverlapping;
use kernel_vmem::address_space::{AddressSpaceMapOneError, AddressSpaceMapRegionError, MapSize};
use kernel_vmem::addresses::{PageSize, PhysicalAddress, Size4K, VirtualAddress, VirtualPage};
use kernel_vmem::{AddressSpace, FrameAlloc, PhysMapper};
use kernel_vmem::{VirtualMemoryPageBits, invalidate_tlb_page};

/// Minimal kernel virtual memory manager.
pub struct Vmm<'m, M: PhysMapper, A: FrameAlloc> {
    aspace: AddressSpace<'m, M>,
    alloc: &'m mut A,
}

impl<'m, M: PhysMapper, A: FrameAlloc> Vmm<'m, M, A> {
    /// # Safety
    /// - Must run at CPL0 with paging enabled.
    /// - Assumes CR3 points at a valid PML4 frame.
    pub unsafe fn from_current(mapper: &'m M, alloc: &'m mut A) -> Self {
        let aspace = unsafe { AddressSpace::from_current(mapper) };
        Self { aspace, alloc }
    }

    /// Translate VA→PA if mapped (handles 1G/2M/4K leaves with offset).
    #[must_use]
    pub fn query(&self, va: VirtualAddress) -> Option<PhysicalAddress> {
        self.aspace.query(va)
    }

    /// Map **one** page of size `S` with `leaf_flags`, creating parents with `nonleaf_flags`.
    pub fn map_one<S: MapSize>(
        &mut self,
        va: VirtualAddress,
        pa: PhysicalAddress,
        nonleaf_flags: VirtualMemoryPageBits,
        leaf_flags: VirtualMemoryPageBits,
    ) -> Result<(), AddressSpaceMapOneError> {
        self.aspace
            .map_one::<A, S>(self.alloc, va, pa, nonleaf_flags, leaf_flags)
    }

    /// Unmap a single **4 KiB** page at `va`. Returns Err if not a 4K mapping.
    pub fn unmap_one_4k(&mut self, va: VirtualAddress) -> Result<(), &'static str> {
        self.aspace.unmap_one(va)
    }

    /// Greedy region map: tiles `[va .. va+len)` to `[pa .. pa+len)` with 1G/2M/4K leaves.
    ///
    /// # Errors
    /// Allocation fails, e.g. due to OOM.
    pub fn map_region(
        &mut self,
        va: VirtualAddress,
        pa: PhysicalAddress,
        len: u64,
        nonleaf: VirtualMemoryPageBits,
        leaf: VirtualMemoryPageBits,
    ) -> Result<(), VmmError> {
        Ok(self
            .aspace
            .map_region(self.alloc, va, pa, len, nonleaf, leaf)?)
    }

    pub fn unmap_region(&mut self, va: VirtualAddress, len: u64) {
        self.aspace.unmap_region(va, len);
    }

    /// Convenience: map a **per-page** region using freshly allocated 4K frames (no PA contiguity).
    ///
    /// Leaves `guard` bytes at the beginning **unmapped** (for stacks).
    pub fn map_anon_4k_pages(
        &mut self,
        va_start: VirtualAddress,
        guard: u64,
        bytes: u64,
        nonleaf: VirtualMemoryPageBits,
        leaf: VirtualMemoryPageBits,
    ) -> Result<(), VmmError> {
        debug_assert!(guard.is_multiple_of(Size4K::SIZE) && bytes.is_multiple_of(Size4K::SIZE));

        let base = VirtualAddress::new(va_start.as_u64() + guard);
        let pages = bytes / Size4K::SIZE;

        for i in 0..pages {
            let va = VirtualAddress::new(base.as_u64() + i * Size4K::SIZE);
            let Some(pp) = self.alloc.alloc_4k() else {
                return Err(VmmError::OutOfMemory);
            };
            let pa = pp.base();
            self.aspace
                .map_one::<A, Size4K>(self.alloc, va, pa, nonleaf, leaf)?;
        }
        Ok(())
    }

    /// Copy a kernel slice into an already **mapped** user region.
    ///
    /// # Safety
    /// - `dst_user .. dst_user+src.len()` must be mapped and writable.
    /// - Same address space active (your current setup).
    pub unsafe fn copy_to_mapped_user(
        &mut self,
        dst_user: VirtualAddress,
        src: &[u8],
    ) -> Result<(), VmmError> {
        // Optional simple mapping check: walk each page boundary
        let start = dst_user.as_u64();
        let end = start
            .checked_add(src.len() as u64)
            .ok_or(VmmError::InvalidRange)?;

        let mut probe = start & !(Size4K::SIZE - 1);
        while probe < end {
            if self.query(VirtualAddress::new(probe)).is_none() {
                return Err(VmmError::Unmapped);
            }
            probe = probe.saturating_add(Size4K::SIZE);
        }

        // Raw copy: kernel can write US=1 pages
        let dst = dst_user.as_u64() as *mut u8;
        let srcp = src.as_ptr();
        unsafe {
            copy_nonoverlapping(srcp, dst, src.len());
        }
        Ok(())
    }

    /// Change per-page protection from RW to RX by unmapping & remapping with the same PA.
    /// Works for 4K pages created by `map_anon_4k_pages`.
    pub fn make_region_rx(
        &mut self,
        va_start: VirtualAddress,
        len: u64,
        nonleaf: VirtualMemoryPageBits,
        leaf_rx: VirtualMemoryPageBits,
    ) -> Result<(), VmmError> {
        let pages = (len + Size4K::SIZE - 1) / Size4K::SIZE;
        for i in 0..pages {
            let va = VirtualAddress::new(va_start.as_u64() + i * Size4K::SIZE);
            let Some(pa) = self.query(va) else {
                return Err(VmmError::Unmapped);
            };

            // Ensure 4K mapping
            self.unmap_one_4k(va).map_err(VmmError::UnmapFailed)?;
            self.map_one::<Size4K>(va, pa_aligned_4k(pa), nonleaf, leaf_rx)?;
            self.invlpg(VirtualPage::<Size4K>::containing_address(va));
        }
        Ok(())
    }

    /// Invalidate one VA on this CPU (if you modify the active space).
    #[inline]
    pub fn invlpg(&self, page: VirtualPage<Size4K>) {
        unsafe { invalidate_tlb_page(page) }
    }

    /// Flush the entire TLB.
    pub unsafe fn local_tlb_flush_all(&self) {
        unsafe {
            let cr3: u64;
            core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
            core::arch::asm!("mov cr3, {}", in(reg) cr3, options(nostack, preserves_flags));
        }
    }
}

#[inline]
fn pa_aligned_4k(pa: PhysicalAddress) -> PhysicalAddress {
    // PhysicalAddress::new(pa.as_u64() & !(Size4K::SIZE - 1))
    pa.page::<Size4K>().base()
}

#[derive(Debug, thiserror::Error)]
pub enum VmmError {
    #[error("out of memory")]
    OutOfMemory,
    #[error("unaligned allocation")]
    Unaligned,
    #[error("invalid range")]
    InvalidRange,
    #[error("access to unmapped memory")]
    Unmapped,
    #[error("failed to unmap memory: {0}")]
    UnmapFailed(&'static str),
}

impl From<AddressSpaceMapOneError> for VmmError {
    fn from(value: AddressSpaceMapOneError) -> Self {
        match value {
            AddressSpaceMapOneError::OutOfMemory(_) => Self::OutOfMemory,
        }
    }
}

impl From<AddressSpaceMapRegionError> for VmmError {
    fn from(value: AddressSpaceMapRegionError) -> Self {
        match value {
            AddressSpaceMapRegionError::OutOfMemory(_) => Self::OutOfMemory,
            AddressSpaceMapRegionError::Unaligned(_, _) => Self::Unaligned,
        }
    }
}
