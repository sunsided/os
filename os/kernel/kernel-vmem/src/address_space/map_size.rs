//! # Memory Page Table Mapping Size
//!
//! This module defines the behavior of [`AddressSpace::map_one`](AddressSpace::map_one) for
//! different page sizes.
//!
//! The `MapSize` trait is implemented for each page size, and provides the
//! following methods:
//!
//! - `ensure_chain_for`: Given a virtual address, ensure that the non-leaf
//!   chain for that address down to the table that holds the leaf for the

use crate::addresses::{
    PageSize, PhysicalAddress, PhysicalPage, Size1G, Size2M, Size4K, VirtualAddress,
};
use crate::page_table::pd::{L2Index, PdEntry, PdEntryKind};
use crate::page_table::pdpt::{L3Index, PdptEntry, PdptEntryKind};
use crate::page_table::pml4::{L4Index, Pml4Entry};
use crate::page_table::pt::{L1Index, PtEntry};
use crate::unified2::UnifiedEntry;
use crate::{AddressSpace, FrameAlloc, PhysMapper};

/// # Page-size–directed mapping behavior
///
/// `MapSize` encodes, at the type level, how to:
/// 1) **ensure** the non-leaf page-table chain exists for a given virtual
///    address, and
/// 2) **install** the correct **leaf** entry for that page size.
///
/// Implementations for [`Size1G`], [`Size2M`], and [`Size4K`] decide **where to
/// stop the walk** and **which entry to write**, so callers don’t branch at
/// runtime. This keeps the mapping code zero-cost and compile-time checked.
///
/// ## What `ensure_chain_for` returns
///
/// It returns the **target table frame (4 KiB page)** into which you will write
/// the leaf entry for `Self`:
///
/// - For **1 GiB** pages (`Self = Size1G`): returns the **PDPT** frame
///   *(you will write a PDPTE with `PS=1`)*.
/// - For **2 MiB** pages (`Self = Size2M`): returns the **PD** frame
///   *(you will write a PDE with `PS=1`)*.
/// - For **4 KiB** pages (`Self = Size4K`): returns the **PT** frame
///   *(you will write a PTE with `PS=0`)*.
///
/// Newly created non-leaf entries are initialized with `nonleaf_flags`
/// (e.g., `present | writable`), and any **conflicting huge leaves are split**
/// on demand by allocating and linking the next-level table.
///
/// ## Typical flow
///
/// ```ignore
/// // Decide size with the type parameter S, no runtime branching:
/// let leaf_table = S::ensure_chain_for(aspace, alloc, va, nonleaf_flags)?;
/// S::set_leaf(aspace, leaf_table, va, pa, leaf_flags);
/// ```
///
/// ## Safety & alignment
///
/// - Physical alignment is **asserted (debug)** by callers via `pa.offset::<S>() == 0`.
/// - The mapper (`PhysMapper`) must yield **writable** views of table frames.
/// - If you mutate the **active** address space, perform the required **TLB
///   maintenance** (`invlpg` per page or CR3 reload).
pub trait MapSize: PageSize {
    /// Ensure that the non-leaf chain for `va` exists down to the table that
    /// holds the **leaf** for `Self`, allocating and linking intermediate
    /// tables as needed.
    ///
    /// ### Returns
    /// The 4 KiB **frame** (as `PhysicalPage<Size4K>`) of the table where the
    /// leaf for `Self` must be written:
    /// - `Size1G` → PDPT frame
    /// - `Size2M` → PD frame
    /// - `Size4K` → PT frame
    ///
    /// ### Behavior
    /// - Initializes newly allocated non-leaf tables to zeroed state and links
    ///   them with `nonleaf_flags`.
    /// - If a conflicting huge leaf is encountered at a higher level, it is
    ///   **split** by allocating the next-level table and relinking.
    ///
    /// ### Errors
    /// - `"oom: pdpt" / "oom: pd" / "oom: pt"` if allocating an intermediate
    ///   table frame fails.
    fn ensure_chain_for<A: FrameAlloc, M: PhysMapper>(
        aspace: &AddressSpace<M>,
        alloc: &mut A,
        va: VirtualAddress,
        nonleaf_flags: UnifiedEntry,
    ) -> Result<PhysicalPage<Size4K>, MapSizeEnsureChainError>;

