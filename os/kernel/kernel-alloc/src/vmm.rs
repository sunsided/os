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

use kernel_vmem::{
    AddressSpace, FrameAlloc, MemoryPageFlags, PageSize, PhysAddr, PhysMapper, VirtAddr,
};

/// Minimal kernel virtual memory manager.
pub struct Vmm<'a, M: PhysMapper, F: FrameAlloc> {
    aspace: AddressSpace<'a, M>,
    pmm: &'a mut F,
}

impl<'a, M: PhysMapper, F: FrameAlloc> Vmm<'a, M, F> {
    /// Create a new VMM for the given address space root (current CR3).
    pub fn new(mapper: &'a M, pmm: &'a mut F) -> Self {
        let cr3 = unsafe { kernel_vmem::read_cr3_phys() };
        let aspace = AddressSpace::new(mapper, cr3);
        Self { aspace, pmm }
    }

    /// Query the physical address mapped to a virtual address.
    #[must_use]
    pub fn query(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.aspace.query(va)
    }

    /// Map a virtual address to a physical frame with the given flags and size.
    #[allow(clippy::missing_errors_doc)]
    pub fn map(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        size: PageSize,
        flags: MemoryPageFlags,
    ) -> Result<(), &'static str> {
        self.aspace.map_one(self.pmm, va, pa, size, flags)
    }

    /// Unmap a virtual address (single page).
    #[allow(clippy::missing_errors_doc)]
    pub fn unmap(&mut self, va: VirtAddr) -> Result<(), &'static str> {
        self.aspace.unmap_one(va)
    }

    /// Map a region of physical memory to a virtual address range using the largest possible pages.
    /// Handles alignment and size automatically (2 MiB pages, then 4 KiB for head/tail).
    #[allow(clippy::missing_errors_doc)]
    pub fn map_region(
        &mut self,
        virt_start: VirtAddr,
        phys_start: PhysAddr,
        size: u64,
        flags: MemoryPageFlags,
    ) -> Result<(), &'static str> {
        let mut offset = 0u64;
        let mut remaining = size;
        while remaining > 0 {
            let va = virt_start + offset;
            let pa = phys_start + offset;
            // Try 2 MiB page if both addrs and remaining are aligned
            if (va.as_u64() & ((1 << 21) - 1) == 0)
                && (pa.as_u64() & ((1 << 21) - 1) == 0)
                && remaining >= 2 * 1024 * 1024
            {
                self.map(va, pa, PageSize::Size2M, flags)?;
                offset += 2 * 1024 * 1024;
                remaining -= 2 * 1024 * 1024;
            } else if (va.as_u64() & ((1 << 12) - 1) == 0)
                && (pa.as_u64() & ((1 << 12) - 1) == 0)
                && remaining >= 4096
            {
                self.map(va, pa, PageSize::Size4K, flags)?;
                offset += 4096;
                remaining -= 4096;
            } else {
                return Err("Unaligned mapping or unsupported page size");
            }
        }
        Ok(())
    }

    /// Unmap a region of virtual memory using the largest possible pages.
    /// Handles alignment and size automatically (2 MiB pages, then 4 KiB for head/tail).
    #[allow(clippy::missing_errors_doc)]
    pub fn unmap_region(
        &mut self,
        virt_start: VirtAddr,
        phys_start: PhysAddr,
        size: u64,
    ) -> Result<(), &'static str> {
        let mut offset = 0u64;
        let mut remaining = size;
        while remaining > 0 {
            let va = virt_start + offset;
            let pa = phys_start + offset;
            // Try 2 MiB page if both addrs and remaining are aligned
            if (va.as_u64() & ((1 << 21) - 1) == 0)
                && (pa.as_u64() & ((1 << 21) - 1) == 0)
                && remaining >= 2 * 1024 * 1024
            {
                self.unmap(va)?;
                offset += 2 * 1024 * 1024;
                remaining -= 2 * 1024 * 1024;
            } else if (va.as_u64() & ((1 << 12) - 1) == 0)
                && (pa.as_u64() & ((1 << 12) - 1) == 0)
                && remaining >= 4096
            {
                self.unmap(va)?;
                offset += 4096;
                remaining -= 4096;
            } else {
                return Err("Unaligned unmapping or unsupported page size");
            }
        }
        Ok(())
    }
}
