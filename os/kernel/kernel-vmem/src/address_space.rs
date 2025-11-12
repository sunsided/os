//! # Address Space (x86-64, PML4-rooted)
//!
//! Strongly-typed helpers to build and manipulate a **single** virtual address
//! space (tree rooted at a PML4). This complements the typed paging layers
//! (`PageMapLevel4`, `PageDirectoryPointerTable`, `PageDirectory`, `PageTable`).
//!
//! ## Highlights
//!
//! - [`AddressSpace::map_one`] to install one mapping (4 KiB / 2 MiB / 1 GiB).
//! - [`AddressSpace::unmap_one`] to clear a single 4 KiB PTE.
//! - [`AddressSpace::query`] to translate a VA to PA (handles huge pages).
//! - [`AddressSpace::activate`] to load CR3 with this space’s root.
//!
//! ## Design
//!
//! - Non-leaf entries are created with caller-provided **non-leaf flags**
//!   (typically: present + writable, US as needed). Leaf flags come from the
//!   mapping call. We never silently set US/GLOBAL/NX; the caller decides.
//! - Uses [`PhysicalPage<Size4K>`] for page-table frames, and [`VirtualAddress`] /
//!   [`PhysicalAddress`] for endpoints. Alignment is asserted via typed helpers.
//! - Keeps `unsafe` confined to mapping a physical frame to a typed table
//!   through the [`PhysMapper`].
//!
//! ## Safety
//!
//! - Mutating active mappings requires appropriate **TLB maintenance** (e.g.,
//!   `invlpg` per page or CR3 reload).
//! - The provided `PhysMapper` must yield **writable** references to table frames.

mod map_size;

pub use crate::address_space::map_size::MapSize;
use crate::address_space::map_size::MapSizeEnsureChainError;
use crate::addresses::{
    PageSize, PhysicalAddress, PhysicalPage, Size1G, Size2M, Size4K, VirtualAddress,
};
use crate::bits::VirtualMemoryPageBits;
use crate::page_table::pd::{L2Index, PageDirectory, PdEntry, PdEntryKind};
use crate::page_table::pdpt::{L3Index, PageDirectoryPointerTable, PdptEntry, PdptEntryKind};
use crate::page_table::pml4::{L4Index, PageMapLevel4, Pml4Entry};
use crate::page_table::pt::{L1Index, PageTable, PtEntry4k};
use crate::{PhysFrameAlloc, PhysMapper, PhysMapperExt, read_cr3_phys};
use log::{trace, warn};

/// Handle to a single, concrete address space.
pub struct AddressSpace<'m, M: PhysMapper> {
    root: PhysicalPage<Size4K>, // PML4 frame
    mapper: &'m M,
}

#[derive(Debug, thiserror::Error)]
pub enum AddressSpaceError {
    #[error("Failed to create a new address space due to OOM in the allocator")]
    OutOfMemory,
}

/// The PML4 root page for an [`AddressSpace`].
pub type RootPage = PhysicalPage<Size4K>;

impl<'m, M: PhysMapper> AddressSpace<'m, M> {
    #[allow(clippy::missing_errors_doc)]
    pub fn new(mapper: &'m M, alloc: &mut impl PhysFrameAlloc) -> Result<Self, AddressSpaceError> {
        let pml4 = alloc.alloc_4k().ok_or(AddressSpaceError::OutOfMemory)?;
        unsafe {
            let table = mapper.phys_to_mut::<PageMapLevel4>(pml4.base());
            *table = PageMapLevel4::zeroed();
        }

        let mut me = Self::from_root(mapper, pml4);

        // Copy kernel half PML4 entries from current kernel PML4
        let kern = unsafe { AddressSpace::from_current(mapper) };
        me.clone_upper_half_from(&kern);
        Ok(me)
    }

    /// View the **currently active** address space by reading CR3.
    ///
    /// # Safety
    /// - Must run at CPL0 with paging enabled.
    /// - Assumes CR3 points at a valid PML4 frame.
    #[inline]
    pub unsafe fn from_current(mapper: &'m M) -> Self {
        let root_pa = unsafe { read_cr3_phys() };
        let root = PhysicalPage::<Size4K>::from_addr(root_pa);
        Self { root, mapper }
    }

