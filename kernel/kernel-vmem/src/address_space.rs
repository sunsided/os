//! # Virtual Address Space

#![allow(dead_code)]

use crate::{
    Flags, FrameAlloc, PageSize, PageTable, PdPageTable, PdptPageTable, PhysAddr, PhysMapper,
    Pml4PageTable, PtPageTable, VirtAddr, apply_flags, get_table,
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

    /// Ensure the page-table chain exists down to the leaf level for `va`,
    /// allocating intermediate tables as needed.
    ///
    /// Returns `(leaf_phys, is_leaf_huge)` where:
    /// - for **1 GiB**: `leaf_phys = PDPT frame`, `is_leaf_huge = true`
    /// - for **2 MiB**: `leaf_phys = PD frame`, `is_leaf_huge = true`
    /// - for **4 KiB**: `leaf_phys = PT frame`, `is_leaf_huge = false`
    ///
    /// Any newly created intermediate entry is initialized with `PRESENT|WRITABLE`.
    ///
    /// # Errors
    /// - `"OOM for PDPT" / "OOM for PD" / "OOM for PT"` if the allocator runs out.
    ///
    /// # Safety & invariants
    /// - Caller must ensure `root` is the active PML4 of the current address space.
    /// - `map` must be able to provide a valid writable mapping for the involved tables.
    #[allow(clippy::similar_names)]
    #[inline]
    pub fn ensure_chain<A: FrameAlloc>(
        &self,
        alloc: &mut A,
        va: VirtAddr,
        size: PageSize,
    ) -> Result<(PhysAddr, bool), &'static str> {
        // PML4
        let pml4 = self.pml4();
        let e4 = pml4.entry_mut_by_va(va);
        let pdpt_phys = if e4.present() {
            PhysAddr(e4.addr())
        } else {
            let f = alloc.alloc_4k().ok_or("OOM for PDPT")?;
            as_pdpt(self.mapper, f).zero();
            pml4.link_pdpt(va, f);
            f
        };

        // PDPT
        let pdpt = as_pdpt(self.mapper, pdpt_phys);
        if matches!(size, PageSize::Size1G) {
            // Caller will fill PDPTE (set PS + final flags).
            return Ok((pdpt_phys, true));
        }
        let e3 = pdpt.entry_mut_by_va(va);
        let pd_phys = if !e3.present() || e3.ps() {
            // no entry yet OR conflicting 1 GiB leaf → allocate PD
            let f = alloc.alloc_4k().ok_or("OOM for PD")?;
            as_pd(self.mapper, f).zero();
            pdpt.link_pd(va, f);
            f
        } else {
            PhysAddr(e3.addr())
        };

        // PD
        let pd = as_pd(self.mapper, pd_phys);
        if matches!(size, PageSize::Size2M) {
            // Caller will fill PDE (set PS + final flags).
            return Ok((pd_phys, true));
        }
        let e2 = pd.entry_mut_by_va(va);
        let pt_phys = if !e2.present() || e2.ps() {
            // no entry yet OR conflicting 2 MiB leaf → allocate PT
            let f = alloc.alloc_4k().ok_or("OOM for PT")?;
            as_pt(self.mapper, f).zero();
            pd.link_pt(va, f);
            f
        } else {
            PhysAddr(e2.addr())
        };

        // PT leaf for 4 KiB
        Ok((pt_phys, false))
    }

    /// Map a single page at `va → pa` with `size` and `flags`.
    ///
    /// `PRESENT` is added automatically; for huge pages `PS` is set automatically.
    ///
    /// ### Examples
    /// - Map a 4 KiB user page: `WRITABLE | USER` (+ NX if data, clear NX if code)
    /// - Map a kernel HHDM leaf: `WRITABLE | GLOBAL | NX`
    ///
    /// ### Alignment requirements
    /// - `pa` must be aligned to the page size.
    /// - `va` should be aligned to the page size (hardware allows unaligned
    ///   virtual addresses with appropriate offsets, but you almost never want that).
    ///
    /// # Safety
    /// - Caller must ensure `root` is the active tree and that replacing an existing
    ///   entry (e.g., splitting a huge page) is acceptable for the address range.
    /// - This routine does **not** flush TLBs; if you modify live mappings,
    ///   issue the appropriate `invlpg`/CR3 reload as needed.
    ///
    /// # Errors
    /// - Propagates allocation errors from `ensure_chain`.
    #[allow(clippy::missing_errors_doc)]
    #[inline]
    pub fn map_one<A: FrameAlloc>(
        &self,
        alloc: &mut A,
        va: VirtAddr,
        pa: PhysAddr,
        size: PageSize,
        mut flags: Flags,
    ) -> Result<(), &'static str> {
        flags |= Flags::PRESENT;

        // alignment checks for sanity
        debug_assert_eq!(pa.0 & ((1u64 << 12) - 1), 0, "phys not 4K aligned");
        if matches!(size, PageSize::Size2M) {
            debug_assert_eq!(pa.0 & ((1u64 << 21) - 1), 0, "phys not 2M aligned");
        }
        if matches!(size, PageSize::Size1G) {
            debug_assert_eq!(pa.0 & ((1u64 << 30) - 1), 0, "phys not 1G aligned");
        }

        unsafe {
            let (leaf_phys, is_huge_leaf) = self.ensure_chain(alloc, va, size)?;
            match size {
                PageSize::Size1G => {
                    // PDPTE leaf: phys bits 51:30, low 30 bits zero.
                    let pdpt = get_table::<M>(self.mapper, leaf_phys);
                    let e = pdpt.entry_mut(va.pdpt_index());
                    e.set_addr(pa.0);
                    apply_flags(e, flags | Flags::PS, true);
                }
                PageSize::Size2M => {
                    // PDE leaf: phys bits 51:21, low 21 bits zero.
                    let pd = get_table::<M>(self.mapper, leaf_phys);
                    let e = pd.entry_mut(va.pd_index());
                    e.set_addr(pa.0);
                    apply_flags(e, flags | Flags::PS, true);
                }
                PageSize::Size4K => {
                    // PTE leaf: phys bits 51:12, low 12 bits zero.
                    let pt = get_table::<M>(self.mapper, leaf_phys);
                    let e = pt.entry_mut(va.pt_index());
                    e.set_addr(pa.0);
                    apply_flags(e, flags, is_huge_leaf);
                }
            }
        }
        Ok(())
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

/// Map a physical frame as a [`Pml4PageTable`] typed table.
#[inline]
pub fn as_pml4<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut Pml4PageTable {
    unsafe {
        &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa))
            .cast::<Pml4PageTable>()
    }
}

/// Map a physical frame as a [`PdptPageTable`] typed table.
#[inline]
pub fn as_pdpt<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut PdptPageTable {
    unsafe {
        &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa))
            .cast::<PdptPageTable>()
    }
}

/// Map a physical frame as a [`PdPageTable`] typed table.
#[inline]
pub fn as_pd<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut PdPageTable {
    unsafe {
        &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa)).cast::<PdPageTable>()
    }
}

/// Map a physical frame as a [`PtPageTable`] typed table.
#[inline]
pub fn as_pt<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut PtPageTable {
    unsafe {
        &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa)).cast::<PtPageTable>()
    }
}
