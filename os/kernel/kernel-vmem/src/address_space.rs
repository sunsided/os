//! # Virtual Address Space
//!
//! Thin, zero-overhead helpers for operating on a **single x86-64 address
//! space** (page-table tree rooted at a PML4).
//!
//! This module complements the paging primitives by providing:
//!
//! - An [`AddressSpace`] handle that knows the **physical address of the PML4**
//!   and a [`PhysMapper`] to temporarily view/modify page tables.
//! - [`AddressSpace::ensure_chain`] to **allocate missing intermediate tables**
//!   on the walk from `PML4 → PDPT → PD → PT` for a given [`VirtAddr`] and
//!   target [`PageSize`].
//! - [`AddressSpace::map_one`] to **install a single mapping** (4 KiB / 2 MiB / 1 GiB).
//! - [`AddressSpace::activate`] to **load CR3** with this address space (loader/kernel).
//! - `as_*` helpers to temporarily treat a **physical frame** as a typed table
//!   ([`Pml4PageTable`], [`PdptPageTable`], [`PdPageTable`], [`PtPageTable`]).
//!
//! ## Design notes
//!
//! - All table mutations happen through a provided [`PhysMapper`]. This keeps the
//!   paging code agnostic of whether you use identity maps (loader) or a higher-half
//!   direct map (HHDM) in the kernel.
//! - Allocation is delegated to a minimal [`FrameAlloc`] which must return **4 KiB-aligned**
//!   physical frames suitable for page tables.
//! - We keep the API small on purpose; things like **splitting/merging huge pages**,
//!   **unmapping**, or **permission changes** can be layered on top using the same
//!   primitives.
//!
//! ## Typical usage
//!
//! ```ignore
//! // Allocate a fresh PML4 and zero it
//! let pml4_phys = alloc.alloc_4k().unwrap();
//! unsafe { get_table(&mapper, pml4_phys).zero(); }
//!
//! // Bind an AddressSpace to it
//! let aspace = AddressSpace::new(&mapper, pml4_phys);
//!
//! // Map a higher-half kernel page (4 KiB) as RW, global, NX
//! aspace.map_one(
//!     &mut alloc,
//!     VirtAddr(0xffff_8000_0000_0000),
//!     PhysAddr(0x0010_0000),
//!     PageSize::Size4K,
//!     Flags::WRITABLE | Flags::GLOBAL | Flags::NX,
//! )?;
//!
//! // Make it active (requires having set CR0.WP/CR4.PGE/EFER.NXE as needed)
//! unsafe { aspace.activate(); }
//! ```
//!
//! ## Invariants & safety summary
//!
//! - **Tables are writable**: the caller’s [`PhysMapper`] must return **writable** references
//!   for the page-table frames touched (PML4/PDPT/PD/PT).
//! - **Root is active** when mutating live mappings: if you modify the active address
//!   space, remember to **flush TLBs** (e.g., `invlpg` per page or CR3 reload).
//! - **Alignment**: physical addresses used in leaves must be aligned to the chosen
//!   page size; this is enforced via debug assertions.
//!
//! See also the companion docs in the paging module for a recap of the x86-64 walk.

#![allow(dead_code)]

use crate::{
    Flags, FrameAlloc, PageSize, PageTable, PdPageTable, PdptPageTable, PhysAddr, PhysMapper,
    Pml4PageTable, PtPageTable, VirtAddr, apply_flags, get_table,
};

/// A handle to one **concrete address space** (page-table tree).
///
/// It stores:
/// - `root_phys`: the **physical** address of the active PML4.
/// - `mapper`: a [`PhysMapper`] capable of providing temporary, writable access
///   to page-table frames.
///
/// This type does **not** own the memory; it’s a **view** over an existing tree.
pub struct AddressSpace<'m, M: PhysMapper> {
    /// Physical address of the PML4 (root of this address space).
    root_phys: PhysAddr,
    mapper: &'m M,
}

impl<'m, M: PhysMapper> AddressSpace<'m, M> {
    /// Create a new [`AddressSpace`] view for `root_phys` using `mapper`.
    #[inline]
    pub const fn new(mapper: &'m M, root_phys: PhysAddr) -> Self {
        Self { root_phys, mapper }
    }

    /// Borrow the **root PML4** as a typed table.
    ///
    /// This uses the [`PhysMapper`] to reinterpret `root_phys` as a [`Pml4PageTable`].
    #[inline]
    #[must_use]
    pub fn pml4(&self) -> &mut Pml4PageTable {
        as_pml4(self.mapper, self.root_phys)
    }