    /// Install the **leaf** entry for `va → pa` in the `leaf_tbl_page`
    /// returned by [`ensure_chain_for`](Self::ensure_chain_for), with the given `leaf_flags`.
    ///
    /// - `Size1G`: writes a **PDPTE (PS=1)** into the PDPT at `va`.
    /// - `Size2M`: writes a **PDE   (PS=1)** into the PD   at `va`.
    /// - `Size4K`: writes a **PTE   (PS=0)** into the PT   at `va`.
    ///
    /// Callers should assert (in debug) that `pa` is aligned to `Self`:
    /// `debug_assert_eq!(pa.offset::<Self>().as_u64(), 0)`.
    fn set_leaf<M: PhysMapper>(
        aspace: &AddressSpace<M>,
        leaf_tbl_page: PhysicalPage<Size4K>,
        va: VirtualAddress,
        pa: PhysicalAddress,
        leaf_flags: UnifiedEntry,
    );
}

/// Error returned by [`MapSize::ensure_chain_for`] when allocating a new
/// intermediate table frame fails.
#[derive(Debug, Copy, Clone, PartialEq, Eq, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
pub enum MapSizeEnsureChainError {
    #[error("out of memory (PDPT)")]
    OomPdpt,
    #[error("out of memory (PD)")]
    OomPd,
    #[error("out of memory (PT)")]
    OomPt,
}

impl MapSize for Size1G {
    fn ensure_chain_for<A: FrameAlloc, M: PhysMapper>(
        aspace: &AddressSpace<M>,
        alloc: &mut A,
        va: VirtualAddress,
        nonleaf_flags: UnifiedEntry,
    ) -> Result<PhysicalPage<Size4K>, MapSizeEnsureChainError> {
        let i4 = L4Index::from(va);

        // L4 → L3
        let pml4 = aspace.pml4_mut();
        let e4 = pml4.get(i4);
        if let Some(pdpt_page) = e4.next_table() {
            return Ok(pdpt_page);
        }
        let f = alloc.alloc_4k().ok_or(MapSizeEnsureChainError::OomPdpt)?;
        aspace.zero_pdpt(f);
        pml4.set(i4, Pml4Entry::make(f, nonleaf_flags.to_pml4e()));
        Ok(f)
    }

    fn set_leaf<M: PhysMapper>(
        aspace: &AddressSpace<M>,
        leaf_tbl_page: PhysicalPage<Size4K>,
        va: VirtualAddress,
        pa: PhysicalAddress,
        leaf_flags: UnifiedEntry,
    ) {
        // require 1 GiB alignment in debug
        debug_assert_eq!(pa.offset::<Self>().as_u64(), 0);
        let pdpt = aspace.pdpt_mut(leaf_tbl_page);
        let idx = L3Index::from(va);
        let g1 = PhysicalPage::<Self>::from_addr(pa);
        pdpt.set(idx, PdptEntry::make_1g(g1, leaf_flags.to_pdpte_1g()));
    }
}

impl MapSize for Size2M {
    fn ensure_chain_for<A: FrameAlloc, M: PhysMapper>(
        aspace: &AddressSpace<M>,
        alloc: &mut A,
        va: VirtualAddress,
        nonleaf_flags: UnifiedEntry,
    ) -> Result<PhysicalPage<Size4K>, MapSizeEnsureChainError> {
        let i4 = L4Index::from(va);
        let i3 = L3Index::from(va);

        // L4 → L3
        let pml4 = aspace.pml4_mut();
        let e4 = pml4.get(i4);
        let pdpt_page = if let Some(p) = e4.next_table() {
            p
        } else {
            let f = alloc.alloc_4k().ok_or(MapSizeEnsureChainError::OomPdpt)?;
            aspace.zero_pdpt(f);
            pml4.set(i4, Pml4Entry::make(f, nonleaf_flags.to_pml4e()));
            f
        };

        // L3 → L2 (and split 1GiB if necessary)
        let pdpt = aspace.pdpt_mut(pdpt_page);
        let e3 = pdpt.get(i3);
        Ok(match e3.kind() {
            Some(PdptEntryKind::NextPageDirectory(pd, _)) => pd,
            Some(PdptEntryKind::Leaf1GiB(_, _)) | None => {
                let f = alloc.alloc_4k().ok_or(MapSizeEnsureChainError::OomPd)?;
                aspace.zero_pd(f);
                pdpt.set(i3, PdptEntry::make_next(f, nonleaf_flags.to_pdpte()));
                f
            }
        })
    }

