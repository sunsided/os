//! # Virtual Memory Support
//!
//! Minimal x86-64 paging helpers for a hobby OS loader/kernel.
//!
//! ## What you get
//! - Tiny [`PhysAddr`]/[`VirtAddr`] newtypes (u64) to avoid mixing address kinds.
//! - A [`PageSize`] enum for 4 KiB / 2 MiB / 1 GiB mappings.
//! - x86-64 page-table [`Flags`] with practical explanations.
//! - A 4 KiB-aligned [`PageTable`] wrapper and index helpers.
//! - A tiny allocator/mapper interface ([`FrameAlloc`], [`PhysMapper`]).
//! - [`ensure_chain`] to allocate missing intermediate tables on the path.
//! - [`map_one`] to create a single mapping of any supported page size.
//!
//! ## x86-64 Virtual Address → Physical Address Walk
//!
//! Each 48-bit virtual address is divided into five fields:
//!
//! ```text
//! | 47‒39 | 38‒30 | 29‒21 | 20‒12 | 11‒0   |
//! |  PML4 |  PDPT |   PD  |   PT  | Offset |
//! ```
//!
//! The CPU uses these fields as **indices** into four levels of page tables,
//! each level containing 512 (2⁹) entries of 8 bytes (64 bits) each.
//!
//! ```text
//!  PML4  →  PDPT  →  PD  →  PT  →  Physical Page
//!   │        │        │        │
//!   │        │        │        └───► PTE   (Page Table Entry)  → maps 4 KiB page
//!   │        │        └────────────► PDE   (Page Directory Entry) → PS=1 → 2 MiB page
//!   │        └─────────────────────► PDPTE (Page Directory Pointer Table Entry) → PS=1 → 1 GiB page
//!   └──────────────────────────────► PML4E (Page Map Level 4 Entry)
//! ```
//!
//! ### Levels and their roles
//!
//! | Level | Table name | Entry name | Description |
//! |:------|:------------|:-----------|:-------------|
//! | 1 | **PML4** (Page Map Level 4) | **PML4E** | Top-level table; each entry points to a PDPT. One PML4 table per address space, referenced by Control Register 3 ([`CR3`](https://wiki.osdev.org/CPU_Registers_x86#CR3)). |
//! | 2 | **PDPT** (Page Directory Pointer Table) | **PDPTE** | Each entry points to a PD. If `PS=1`, it directly maps a 1 GiB page (leaf). |
//! | 3 | **PD** (Page Directory) | **PDE** | Each entry points to a PT. If `PS=1`, it directly maps a 2 MiB page (leaf). |
//! | 4 | **PT** (Page Table) | **PTE** | Each entry maps a 4 KiB physical page (always a leaf). |
//!
//! ### Leaf vs. non-leaf entries
//!
//! - A **leaf entry** directly maps physical memory — it contains the physical base address
//!   and the permission bits ([`PRESENT`](PageTableEntry::present), [`WRITABLE`](PageTableEntry::writable), [`USER`](PageTableEntry::user), [`GLOBAL`](PageTableEntry::global), [`NX`](PageTableEntry::nx), etc.).
//!   - A **PTE** is always a leaf (maps 4 KiB).
//!   - A **PDE** with `PS=1` is a leaf (maps 2 MiB).
//!   - A **PDPTE** with `PS=1` is a leaf (maps 1 GiB).
//!
//! - A **non-leaf entry** points to the next lower table level and continues the walk.
//!   For example, a PML4E points to a PDPT, and a PDE with `PS=0` points to a PT.
//!
//! ### Offset
//!
//! - The final **Offset** field (bits 11–0) selects the byte inside the 4 KiB (or larger) page.
//!
//! ### Summary
//!
//! A canonical 48-bit virtual address is effectively:
//!
//! ```text
//! VA = [PML4:9] [PDPT:9] [PD:9] [PT:9] [Offset:12]
//! ```
//!
//! This creates a four-level translation tree that can map up to **256 TiB** of
//! virtual address space, using leaf pages of 1 GiB, 2 MiB, or 4 KiB depending
//! on which level the translation stops.

#![cfg_attr(not(test), no_std)]
#![allow(unsafe_code)]

mod page_table;

extern crate alloc;

pub use crate::page_table::{PageTable, PageTableEntry};