    /// Ensure the non-leaf chain for `va` exists down to the appropriate level for `size`,
    /// allocating any missing intermediate tables.
    ///
    /// Returns `(leaf_phys, is_leaf_huge)` where:
    /// - for **1 GiB** pages: `leaf_phys = PDPT frame`, `is_leaf_huge = true` (you will set the PDPTE leaf)
    /// - for **2 MiB** pages: `leaf_phys = PD   frame`, `is_leaf_huge = true` (you will set the  PDE  leaf)
    /// - for **4 KiB** pages: `leaf_phys = PT   frame`, `is_leaf_huge = false` (you will set the  PTE  leaf)
    ///
    /// Newly created intermediate entries are initialized as **present + writable**.
    ///
    /// # Errors
    /// - `"OOM for PDPT" / "OOM for PD" / "OOM for PT"` if the allocator cannot
    ///   provide a new 4 KiB frame.
    ///
    /// # Safety & invariants
    /// - `root_phys` must correspond to the intended address space (ideally active).
    /// - The provided [`PhysMapper`] must be able to map each table frame **writable**.
    /// - If a conflicting huge leaf is encountered at an intermediate level,
    ///   it will be **replaced** by a new non-leaf (split) to continue the chain.
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
            // Caller will set PDPTE as a 1 GiB leaf.
            return Ok((pdpt_phys, true));
        }
        let e3 = pdpt.entry_mut_by_va(va);
        let pd_phys = if !e3.present() || e3.ps() {
            // Absent or conflicting 1 GiB leaf → allocate PD and link
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
            // Caller will set PDE as a 2 MiB leaf.
            return Ok((pd_phys, true));
        }
        let e2 = pd.entry_mut_by_va(va);
        let pt_phys = if !e2.present() || e2.ps() {
            // Absent or conflicting 2 MiB leaf → allocate PT and link
            let f = alloc.alloc_4k().ok_or("OOM for PT")?;
            as_pt(self.mapper, f).zero();
            pd.link_pt(va, f);
            f
        } else {
            PhysAddr(e2.addr())
        };

        // PT leaf for 4 KiB mappings
        Ok((pt_phys, false))
    }

    /// Map **one** page at `va → pa` with the given `size` and `flags`.
    ///
    /// - `PRESENT` is added automatically.
    /// - For huge pages, `PS` is set automatically.
    ///
    /// ### Examples
    /// - **User data page (4 KiB)**: `WRITABLE | USER | NX`
    /// - **Kernel code page (2 MiB)**: `GLOBAL` (no `NX`)
    /// - **HHDM mapping (1 GiB)**: `WRITABLE | GLOBAL | NX`
    ///
    /// ### Alignment
    /// - `pa` **must** be aligned to the chosen `size` (debug-asserted).
    /// - `va` should be aligned for sanity (hardware permits offsets, but avoid it).
    ///
    /// # Safety
    /// - If you are **modifying live mappings**, you are responsible for performing
    ///   required **TLB invalidations** (`invlpg` / CR3 reload).
    /// - Splitting an existing huge page is allowed by this routine (via
    ///   `ensure_chain`)—ensure this is acceptable for your use case.
    ///
    /// # Errors
    /// - Propagates allocation failures from [`ensure_chain`](Self::ensure_chain).
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

        // Physical alignment sanity checks (debug builds).
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
                    // PDPTE leaf
                    let pdpt = get_table::<M>(self.mapper, leaf_phys);
                    let e = pdpt.entry_mut(va.pdpt_index());
                    e.set_addr(pa.0);
                    apply_flags(e, flags | Flags::PS, true);
                }
                PageSize::Size2M => {
                    // PDE leaf
                    let pd = get_table::<M>(self.mapper, leaf_phys);
                    let e = pd.entry_mut(va.pd_index());
                    e.set_addr(pa.0);
                    apply_flags(e, flags | Flags::PS, true);
                }
                PageSize::Size4K => {
                    // PTE leaf
                    let pt = get_table::<M>(self.mapper, leaf_phys);
                    let e = pt.entry_mut(va.pt_index());
                    e.set_addr(pa.0);
                    apply_flags(e, flags, is_huge_leaf);
                }
            }
        }
        Ok(())
    }

    /// Load **CR3** with this address space’s `root_phys`.
    ///
    /// This is a low-level operation and assumes you have configured paging-related
    /// CPU state appropriately (e.g., `CR0.WP`, `CR4.PGE`, `EFER.NXE`) to match the
    /// flags you intend to use (e.g., `GLOBAL`, `NX`).
    ///
    /// # Safety
    /// - Switching CR3 changes the active address space. Ensure that:
    ///   - The code executing after this call is mapped/executable in the target tree.
    ///   - Interrupt and exception handlers are mapped accordingly.
    ///   - Any per-CPU data/TSS locations are valid in the new space.
    #[inline]
    pub unsafe fn activate(&self) {
        // Enable/disable PGE/NXE elsewhere as needed
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) self.root_phys.0, options(nostack, preserves_flags));
        }
    }
}

/// Treat a **physical frame** as a [`Pml4PageTable`].
///
/// # Safety
/// - The frame at `pa` must contain a valid PML4 table (or be zeroed before first use).
#[inline]
pub fn as_pml4<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut Pml4PageTable {
    unsafe {
        &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa))
            .cast::<Pml4PageTable>()
    }
}

/// Treat a **physical frame** as a [`PdptPageTable`] (level 3).
///
/// # Safety
/// - The frame at `pa` must contain a valid PDPT (or be zeroed before first use).
#[inline]
pub fn as_pdpt<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut PdptPageTable {
    unsafe {
        &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa))
            .cast::<PdptPageTable>()
    }
}

/// Treat a **physical frame** as a [`PdPageTable`] (level 2).
///
/// # Safety
/// - The frame at `pa` must contain a valid PD (or be zeroed before first use).
#[inline]
pub fn as_pd<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut PdPageTable {
    unsafe {
        &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa)).cast::<PdPageTable>()
    }
}

/// Treat a **physical frame** as a [`PtPageTable`] (level 1).
///
/// # Safety
/// - The frame at `pa` must contain a valid PT (or be zeroed before first use).
#[inline]
pub fn as_pt<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut PtPageTable {
    unsafe {
        &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa)).cast::<PtPageTable>()
    }
}
