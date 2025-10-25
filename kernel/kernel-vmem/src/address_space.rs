//! # Virtual Address Space

#![allow(dead_code)]

use crate::{
    Flags, FrameAlloc, PageSize, PhysAddr, PhysMapper, Pml4PageTable, VirtAddr, as_pml4,
    ensure_chain, map_one,
};

pub struct AddressSpace<'m, M: PhysMapper> {
    pub root_phys: PhysAddr,
    mapper: &'m M,
}

impl<'m, M: PhysMapper> AddressSpace<'m, M> {
    #[inline]
    pub const fn new(mapper: &'m M, root_phys: PhysAddr) -> Self {
        Self { root_phys, mapper }
    }

    #[inline]
    #[must_use]
    pub fn pml4(&self) -> &mut Pml4PageTable {
        as_pml4(self.mapper, self.root_phys)
    }

    #[allow(clippy::missing_errors_doc)]
    #[inline]
    pub fn ensure_chain<A: FrameAlloc>(
        &self,
        alloc: &mut A,
        va: VirtAddr,
        size: PageSize,
    ) -> Result<(PhysAddr, bool), &'static str> {
        ensure_chain(alloc, self.mapper, self.root_phys, va, size)
    }

    #[allow(clippy::missing_errors_doc)]
    #[inline]
    pub fn map_one<A: FrameAlloc>(
        &self,
        alloc: &mut A,
        va: VirtAddr,
        pa: PhysAddr,
        size: PageSize,
        flags: Flags,
    ) -> Result<(), &'static str> {
        map_one(alloc, self.mapper, self.root_phys, va, pa, size, flags)
    }

    /// Make this address space active (loader/kernel only).
    ///
    /// # Safety
    /// Not assessed.
    #[inline]
    pub unsafe fn activate(&self) {
        // enable PGE/NXE separately if you use them
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) self.root_phys.0, options(nostack, preserves_flags));
        }
    }
}
