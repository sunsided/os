//! # Address Space (x86-64, PML4-rooted)
//!
//! Strongly-typed helpers to build and manipulate a **single** virtual address
//! space (tree rooted at a PML4). This complements the typed paging layers
//! (`PageMapLevel4`, `PageDirectoryPointerTable`, `PageDirectory`, `PageTable`).
//!
//! ## Highlights
//!
//! - `AddressSpace::ensure_chain` to allocate/link missing intermediate tables
//!   down to the level implied by the target page size.
//! - `AddressSpace::map_one` to install one mapping (4 KiB / 2 MiB / 1 GiB).
//! - `AddressSpace::unmap_one` to clear a single 4 KiB PTE.
//! - `AddressSpace::query` to translate a VA to PA (handles huge pages).
//! - `AddressSpace::activate` to load CR3 with this space’s root.
//!
//! ## Design
//!
//! - Non-leaf entries are created with caller-provided **non-leaf flags**
//!   (typically: present + writable, US as needed). Leaf flags come from the
//!   mapping call. We never silently set US/GLOBAL/NX; the caller decides.
//! - Uses `PhysicalPage<Size4K>` for page-table frames, and `VirtualAddress` /
//!   `PhysicalAddress` for endpoints. Alignment is asserted via typed helpers.
//! - Keeps `unsafe` confined to mapping a physical frame to a typed table
//!   through the `PhysMapper`.
//!
//! ## Safety
//!
//! - Mutating active mappings requires appropriate **TLB maintenance** (e.g.,
//!   `invlpg` per page or CR3 reload).
//! - The provided `PhysMapper` must yield **writable** references to table frames.

use crate::PageEntryBits;
use crate::addr2::{
    MemoryAddressOffset, PageSize, PhysicalAddress, PhysicalPage, Size1G, Size2M, Size4K,
    VirtualAddress,
};
use crate::table2::pd::PdEntry;
use crate::table2::pd::{L2Index, PageDirectory, PdEntryKind};
use crate::table2::pdpt::{L3Index, PageDirectoryPointerTable, PdptEntry, PdptEntryKind};
use crate::table2::pml4::{L4Index, PageMapLevel4, Pml4Entry};
use crate::table2::pt::{L1Index, PageTable, PtEntry};

/// Minimal allocator that hands out **4 KiB** page-table frames.
pub trait FrameAlloc {
    /// Allocate a zeroed 4 KiB page suitable for a page-table.
    fn alloc_4k(&mut self) -> Option<PhysicalPage<Size4K>>;
}

/// Mapper capable of temporarily viewing physical frames as typed tables.
pub trait PhysMapper {
    /// Map a 4 KiB physical frame and get a **mutable** reference to type `T`.
    ///
    /// The implementation must ensure that the returned reference aliases the
    /// mapped frame, and that writes reach memory.
    unsafe fn phys_to_mut<T>(&self, at: PhysicalAddress) -> &mut T;
}

/// Handle to a single, concrete address space.
pub struct AddressSpace<'m, M: PhysMapper> {
    root: PhysicalPage<Size4K>, // PML4 frame
    mapper: &'m M,
}

impl<'m, M: PhysMapper> AddressSpace<'m, M> {
    /// Create a view for `root` using `mapper`. The PML4 is expected to be valid or zeroed.
    #[inline]
    pub const fn new(mapper: &'m M, root: PhysicalPage<Size4K>) -> Self {
        Self { root, mapper }
    }

    /// Physical page of the PML4.
    #[inline]
    pub const fn root_page(&self) -> PhysicalPage<Size4K> {
        self.root
    }

    /// Borrow the PML4 as a typed table.
    #[inline]
    fn pml4_mut(&self) -> &mut PageMapLevel4 {
        unsafe { self.mapper.phys_to_mut::<PageMapLevel4>(self.root.base()) }
    }

    /// Map a PDPT/PD/PT frame to its typed view.
    #[inline]
    fn pdpt_mut(&self, page: PhysicalPage<Size4K>) -> &mut PageDirectoryPointerTable {
        unsafe {
            self.mapper
                .phys_to_mut::<PageDirectoryPointerTable>(page.base())
        }
    }

    #[inline]
    fn pd_mut(&self, page: PhysicalPage<Size4K>) -> &mut PageDirectory {
        unsafe { self.mapper.phys_to_mut::<PageDirectory>(page.base()) }
    }