/// A memory address as it is used in pointers.
///
/// See [`PhysAddr`] and [`VirtAddr`] for usages.
pub type MemoryAddress = u64;

/// A **physical** memory address (machine bus address).
///
/// Newtype over `u64` to prevent mixing with virtual addresses.
/// No alignment guarantees by itself.
///
/// ### Notes
/// - When used inside page-table entries, the low N bits must be zeroed
///   (N ∈ {12, 21, 30} for 4 KiB/2 MiB/1 GiB).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct PhysAddr(pub MemoryAddress);

/// A **virtual** memory address (process/kernel address space).
///
/// Newtype over `u64` to prevent mixing with physical addresses.
/// No alignment guarantees by itself.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct VirtAddr(pub MemoryAddress);

/// Supported x86-64 page sizes.
///
/// These correspond to the PS (Page Size) bit usage in PDE/PDPTE.
/// 4 KiB pages are mapped through the PT level; 2 MiB and 1 GiB are
/// "huge pages" that terminate early at PD or PDPT.
#[derive(Copy, Clone, Debug)]
pub enum PageSize {
    /// 4 KiB page mapped by a PTE (PT leaf).
    Size4K,
    /// 2 MiB page mapped by a PDE with `PS=1` (PD leaf).
    Size2M,
    /// 1 GiB page mapped by a PDPTE with `PS=1` (PDPT leaf).
    Size1G,
}

bitflags::bitflags! {
    /// Page table entry flags used in x86_64 virtual memory.
    ///
    /// These flags control access permissions, caching behavior,
    /// and indicate page status (e.g., accessed or dirty).
    /// They apply to all paging levels (PTE, PDE, PDPTE, PML4E),
    /// except where noted (e.g., `PS` only valid for PDE/PDPTE).
    #[derive(Copy, Clone)]
    pub struct Flags: u64 {
        /// Page is present in physical memory.
        ///
        /// Must be set for valid mappings; cleared indicates a page fault
        /// on access (used for demand paging or swapping).
        const PRESENT  = 1 << 0;

        /// Page is writable.
        ///
        /// If cleared, the page is read-only; writes trigger a fault
        /// unless running in ring 0 with write protection disabled (CR0.WP = 0).
        const WRITABLE = 1 << 1;

        /// Page is accessible from user mode (CPL=3).
        ///
        /// If cleared, only supervisor mode (CPL ≤ 2) can access the page.
        const USER     = 1 << 2;

        /// Write-through caching enabled.
        ///
        /// Writes are immediately propagated to main memory; typically used
        /// for memory-mapped I/O regions.
        const WT       = 1 << 3;

        /// Caching disabled for this page.
        ///
        /// When set, the CPU bypasses its caches; used for MMIO or strongly
        /// ordered regions.
        const CD       = 1 << 4;

        /// Page has been accessed (read or written).
        ///
        /// The processor sets this bit automatically on access.
        /// Can be cleared by software for tracking or page aging.
        const ACCESSED = 1 << 5;

        /// Page has been written to.
        ///
        /// The processor sets this bit on the first write to the page.
        /// Useful for implementing dirty-page tracking and write-back strategies.
        const DIRTY    = 1 << 6;

        /// Page size flag.
        ///
        /// Indicates this entry maps a large page (2 MiB in a PDE or 1 GiB in a PDPTE)
        /// instead of pointing to a lower-level page table.
        const PS       = 1 << 7;

        /// Global page.
        ///
        /// Prevents the TLB entry from being flushed on CR3 reload,
        /// if CR4.PGE is enabled. Typically used for kernel-space mappings.
        const GLOBAL   = 1 << 8;

        /// No-execute (NX) flag.
        ///
        /// Marks the page as non-executable when EFER.NXE is set.
        /// Execution from such a page triggers a page fault.
        const NX       = 1 << 63;
    }
}

/// Minimal frame allocator used to obtain **physical** 4 KiB frames
/// for page tables.
///
/// The implementation decides where frames come from (bootloader pool,
/// bitmap, etc.). Returned frames **must** be 4 KiB aligned.
///
/// Returns `None` on out-of-memory.
pub trait FrameAlloc {
    /// Allocate one 4 KiB *physical* frame for page tables. Must return page-aligned frames.
    fn alloc_4k(&mut self) -> Option<PhysAddr>;
}