    /// If you already know the root frame (e.g., from your own allocator),
    /// you can still use the explicit constructor:
    #[inline]
    pub const fn from_root(mapper: &'m M, root: PhysicalPage<Size4K>) -> Self {
        Self { root, mapper }
    }

    /// Load CR3 with this address space’s root.
    ///
    /// # Safety
    /// You must ensure the CPU paging state (CR0/CR4/EFER) and code/data mappings
    /// are consistent with the target space. Consider reloading CR3 or issuing
    /// `invlpg` after changes to active mappings.
    #[inline]
    pub unsafe fn activate(&self) {
        let cr3 = self.root.base().as_u64();
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) cr3, options(nostack, preserves_flags));
        }
    }

    /// Physical page of the PML4.
    #[inline]
    #[must_use]
    pub const fn root_page(&self) -> RootPage {
        self.root
    }

    /// Borrow a [`PageTable`] (PT) in this frame.
    ///
    /// Convenience wrapper for [`PhysMapper::pt_mut`].
    #[inline]
    pub(crate) fn pt_mut(&self, page: PhysicalPage<Size4K>) -> &mut PageTable {
        self.mapper.pt_mut(page)
    }

    /// Translate a `VirtualAddress` to `PhysicalAddress` if mapped.
    ///
    /// Handles 1 GiB and 2 MiB leaves by adding the appropriate **in-page offset**.
    #[must_use]
    pub fn query(&self, va: VirtualAddress) -> Option<PhysicalAddress> {
        match self.walk(va) {
            WalkResult::Leaf1G { base, .. } => {
                let off = va.offset::<Size1G>();
                Some(base.join(off))
            }
            WalkResult::Leaf2M { base, .. } => {
                let off = va.offset::<Size2M>();
                Some(base.join(off))
            }
            WalkResult::L1 { pte, .. } => {
                let (base4k, _fl) = pte.page_4k()?;
                let off = va.offset::<Size4K>();
                Some(base4k.join(off))
            }
            WalkResult::Missing => None,
        }
    }

    /// Map **one** page at `va → pa` with size `S` and `leaf_flags`.
    ///
    /// - Non-leaf links are created with `nonleaf_flags` (e.g., present+writable).
    /// - Alignment is asserted (debug) via typed wrappers.
    ///
    /// # Errors
    /// - An Out of Memory error occurred in one of the tables.
    pub fn map_one<A: PhysFrameAlloc, S: MapSize>(
        &self,
        alloc: &mut A,
        va: VirtualAddress,
        pa: PhysicalAddress,
        nonleaf_flags: VirtualMemoryPageBits,
        leaf_flags: VirtualMemoryPageBits,
    ) -> Result<(), AddressSpaceMapOneError> {
        debug_assert_eq!(pa.offset::<S>().as_u64(), 0, "physical address not aligned");
        let leaf_tbl = S::ensure_chain_for(self, alloc, va, nonleaf_flags).inspect_err(|err| {
            warn!("physical address mapping error: {err:?}");
        })?;

        trace!("Mapped one {} page at VA={va} -> PA={pa}", S::as_str());
        S::set_leaf(self, leaf_tbl, va, pa, leaf_flags);
        Ok(())
    }

    /// Unmap a single **4 KiB** page at `va`. Returns Err if missing.
    ///
    /// # Errors
    /// - Invalid tables
    // TODO: Refactor to error type
    pub fn unmap_one(&self, va: VirtualAddress) -> Result<(), &'static str> {
        match self.walk(va) {
            WalkResult::L1 { pt, i1, pte } => {
                if !pte.present() {
                    return Err("missing: pte");
                }

                trace!("Unmapped VA={va}");
                pt.set_zero(i1);
                Ok(())
            }
            WalkResult::Leaf2M { .. } => Err("found 2MiB leaf (not a 4KiB mapping)"),
            WalkResult::Leaf1G { .. } => Err("found 1GiB leaf (not a 4KiB mapping)"),
            WalkResult::Missing => Err("missing: chain"),
        }
    }

    /// Greedy region mapping: tiles `[virt_start .. virt_start+len)` onto
    /// `[phys_start .. phys_start+len)` using 1G / 2M / 4K pages as alignment permits.
    ///
    /// - Non-leaf links use `nonleaf_flags` (e.g. present|writable).
    /// - Leaves use `leaf_flags` (e.g. perms, NX, GLOBAL).
    ///
    /// # Errors
    /// - Propagates OOMs from intermediate table allocation.
    pub fn map_region<A: PhysFrameAlloc>(
        &self,
        alloc: &mut A,
        virt_start: VirtualAddress,
        phys_start: PhysicalAddress,
        len: u64,
        nonleaf_flags: VirtualMemoryPageBits,
        leaf_flags: VirtualMemoryPageBits,
    ) -> Result<(), AddressSpaceMapRegionError> {
        let mut off = 0u64;
        while off < len {
            let va = VirtualAddress::new(virt_start.as_u64() + off);
            let pa = PhysicalAddress::new(phys_start.as_u64() + off);
            let remain = len - off;

            // Try 1 GiB
            if (va.as_u64() & (Size1G::SIZE - 1) == 0)
                && (pa.as_u64() & (Size1G::SIZE - 1) == 0)
                && remain >= Size1G::SIZE
            {
                self.map_one::<A, Size1G>(alloc, va, pa, nonleaf_flags, leaf_flags)?;
                off += Size1G::SIZE;
                continue;
            }

            // Try 2 MiB
            if (va.as_u64() & (Size2M::SIZE - 1) == 0)
                && (pa.as_u64() & (Size2M::SIZE - 1) == 0)
                && remain >= Size2M::SIZE
            {
                self.map_one::<A, Size2M>(alloc, va, pa, nonleaf_flags, leaf_flags)?;
                off += Size2M::SIZE;
                continue;
            }

            // Fall back to 4 KiB
            if (va.as_u64() & (Size4K::SIZE - 1) == 0) && (pa.as_u64() & (Size4K::SIZE - 1) == 0) {
                self.map_one::<A, Size4K>(alloc, va, pa, nonleaf_flags, leaf_flags)?;
                off += Size4K::SIZE;
                continue;
            }

            return Err(AddressSpaceMapRegionError::Unaligned(va, pa));
        }
        Ok(())
    }

    /// Greedy unmap of a region: clears whole 1G/2M leaves when aligned, otherwise 4K PTEs.
    /// (Does not collapse tables; that's a separate optimization pass.)
    pub fn unmap_region(&self, virt_start: VirtualAddress, len: u64) {
        let mut off = 0u64;
        while off < len {
            let va = VirtualAddress::new(virt_start.as_u64() + off);
            match self.walk(va) {
                WalkResult::Leaf1G { pdpt, i3, .. }
                    if (va.as_u64() & (Size1G::SIZE - 1) == 0) && (len - off) >= Size1G::SIZE =>
                {
                    // Clear the PDPTE
                    pdpt.set_zero(i3);
                    off += Size1G::SIZE;
                }
                WalkResult::Leaf2M { pd, i2, .. }
                    if (va.as_u64() & (Size2M::SIZE - 1) == 0) && (len - off) >= Size2M::SIZE =>
                {
                    // Clear the PDE
                    pd.set_zero(i2);
                    off += Size2M::SIZE;
                }
                WalkResult::L1 { pt, i1, pte } => {
                    if pte.present() {
                        pt.set_zero(i1);
                    }
                    off += Size4K::SIZE;
                }
                // Missing entry: treat as unmapped 4K and advance to avoid infinite loop,
                // or
                // Leaf size larger than remaining or unaligned start: fall back to 4K step.
                _ => off += Size4K::SIZE,
            }
        }
    }

    /// Walks the whole tree and frees empty tables (PT/PD/PDPT). Does not merge leaves.
    #[allow(clippy::similar_names)]
    pub fn collapse_empty_tables<F: PhysFrameAlloc>(&self, free: &mut F) {
        let pml4 = self.pml4_mut();

        // For every L4 entry:
        for i4 in 0..512 {
            let e4 = pml4.get(L4Index::new(i4));
            let Some(pdpt_page) = e4.next_table() else {
                continue;
            };
            let pdpt = self.pdpt_mut(pdpt_page);

            // For every L3 entry:
            let mut used_l3 = false;
            for i3 in 0..512 {
                match pdpt.get(L3Index::new(i3)).kind() {
                    Some(PdptEntryKind::Leaf1GiB(_, _)) => {
                        used_l3 = true;
                    }
                    Some(PdptEntryKind::NextPageDirectory(pd_page, _)) => {
                        let pd = self.pd_mut(pd_page);
                        // For every L2 entry:
                        let mut used_l2 = false;
                        for i2 in 0..512 {
                            match pd.get(L2Index::new(i2)).kind() {
                                Some(PdEntryKind::Leaf2MiB(_, _)) => {
                                    used_l2 = true;
                                }
                                Some(PdEntryKind::NextPageTable(pt_page, _)) => {
                                    let pt = self.pt_mut(pt_page);
                                    let mut any_present = false;
                                    for i1 in 0..512 {
                                        if pt.get(L1Index::new(i1)).present() {
                                            any_present = true;
                                            break;
                                        }
                                    }
                                    if any_present {
                                        used_l2 = true;
                                    } else {
                                        // free PT
                                        pd.set(L2Index::new(i2), PdEntry::zero());
                                        free.free_4k(pt_page);
                                    }
                                }
                                None => {}
                            }
                        }
                        // If PD ended up empty (no leaves / no child PTs left), free it.
                        if used_l2 {
                            used_l3 = true;
                        } else {
                            pdpt.set(L3Index::new(i3), PdptEntry::zero());
                            free.free_4k(pd_page);
                        }
                    }
                    None => {}
                }
            }

            // If PDPT is now empty, free it.
            if !used_l3 {
                pml4.set(L4Index::new(i4), Pml4Entry::zero());
                free.free_4k(pdpt_page);
            }
        }
    }

    /// Internal walker: resolves VA to the point it terminates.
    #[allow(clippy::similar_names)]
    fn walk(&self, va: VirtualAddress) -> WalkResult<'_> {
        let (i4, i3, i2, i1) = crate::page_table::split_indices(va);

        // PML4
        let pml4 = self.pml4_mut();
        let Some(pdpt_page) = pml4.get(i4).next_table() else {
            return WalkResult::Missing;
        };

        // PDPT
        let pdpt = self.pdpt_mut(pdpt_page);
        match pdpt.get(i3).kind() {
            Some(PdptEntryKind::Leaf1GiB(base, _fl)) => WalkResult::Leaf1G { base, pdpt, i3 },
            Some(PdptEntryKind::NextPageDirectory(pd_page, _fl)) => {
                // PD
                let pd = self.pd_mut(pd_page);
                match pd.get(i2).kind() {
                    Some(PdEntryKind::Leaf2MiB(base, _fl)) => WalkResult::Leaf2M { base, pd, i2 },
                    Some(PdEntryKind::NextPageTable(pt_page, _fl)) => {
                        // PT
                        let pt = self.pt_mut(pt_page);
                        let pte = pt.get(i1);
                        WalkResult::L1 { pt, i1, pte }
                    }
                    None => WalkResult::Missing,
                }
            }
            None => WalkResult::Missing,
        }
    }

    /// Borrow the [`PageMapLevel4`] (PML4) as a typed table.
    ///
    /// Convenience wrapper for [`PhysMapper::pml4_mut`] at the [`root_page`](Self::root_page).
    #[inline]
    pub(crate) fn pml4_mut(&self) -> &mut PageMapLevel4 {
        self.mapper.pml4_mut(self.root)
    }

    /// Borrow a [`PageDirectoryPointerTable`] (PDPT) in this frame
    ///
    /// Convenience wrapper for [`PhysMapper::pdpt_mut`].
    #[inline]
    pub(crate) fn pdpt_mut(&self, page: PhysicalPage<Size4K>) -> &mut PageDirectoryPointerTable {
        self.mapper.pdpt_mut(page)
    }

    /// Borrow a [`PageDirectory`] (PD) in this frame
    ///
    /// Convenience wrapper for [`PhysMapper::pd_mut`].
    #[inline]
    pub(crate) fn pd_mut(&self, page: PhysicalPage<Size4K>) -> &mut PageDirectory {
        self.mapper.pd_mut(page)
    }

    /// Zeroes the [`PageDirectoryPointerTable`] (PDPT) in this frame
    ///
    /// Convenience wrapper for [`PhysMapper::zero_pdpt`].
    #[inline]
    pub(crate) fn zero_pdpt(&self, page: PhysicalPage<Size4K>) {
        self.mapper.zero_pdpt(page);
    }

    /// Zeroes the [`PageDirectory`] (PD) in this frame
    ///
    /// Convenience wrapper for [`PhysMapper::zero_pd`].
    #[inline]
    pub(crate) fn zero_pd(&self, page: PhysicalPage<Size4K>) {
        self.mapper.zero_pd(page);
    }

    /// Zeroes the [`PageTable`] (PD) in this frame
    ///
    /// Convenience wrapper for [`PhysMapper::zero_pt`].
    #[inline]
    pub(crate) fn zero_pt(&self, page: PhysicalPage<Size4K>) {
        self.mapper.zero_pt(page);
    }

    /// Copy kernel upper-half PML4 entries (slots 256..=511) from `src` into `self`,
    /// aliasing the same kernel page-table subtrees. Does not touch lower levels.
    fn clone_upper_half_from(&mut self, src: &Self) {
        let dst_pa = self.root_page();
        let src_pa = src.root_page();

        // Map both PML4 pages via HHDM
        let dst_l4: &mut PageMapLevel4 = unsafe { self.mapper.phys_to_mut(dst_pa.base()) };
        let src_l4: &mut PageMapLevel4 = unsafe { self.mapper.phys_to_mut(src_pa.base()) };

        // Kernel half: indices 256..=511 (works for 48-bit and LA57)
        for i in (256..512).map(L4Index::new) {
            let e = src_l4.get(i);
            if e.present() {
                debug_assert!(!e.user(), "kernel PML4E must have US=0");
            }
            dst_l4.set(i, e);
        }
    }

    /// Post-bringup clearing.
    pub fn clear_lower_half(&mut self) {
        let root = self.root_page();

        // Map both PML4 pages via HHDM
        let l4: &mut PageMapLevel4 = unsafe { self.mapper.phys_to_mut(root.base()) };

        // User half: indices 0..256
        for i in (0..256).map(L4Index::new) {
            l4.set(i, Pml4Entry::zero());
        }
    }
}