    #[inline]
    fn pt_mut(&self, page: PhysicalPage<Size4K>) -> &mut PageTable {
        unsafe { self.mapper.phys_to_mut::<PageTable>(page.base()) }
    }

    /// Ensure the non-leaf chain for `va` exists down to the level implied by `size`.
    ///
    /// Returns the **target table page** to write the leaf into and the **level**.
    ///
    /// - For 1 GiB: returns the PDPT page (you will write the PDPTE leaf).
    /// - For 2 MiB: returns the PD page (you will write the PDE leaf).
    /// - For 4 KiB: returns the PT page (you will write the PTE leaf).
    ///
    /// Newly created non-leaf entries are initialized with `nonleaf_flags`.
    ///
    /// # Errors
    /// - `"oom: pdpt" / "oom: pd" / "oom: pt"` when the allocator fails.
    pub fn ensure_chain<A: FrameAlloc, S: PageSize>(
        &self,
        alloc: &mut A,
        va: VirtualAddress,
        nonleaf_flags: PageEntryBits,
    ) -> Result<(PhysicalPage<Size4K>, EnsureTarget), &'static str> {
        let i4 = L4Index::from(va);
        let i3 = L3Index::from(va);
        let i2 = L2Index::from(va);

        // L4 → L3
        let pml4 = self.pml4_mut();
        let e4 = pml4.get(i4);
        let pdpt_page = if let Some(next) = e4.next_table() {
            next
        } else {
            let f = alloc.alloc_4k().ok_or("oom: pdpt")?;
            // zero and link
            *self.pdpt_mut(f) = PageDirectoryPointerTable::zeroed();
            pml4.set(i4, Pml4Entry::make(f, nonleaf_flags));
            f
        };

        // Stop at L3 for 1 GiB leaves:
        if Size1G::SIZE == S::SIZE {
            return Ok((pdpt_page, EnsureTarget::L3For1G));
        }

        // L3 → L2
        let pdpt = self.pdpt_mut(pdpt_page);
        let e3 = pdpt.get(i3);
        let pd_page = match e3.kind() {
            Some(PdptEntryKind::NextPageDirectory(pd, _)) => pd,
            Some(PdptEntryKind::Leaf1GiB(_, _)) => {
                // conflicting huge leaf → split (allocate PD)
                let f = alloc.alloc_4k().ok_or("oom: pd")?;
                *self.pd_mut(f) = PageDirectory::zeroed();
                pdpt.set(i3, PdptEntry::make_next(f, nonleaf_flags));
                f
            }
            None => {
                let f = alloc.alloc_4k().ok_or("oom: pd")?;
                *self.pd_mut(f) = PageDirectory::zeroed();
                pdpt.set(i3, PdptEntry::make_next(f, nonleaf_flags));
                f
            }
        };

        // Stop at L2 for 2 MiB leaves:
        if Size2M::SIZE == S::SIZE {
            return Ok((pd_page, EnsureTarget::L2For2M));
        }

        // L2 → L1
        let pd = self.pd_mut(pd_page);
        let e2 = pd.get(i2);
        let pt_page = match e2.kind() {
            Some(PdEntryKind::NextPageTable(pt, _)) => pt,
            Some(PdEntryKind::Leaf2MiB(_, _)) => {
                // conflicting huge leaf → split (allocate PT)
                let f = alloc.alloc_4k().ok_or("oom: pt")?;
                *self.pt_mut(f) = PageTable::zeroed();
                pd.set(i2, PdEntry::make_next(f, nonleaf_flags));
                f
            }
            None => {
                let f = alloc.alloc_4k().ok_or("oom: pt")?;
                *self.pt_mut(f) = PageTable::zeroed();
                pd.set(i2, PdEntry::make_next(f, nonleaf_flags));
                f
            }
        };