/// Converts physical addresses to *temporarily* usable pointers in the current
/// virtual address space (e.g., via identity map or a higher-half direct map, HHDM).
///
/// Typical patterns:
/// - **Loader**: often identity-maps low memory; returns direct pointers.
/// - **Kernel**: uses HHDM; adds a constant offset before returning a pointer.
///
/// # Safety
/// - You must ensure `pa` is mapped as writable in the current page tables
///   for `&mut T`.
/// - Lifetime `'a` is purely borrow-checked; the mapping must remain valid
///   for `'a`.
/// - Type `T` must match the bytes at `pa` (no aliasing UB).
pub trait PhysMapper {
    /// Convert a *physical* address to a usable mutable pointer in the current address space.
    /// Loader: often identity or HHDM. Kernel: via HHDM.
    unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T;
}

impl VirtAddr {
    /// Extract the PML4 index (bits 47-39 of the virtual address).
    #[inline]
    const fn pml4_index(self) -> usize {
        ((self.0 >> 39) & 0x1ff) as usize
    }

    /// Extract the PDPT index (bits 38-30 of the virtual address).
    #[inline]
    const fn pdpt_index(self) -> usize {
        ((self.0 >> 30) & 0x1ff) as usize
    }

    /// Extract the PD index (bits 29-21 of the virtual address).
    #[inline]
    const fn pd_index(self) -> usize {
        ((self.0 >> 21) & 0x1ff) as usize
    }

    /// Extract the PT index (bits 20-12 of the virtual address).
    #[inline]
    const fn pt_index(self) -> usize {
        ((self.0 >> 12) & 0x1ff) as usize
    }
}

/// A PML4 page table.
#[repr(transparent)]
pub struct Pml4(PageTable);

/// A PDPT page table.
#[repr(transparent)]
pub struct Pdpt(PageTable);

/// A PD page table.
#[repr(transparent)]
pub struct Pd(PageTable);

/// A PT (leaf) page table.
#[repr(transparent)]
pub struct Pt(PageTable);

/// Implement common functionality for page tables.
macro_rules! table_common {
    ($T:ty) => {
        impl $T {
            #[inline]
            pub fn zero(&mut self) {
                self.0.zero()
            }

            #[inline]
            pub const fn as_page_table_mut(&mut self) -> &mut PageTable {
                &mut self.0
            }

            #[inline]
            unsafe fn entry_mut(&mut self, idx: usize) -> &mut PageTableEntry {
                debug_assert!(idx < 512);
                unsafe { &mut *self.0.as_ptr().add(idx) }
            }
        }

        impl AsMut<PageTable> for $T {
            fn as_mut(&mut self) -> &mut PageTable {
                &mut self.0
            }
        }

        impl From<PageTable> for $T {
            fn from(pt: PageTable) -> Self {
                Self(pt)
            }
        }
    };
}

table_common!(Pml4);
table_common!(Pdpt);
table_common!(Pd);
table_common!(Pt);

/// Map a physical frame as a [`Pml4`] typed table.
#[inline]
pub fn as_pml4<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut Pml4 {
    unsafe { &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa)).cast::<Pml4>() }
}

/// Map a physical frame as a [`Pdpt`] typed table.
#[inline]
pub fn as_pdpt<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut Pdpt {
    unsafe { &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa)).cast::<Pdpt>() }
}

/// Map a physical frame as a [`Pd`] typed table.
#[inline]
pub fn as_pd<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut Pd {
    unsafe { &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa)).cast::<Pd>() }
}

/// Map a physical frame as a [`Pt`] typed table.
#[inline]
pub fn as_pt<'t, M: PhysMapper>(m: &M, pa: PhysAddr) -> &'t mut Pt {
    unsafe { &mut *core::ptr::from_mut::<PageTable>(m.phys_to_mut::<PageTable>(pa)).cast::<Pt>() }
}

impl Pml4 {
    #[inline]
    pub fn entry_mut_by_va(&mut self, va: VirtAddr) -> &mut PageTableEntry {
        unsafe { self.entry_mut(va.pml4_index()) }
    }

    /// Initialize a non-leaf entry to point to a PDPT table.
    pub fn link_pdpt(&mut self, va: VirtAddr, pdpt_phys: PhysAddr) {
        let e = self.entry_mut_by_va(va);
        e.set_present(true);
        e.set_writable(true);
        e.set_ps(false);
        e.set_addr(pdpt_phys.0);
    }
}