/// A mapping error.
#[derive(Debug, Copy, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AddressSpaceMapOneError {
    #[error(transparent)]
    OutOfMemory(#[from] MapSizeEnsureChainError),
}

/// A mapping error.
#[derive(Debug, Copy, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AddressSpaceMapRegionError {
    #[error(transparent)]
    OutOfMemory(#[from] MapSizeEnsureChainError),
    #[error("unaligned va/pa for remaining size: {0:?} -> {1:?}")]
    Unaligned(VirtualAddress, PhysicalAddress),
}

impl From<AddressSpaceMapOneError> for AddressSpaceMapRegionError {
    fn from(e: AddressSpaceMapOneError) -> Self {
        match e {
            AddressSpaceMapOneError::OutOfMemory(e) => Self::OutOfMemory(e),
        }
    }
}

/// Target table/level produced by `ensure_chain`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EnsureTarget {
    /// You will write a **PDPTE** (1 GiB leaf).
    L3For1G,
    /// You will write a **PDE** (2 MiB leaf).
    L2For2M,
    /// You will write a **PTE** (4 KiB leaf).
    L1For4K,
}

/// The result of a table walk.
#[allow(dead_code)]
enum WalkResult<'a> {
    /// Hit a 1 GiB leaf at PDPT.
    Leaf1G {
        base: PhysicalPage<Size1G>,
        pdpt: &'a mut PageDirectoryPointerTable,
        i3: L3Index,
    },
    /// Hit a 2 MiB leaf at PD.
    Leaf2M {
        base: PhysicalPage<Size2M>,
        pd: &'a mut PageDirectory,
        i2: L2Index,
    },
    /// Reached PT (L1) with its index and current entry.
    L1 {
        pt: &'a mut PageTable,
        i1: L1Index,
        pte: PtEntry4k,
    },
    /// Missing somewhere in the chain.
    Missing,
}