    fn set_leaf<M: PhysMapper>(
        aspace: &AddressSpace<M>,
        leaf_tbl_page: PhysicalPage<Size4K>,
        va: VirtualAddress,
        pa: PhysicalAddress,
        leaf_flags: UnifiedEntry,
    ) {
        debug_assert_eq!(pa.offset::<Self>().as_u64(), 0);
        let pd = aspace.pd_mut(leaf_tbl_page);
        let idx = L2Index::from(va);
        let m2 = PhysicalPage::<Self>::from_addr(pa);
        pd.set(idx, PdEntry::make_2m(m2, leaf_flags.to_pde_2m()));
    }
}

impl MapSize for Size4K {
    fn ensure_chain_for<A: FrameAlloc, M: PhysMapper>(
        aspace: &AddressSpace<M>,
        alloc: &mut A,
        va: VirtualAddress,
        nonleaf_flags: UnifiedEntry,
    ) -> Result<PhysicalPage<Size4K>, MapSizeEnsureChainError> {
        let i4 = L4Index::from(va);
        let i3 = L3Index::from(va);
        let i2 = L2Index::from(va);

        // L4 → L3
        let pml4 = aspace.pml4_mut();
        let e4 = pml4.get(i4);
        let pdpt_page = if let Some(p) = e4.next_table() {
            p
        } else {
            let f = alloc.alloc_4k().ok_or(MapSizeEnsureChainError::OomPdpt)?;
            aspace.zero_pdpt(f);
            pml4.set(i4, Pml4Entry::make(f, nonleaf_flags.to_pml4e()));
            f
        };

        // L3 → L2 (and split 1GiB if necessary)
        let pdpt = aspace.pdpt_mut(pdpt_page);
        let e3 = pdpt.get(i3);
        let pd_page = match e3.kind() {
            Some(PdptEntryKind::NextPageDirectory(pd, _)) => pd,
            Some(PdptEntryKind::Leaf1GiB(_, _)) | None => {
                let f = alloc.alloc_4k().ok_or(MapSizeEnsureChainError::OomPd)?;
                aspace.zero_pd(f);
                pdpt.set(i3, PdptEntry::make_next(f, nonleaf_flags.to_pdpte()));
                f
            }
        };

        // L2 → L1 (and split 2MiB if necessary)
        let pd = aspace.pd_mut(pd_page);
        let e2 = pd.get(i2);
        Ok(match e2.kind() {
            Some(PdEntryKind::NextPageTable(pt, _)) => pt,
            Some(PdEntryKind::Leaf2MiB(_, _)) | None => {
                let f = alloc.alloc_4k().ok_or(MapSizeEnsureChainError::OomPt)?;
                aspace.zero_pt(f);
                pd.set(i2, PdEntry::make_next(f, nonleaf_flags.to_pde()));
                f
            }
        })
    }

    fn set_leaf<M: PhysMapper>(
        aspace: &AddressSpace<M>,
        leaf_tbl_page: PhysicalPage<Size4K>,
        va: VirtualAddress,
        pa: PhysicalAddress,
        leaf_flags: UnifiedEntry,
    ) {
        debug_assert_eq!(pa.offset::<Self>().as_u64(), 0);
        let pt = aspace.pt_mut(leaf_tbl_page);
        let idx = L1Index::from(va);
        let k4 = PhysicalPage::<Self>::from_addr(pa);
        pt.set(idx, PtEntry::make_4k(k4, leaf_flags.to_pte_4k()));
    }
}