        Ok((pt_page, EnsureTarget::L1For4K))
    }

    /// Map **one** page at `va → pa` with size `S` and `leaf_flags`.
    ///
    /// - Non-leaf links are created with `nonleaf_flags` (e.g., present+writable).
    /// - Alignment is asserted (debug) via typed wrappers.
    ///
    /// # Errors
    /// - Propagates allocation failures from [`ensure_chain`].
    pub fn map_one<A: FrameAlloc, S: PageSize>(
        &self,
        alloc: &mut A,
        va: VirtualAddress,
        pa: PhysicalAddress,
        nonleaf_flags: PageEntryBits,
        leaf_flags: PageEntryBits,
    ) -> Result<(), &'static str> {
        // Assert physical alignment, then get the typed page base.
        debug_assert_eq!(pa.offset::<S>().as_u64(), 0, "physical address not aligned");
        let ppage = pa.page::<S>();

        let (leaf_tbl_page, target) = self.ensure_chain::<A, S>(alloc, va, nonleaf_flags)?;
        match target {
            EnsureTarget::L3For1G => {
                let pdpt = self.pdpt_mut(leaf_tbl_page);
                let idx = L3Index::from(va);
                let e = PdptEntry::make_1g(ppage.into(), leaf_flags);
                pdpt.set(idx, e);
            }
            EnsureTarget::L2For2M => {
                let pd = self.pd_mut(leaf_tbl_page);
                let idx = L2Index::from(va);
                let e = PdEntry::make_2m(ppage.into(), leaf_flags);
                pd.set(idx, e);
            }
            EnsureTarget::L1For4K => {
                let pt = self.pt_mut(leaf_tbl_page);
                let idx = L1Index::from(va);

                // convert back to 4K page
                let k4 = PhysicalPage::<Size4K>::from_addr(pa);
                let e = PtEntry::make_4k(k4, leaf_flags);
                pt.set(idx, e);
            }
        }
        Ok(())
    }

    /// Unmap a single **4 KiB** page at `va`. Returns Err if missing.
    pub fn unmap_one(&self, va: VirtualAddress) -> Result<(), &'static str> {
        let i4 = L4Index::from(va);
        let i3 = L3Index::from(va);
        let i2 = L2Index::from(va);
        let i1 = L1Index::from(va);

        // PML4
        let pml4 = self.pml4_mut();
        let e4 = pml4.get(i4);
        let Some(pdpt_page) = e4.next_table() else {
            return Err("missing: pml4");
        };

        // PDPT
        let pdpt = self.pdpt_mut(pdpt_page);
        let e3 = pdpt.get(i3);
        let Some(PdptEntryKind::NextPageDirectory(pd_page, _)) = e3.kind() else {
            return Err("missing: pdpt (or 1GiB leaf)");
        };

        // PD
        let pd = self.pd_mut(pd_page);
        let e2 = pd.get(i2);
        let Some(PdEntryKind::NextPageTable(pt_page, _)) = e2.kind() else {
            return Err("missing: pd (or 2MiB leaf)");
        };

        // PT
        let pt = self.pt_mut(pt_page);
        let e1 = pt.get(i1);
        if !e1.is_present() {
            return Err("missing: pte");
        }
        pt.set(i1, PtEntry::zero());
        Ok(())
    }

    /// Translate a `VirtualAddress` to `PhysicalAddress` if mapped.
    ///
    /// Handles 1 GiB and 2 MiB leaves by adding the appropriate **in-page offset**.
    #[must_use]
    pub fn query(&self, va: VirtualAddress) -> Option<PhysicalAddress> {
        let i4 = L4Index::from(va);
        let i3 = L3Index::from(va);
        let i2 = L2Index::from(va);
        let i1 = L1Index::from(va);

        // PML4
        let pml4 = self.pml4_mut();
        let e4 = pml4.get(i4);
        let pdpt_page = e4.next_table()?;

        // PDPT
        let pdpt = self.pdpt_mut(pdpt_page);
        match pdpt.get(i3).kind()? {
            PdptEntryKind::Leaf1GiB(base, _) => {
                let off: MemoryAddressOffset<Size1G> = va.offset::<Size1G>();
                Some(base.join(off))
            }
            PdptEntryKind::NextPageDirectory(pd_page, _) => {
                // continue
                let pd = self.pd_mut(pd_page);
                match pd.get(i2).kind()? {
                    PdEntryKind::Leaf2MiB(base, _) => {
                        let off: MemoryAddressOffset<Size2M> = va.offset::<Size2M>();
                        Some(base.join(off))
                    }
                    PdEntryKind::NextPageTable(pt_page, _) => {
                        let pt = self.pt_mut(pt_page);
                        let e1 = pt.get(i1);
                        let (base4k, _) = e1.page_4k()?;
                        let off: MemoryAddressOffset<Size4K> = va.offset::<Size4K>();
                        Some(base4k.join(off))
                    }
                }
            }
        }
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