impl Pdpt {
    #[inline]
    pub fn entry_mut_by_va(&mut self, va: VirtAddr) -> &mut PageTableEntry {
        unsafe { self.entry_mut(va.pdpt_index()) }
    }

    /// Non-leaf: link to a PD table.
    pub fn link_pd(&mut self, va: VirtAddr, pd_phys: PhysAddr) {
        let e = self.entry_mut_by_va(va);
        e.set_present(true);
        e.set_writable(true);
        e.set_ps(false);
        e.set_addr(pd_phys.0);
    }

    /// **Leaf (1 GiB):** set PDPTE as 1 GiB mapping.
    pub fn map_1g_leaf(&mut self, va: VirtAddr, pa: PhysAddr, flags: Flags) {
        let e = self.entry_mut_by_va(va);
        e.set_addr(pa.0);
        e.set_present(true);
        e.set_ps(true);
        e.set_writable(flags.contains(Flags::WRITABLE));
        e.set_user(flags.contains(Flags::USER));
        e.set_write_through(flags.contains(Flags::WT));
        e.set_cache_disable(flags.contains(Flags::CD));
        e.set_global(flags.contains(Flags::GLOBAL));
        e.set_nx(flags.contains(Flags::NX));
    }
}

impl Pd {
    #[inline]
    pub fn entry_mut_by_va(&mut self, va: VirtAddr) -> &mut PageTableEntry {
        unsafe { self.entry_mut(va.pd_index()) }
    }

    /// Non-leaf: link to a PT table.
    pub fn link_pt(&mut self, va: VirtAddr, pt_phys: PhysAddr) {
        let e = self.entry_mut_by_va(va);
        e.set_present(true);
        e.set_writable(true);
        e.set_ps(false);
        e.set_addr(pt_phys.0);
    }

    /// **Leaf (2 MiB):** set PDE as 2 MiB mapping.
    pub fn map_2m_leaf(&mut self, va: VirtAddr, pa: PhysAddr, flags: Flags) {
        let e = self.entry_mut_by_va(va);
        e.set_addr(pa.0);
        e.set_present(true);
        e.set_ps(true);
        e.set_writable(flags.contains(Flags::WRITABLE));
        e.set_user(flags.contains(Flags::USER));
        e.set_write_through(flags.contains(Flags::WT));
        e.set_cache_disable(flags.contains(Flags::CD));
        e.set_global(flags.contains(Flags::GLOBAL));
        e.set_nx(flags.contains(Flags::NX));
    }
}

impl Pt {
    #[inline]
    pub fn entry_mut_by_va(&mut self, va: VirtAddr) -> &mut PageTableEntry {
        unsafe { self.entry_mut(va.pt_index()) }
    }

    /// **Leaf (4 KiB):** set PTE as 4 KiB mapping (no PS).
    pub fn map_4k_leaf(&mut self, va: VirtAddr, pa: PhysAddr, flags: Flags) {
        let e = self.entry_mut_by_va(va);
        e.set_addr(pa.0);
        e.set_present(true);
        e.set_ps(false);
        e.set_writable(flags.contains(Flags::WRITABLE));
        e.set_user(flags.contains(Flags::USER));
        e.set_write_through(flags.contains(Flags::WT));
        e.set_cache_disable(flags.contains(Flags::CD));
        e.set_global(flags.contains(Flags::GLOBAL));
        e.set_nx(flags.contains(Flags::NX));
    }
}

/// Handles to key paging structures owned by the caller.
///
/// The minimal thing we track here is the physical address of the **active PML4**.
/// You can extend this struct to hold scratch tables, HHDM base, etc.
#[derive(Copy, Clone)]
pub struct Tables {
    /// Physical address of the active PML4.
    pub pml4_phys: PhysAddr,
}

/// Map a physical page table frame into the current virtual address space and
/// return a mutable reference to it.
///
/// # Safety
/// - `phys` must point to a valid 4 KiB page containing a page table.
/// - The mapping must be writable for mut access.
#[inline]
unsafe fn get_table<'a, M: PhysMapper>(m: &M, phys: PhysAddr) -> &'a mut PageTable {
    unsafe { m.phys_to_mut::<PageTable>(phys) }
}

