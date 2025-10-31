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

use kernel_vmem::UnifiedEntry;
use kernel_vmem::address_space::AddressSpaceMapRegionError;
use kernel_vmem::addresses::{PhysicalAddress, VirtualAddress};
use kernel_vmem::{AddressSpace, FrameAlloc, PhysMapper};

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

    /// # Errors
    /// Allocation fails, e.g. due to OOM.
    pub fn map_region(
        &mut self,
        va: VirtualAddress,
        pa: PhysicalAddress,
        len: u64,
        nonleaf: UnifiedEntry,
        leaf: UnifiedEntry,
    ) -> Result<(), AddressSpaceMapRegionError> {
        self.aspace
            .map_region(self.alloc, va, pa, len, nonleaf, leaf)
    }

    pub fn unmap_region(&mut self, va: VirtualAddress, len: u64) {
        self.aspace.unmap_region(va, len);
    }

    #[must_use]
    pub fn query(&self, va: VirtualAddress) -> Option<PhysicalAddress> {
        self.aspace.query(va)
    }
}
