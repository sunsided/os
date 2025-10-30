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

mod map_size;

pub use crate::address_space::map_size::MapSize;
use crate::addresses::{
    MemoryAddressOffset, PhysicalAddress, PhysicalPage, Size1G, Size2M, Size4K, VirtualAddress,
};
use crate::page_table::pd::{PageDirectory, PdEntryKind};
use crate::page_table::pdpt::{PageDirectoryPointerTable, PdptEntryKind};
use crate::page_table::pml4::PageMapLevel4;
use crate::page_table::pt::{PageTable, PtEntry};
use crate::{FrameAlloc, PageEntryBits, PhysMapper, read_cr3_phys};

/// Handle to a single, concrete address space.
pub struct AddressSpace<'m, M: PhysMapper> {
    root: PhysicalPage<Size4K>, // PML4 frame
    mapper: &'m M,
}

/// The PML4 root page for an [`AddressSpace`].
pub type RootPage = PhysicalPage<Size4K>;

impl<'m, M: PhysMapper> AddressSpace<'m, M> {
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
    pub const fn root_page(&self) -> RootPage {
        self.root
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

    /// Borrow a [`PageTable`] (PT) in this frame.
    ///
    /// Convenience wrapper for [`PhysMapper::pt_mut`.
    #[inline]
    pub(crate) fn pt_mut(&self, page: PhysicalPage<Size4K>) -> &mut PageTable {
        self.mapper.pt_mut(page)
    }

    /// Map **one** page at `va → pa` with size `S` and `leaf_flags`.
    ///
    /// - Non-leaf links are created with `nonleaf_flags` (e.g., present+writable).
    /// - Alignment is asserted (debug) via typed wrappers.
    ///
    /// # Errors
    /// - Propagates allocation failures from [`ensure_chain`](Self::ensure_chain).
    pub fn map_one<A: FrameAlloc, S: MapSize>(
        &self,
        alloc: &mut A,
        va: VirtualAddress,
        pa: PhysicalAddress,
        nonleaf_flags: PageEntryBits,
        leaf_flags: PageEntryBits,
    ) -> Result<(), &'static str> {
        debug_assert_eq!(pa.offset::<S>().as_u64(), 0, "physical address not aligned");

        let leaf_tbl = S::ensure_chain_for(self, alloc, va, nonleaf_flags)?;
        S::set_leaf(self, leaf_tbl, va, pa, leaf_flags);
        Ok(())
    }

    /// Unmap a single **4 KiB** page at `va`. Returns Err if missing.
    pub fn unmap_one(&self, va: VirtualAddress) -> Result<(), &'static str> {
        let (i4, i3, i2, i1) = crate::page_table::split_indices(va);

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
        let (i4, i3, i2, i1) = crate::page_table::split_indices(va);

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