/// Internal helper: get a mutable entry reference by index.
///
/// `PageTable` exposes only `as_ptr()`; we provide a small, contained unsafe.
#[inline]
unsafe fn entry_mut(tbl: &mut PageTable, idx: usize) -> &mut PageTableEntry {
    debug_assert!(idx < 512);
    unsafe { &mut *tbl.as_ptr().add(idx) }
}

/// Apply `Flags` to a (leaf or non-leaf) entry. For non-leaf tables you typically
/// keep USER/WT/CD/GLOBAL/NX = false, but we only set bits explicitly present in `flags`.
#[inline]
const fn apply_flags(e: &mut PageTableEntry, flags: Flags, is_leaf_huge: bool) {
    if flags.contains(Flags::PRESENT) {
        e.set_present(true);
    }
    if flags.contains(Flags::WRITABLE) {
        e.set_writable(true);
    }
    if flags.contains(Flags::USER) {
        e.set_user(true);
    }
    if flags.contains(Flags::WT) {
        e.set_write_through(true);
    }
    if flags.contains(Flags::CD) {
        e.set_cache_disable(true);
    }
    if flags.contains(Flags::ACCESSED) {
        e.set_accessed(true);
    }
    if flags.contains(Flags::DIRTY) {
        e.set_dirty(true);
    }
    if flags.contains(Flags::GLOBAL) {
        e.set_global(true);
    }
    if flags.contains(Flags::NX) {
        e.set_nx(true);
    }
    // PS only makes sense for leaves at PDPT/PD:
    if flags.contains(Flags::PS) || is_leaf_huge {
        e.set_ps(true);
    }
}

/// Ensure the page-table chain exists down to the leaf level for `va`,
/// allocating intermediate tables as needed.
///
/// Returns `(leaf_phys, is_leaf_huge)` where:
/// - for **1 GiB**: `leaf_phys = PDPT frame`, `is_leaf_huge = true`
/// - for **2 MiB**: `leaf_phys = PD frame`,  `is_leaf_huge = true`
/// - for **4 KiB**: `leaf_phys = PT frame`,   `is_leaf_huge = false`
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
pub fn ensure_chain<A: FrameAlloc, M: PhysMapper>(
    alloc: &mut A,
    map: &M,
    root: PhysAddr,
    va: VirtAddr,
    size: PageSize,
) -> Result<(PhysAddr, bool), &'static str> {
    // PML4
    let pml4 = as_pml4(map, root);
    let e4 = pml4.entry_mut_by_va(va);
    let pdpt_phys = if e4.present() {
        PhysAddr(e4.addr())
    } else {
        let f = alloc.alloc_4k().ok_or("OOM for PDPT")?;
        as_pdpt(map, f).zero();
        pml4.link_pdpt(va, f);
        f
    };

    // PDPT
    let pdpt = as_pdpt(map, pdpt_phys);
    if matches!(size, PageSize::Size1G) {
        // Caller will fill PDPTE (set PS + final flags).
        return Ok((pdpt_phys, true));
    }
    let e3 = pdpt.entry_mut_by_va(va);
    let pd_phys = if !e3.present() || e3.ps() {
        // no entry yet OR conflicting 1 GiB leaf → allocate PD
        let f = alloc.alloc_4k().ok_or("OOM for PD")?;
        as_pd(map, f).zero();
        pdpt.link_pd(va, f);
        f
    } else {
        PhysAddr(e3.addr())
    };

    // PD
    let pd = as_pd(map, pd_phys);
    if matches!(size, PageSize::Size2M) {
        // Caller will fill PDE (set PS + final flags).
        return Ok((pd_phys, true));
    }
    let e2 = pd.entry_mut_by_va(va);
    let pt_phys = if !e2.present() || e2.ps() {
        // no entry yet OR conflicting 2 MiB leaf → allocate PT
        let f = alloc.alloc_4k().ok_or("OOM for PT")?;
        as_pt(map, f).zero();
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
pub fn map_one<A: FrameAlloc, M: PhysMapper>(
    alloc: &mut A,
    map: &M,
    root: PhysAddr,
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
        let (leaf_phys, is_huge_leaf) = ensure_chain(alloc, map, root, va, size)?;
        match size {
            PageSize::Size1G => {
                // PDPTE leaf: phys bits 51:30, low 30 bits zero.
                let pdpt = get_table::<M>(map, leaf_phys);
                let e = entry_mut(pdpt, va.pdpt_index());
                e.set_addr(pa.0);
                apply_flags(e, flags | Flags::PS, true);
            }
            PageSize::Size2M => {
                // PDE leaf: phys bits 51:21, low 21 bits zero.
                let pd = get_table::<M>(map, leaf_phys);
                let e = entry_mut(pd, va.pd_index());
                e.set_addr(pa.0);
                apply_flags(e, flags | Flags::PS, true);
            }
            PageSize::Size4K => {
                // PTE leaf: phys bits 51:12, low 12 bits zero.
                let pt = get_table::<M>(map, leaf_phys);
                let e = entry_mut(pt, va.pt_index());
                e.set_addr(pa.0);
                apply_flags(e, flags, is_huge_leaf);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// A trivial **bump** allocator: always hands out the next 4 KiB frame.
    ///
    /// Why "bump"? Because it only keeps a cursor (`next`) and "bumps" it by 4096 on each alloc.
    /// There's no free list, no reuse, no fragmentation handling (perfect for tests/boot stage).
    struct BumpAlloc {
        /// Next free physical byte address (must remain 4 KiB aligned)
        next: u64,
        /// Exclusive end (bounds check)
        end: u64,
    }

    impl BumpAlloc {
        fn new(start: u64, end: u64) -> Self {
            Self { next: start, end }
        }
    }

    impl FrameAlloc for BumpAlloc {
        fn alloc_4k(&mut self) -> Option<PhysAddr> {
            if self.next + 4096 > self.end {
                return None;
            }
            let p = self.next;
            self.next += 4096;
            Some(PhysAddr(p))
        }
    }

    /// A 4 KiB-aligned raw frame. We use this as our "physical RAM" backing store in tests.
    #[repr(align(4096))]
    struct Aligned4K(#[allow(dead_code)] [u8; 4096]);

    impl Aligned4K {
        fn new_zeroed() -> Self {
            Self([0u8; 4096])
        }
    }

    /// A tiny in-memory "RAM" plus an HHDM (higher-half direct map) style mapper.
    ///
    /// We simulate physical memory as a vector of 4 KiB-aligned frames. Physical addresses are
    /// simple byte offsets from 0. The mapper turns a physical address into a `&mut T` by:
    ///   1) picking the frame `pa / 4096`,
    ///   2) casting that 4 KiB block to `&mut T` (caller ensures the type matches).
    ///
    /// This is *only* for tests. Real mappers must honor whatever HHDM/identity mapping you set up.
    struct TestPhys {
        frames: Vec<Aligned4K>,
    }

    impl TestPhys {
        fn with_frames(n: usize) -> Self {
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(Aligned4K::new_zeroed());
            }
            Self { frames: v }
        }

        fn frame_mut_ptr(&self, idx: usize) -> *mut u8 {
            // SAFETY: frames are 4 KiB aligned; we return a pointer into the owned buffer.
            &self.frames[idx] as *const Aligned4K as *mut u8
        }
    }

    impl PhysMapper for TestPhys {
        unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T {
            let idx = (pa.0 >> 12) as usize;
            let off = (pa.0 & 0xfff) as usize;
            // For page tables we expect offset==0; assert to catch misuse in the test.
            debug_assert_eq!(off, 0);

            // SAFETY: The caller promises `T` matches the bytes in the frame.
            unsafe { &mut *(self.frame_mut_ptr(idx) as *mut T) }
        }
    }

    unsafe fn entry(tbl: &mut PageTable, idx: usize) -> &mut PageTableEntry {
        unsafe { entry_mut(tbl, idx) }
    }

    #[test]
    fn map_one_4k_creates_tables_and_leaf() {
        // Reserve 64 frames (= 256 KiB) for the test "physical memory".
        let phys = TestPhys::with_frames(64);

        // Our physical address space runs from 0 to 64*4096.
        let start = 0u64;
        let end = (64u64) << 12;
        let mut alloc = BumpAlloc::new(start, end);

        // Allocate and clear the PML4 (root) table.
        let root_pa = alloc.alloc_4k().unwrap();
        unsafe {
            get_table(&phys, root_pa).zero();
        }

        // Pick a virtual+physical pair for mapping (must be 4 KiB aligned).
        let va = VirtAddr(0xffff_8000_0000_0000); // arbitrary higher-half VA
        let pa = PhysAddr(0x0000_0000_0030_0000); // 3 * 2^20 = 3 MiB, aligned to 4 KiB

        map_one(
            &mut alloc,
            &phys,
            root_pa,
            va,
            pa,
            PageSize::Size4K,
            Flags::WRITABLE | Flags::GLOBAL | Flags::NX,
        )
        .unwrap();

        unsafe {
            // Walk the tables again and verify entries were created and look sane.

            // PML4
            let pml4 = get_table(&phys, root_pa);
            let e4 = entry(pml4, va.pml4_index());
            assert!(e4.present());
            let pdpt_pa = PhysAddr(e4.addr());

            // PDPT
            let pdpt = get_table(&phys, pdpt_pa);
            let e3 = entry(pdpt, va.pdpt_index());
            assert!(e3.present());
            assert!(!e3.ps());
            let pd_pa = PhysAddr(e3.addr());

            // PD
            let pd = get_table(&phys, pd_pa);
            let e2 = entry(pd, va.pd_index());
            assert!(e2.present());
            assert!(!e2.ps());
            let pt_pa = PhysAddr(e2.addr());

            // PT (leaf)
            let pt = get_table(&phys, pt_pa);
            let e1 = entry(pt, va.pt_index());
            // Expected leaf encoding: phys|flags (no PS for 4K).
            assert!(e1.present());
            assert_eq!(e1.addr(), pa.0);
            assert!(e1.writable());
            assert!(e1.global());
            assert!(e1.nx());
            assert!(!e1.ps());
        }
    }

    #[test]
    fn map_one_2m_sets_ps_bit() {
        let phys = TestPhys::with_frames(64);
        let mut alloc = BumpAlloc::new(0, (64u64) << 12);

        let root_pa = alloc.alloc_4k().unwrap();
        unsafe {
            get_table(&phys, root_pa).zero();
        }

        let va = VirtAddr(0xffff_8000_2000_0000); // arbitrary VA aligned to 2 MiB
        let pa = PhysAddr(0x0000_0000_0400_0000); // 64 MiB (aligned to 2 MiB)
        map_one(
            &mut alloc,
            &phys,
            root_pa,
            va,
            pa,
            PageSize::Size2M,
            Flags::WRITABLE,
        )
        .unwrap();

        unsafe {
            let pml4 = get_table(&phys, root_pa);
            let pdpt_pa = PhysAddr(entry(pml4, va.pml4_index()).addr());
            let pdpt = get_table(&phys, pdpt_pa);
            let pd_pa = PhysAddr(entry(pdpt, va.pdpt_index()).addr());
            let pd = get_table(&phys, pd_pa);
            let pde = entry(pd, va.pd_index());
            assert!(pde.present());
            assert!(pde.ps());
            assert!(pde.writable());
            assert_eq!(pde.addr(), pa.0);
        }
    }

    #[test]
    fn map_one_1g_sets_ps_bit() {
        let phys = TestPhys::with_frames(64);
        let mut alloc = BumpAlloc::new(0, (64u64) << 12);

        let root_pa = alloc.alloc_4k().unwrap();
        unsafe {
            get_table(&phys, root_pa).zero();
        }

        let va = VirtAddr(0x0000_4000_0000_0000); // arbitrary VA aligned to 1 GiB
        let pa = PhysAddr(0x0000_0000_4000_0000); // 1 GiB (aligned to 1 GiB)
        map_one(
            &mut alloc,
            &phys,
            root_pa,
            va,
            pa,
            PageSize::Size1G,
            Flags::WRITABLE,
        )
        .unwrap();

        unsafe {
            // Walk to PDPT and verify leaf with PS=1.
            let pml4 = get_table(&phys, root_pa);
            let pdpt_pa = PhysAddr(entry(pml4, va.pml4_index()).addr());
            let pdpt = get_table(&phys, pdpt_pa);
            let pdpte = entry(pdpt, va.pdpt_index());
            assert!(pdpte.present());
            assert!(pdpte.ps());
            assert!(pdpte.writable());
            assert_eq!(pdpte.addr(), pa.0);
        }
    }
}
