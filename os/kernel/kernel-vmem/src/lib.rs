//! # Virtual Memory Support
//!
//! Minimal x86-64 paging helpers for a hobby OS loader/kernel.
//!
//! ## What you get
//! - An [`address space`](address_space) describing a `PML4` root page table.
//! - Tiny [`PhysAddr`]/[`VirtAddr`] newtypes (u64) to avoid mixing address kinds.
//! - A [`PageSize`] enum for 4 KiB / 2 MiB / 1 GiB mappings.
//! - x86-64 page-table [`Flags`] with practical explanations.
//! - A 4 KiB-aligned [`PageTable`] wrapper and index helpers.
//! - A tiny allocator/mapper interface ([`FrameAlloc`], [`PhysMapper`]).
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
#![allow(unsafe_code, clippy::inline_always)]

pub mod address_space;
mod addresses;
mod page_table;

extern crate alloc;

pub use crate::address_space::AddressSpace;
use crate::addresses::{PhysAddr, VirtAddr};
pub use crate::page_table::{PageTable, PageTableEntry};

/// Re-export constants as info module.
pub use kernel_info::memory as info;

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
    ///
    /// # Safety
    /// Needs evaluation
    unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T;
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

/// Align `x` down to the nearest multiple of `a`.
///
/// This returns the greatest value `y <= x` such that `y % a == 0`.
///
/// ### Preconditions
/// - `a` must be **non-zero** and a **power of two** (e.g., 1, 2, 4, 8, …).
///   These bit-trick formulas rely on that property.
/// - No additional constraints on `x`.
///
/// ### Notes
/// - If `x` is already aligned to `a`, it is returned unchanged.
/// - For non power-of-two `a`, the result is meaningless.
/// - This function does not perform runtime checks for performance reasons.
///
/// ### Examples
/// ```rust
/// # use kernel_vmem::align_down;
/// assert_eq!(align_down(0,      4096), 0);
/// assert_eq!(align_down(1,      4096), 0);
/// assert_eq!(align_down(4095,   4096), 0);
/// assert_eq!(align_down(4096,   4096), 4096);
/// assert_eq!(align_down(8191,   4096), 4096);
/// assert_eq!(align_down(0x12345,   16), 0x12340);
/// ```
#[inline(always)]
#[must_use]
pub const fn align_down(x: u64, a: u64) -> u64 {
    x & !(a - 1)
}

/// Align `x` up to the nearest multiple of `a`.
///
/// This returns the smallest value `y >= x` such that `y % a == 0`.
///
/// ### Preconditions
/// - `a` must be **non-zero** and a **power of two**.
/// - `x + (a - 1)` must **not overflow** `u64`.
///   In debug builds, overflow panics; in release, it wraps (yielding a wrong result).
///   If you need saturating behavior, handle that before calling.
///
/// ### Notes
/// - If `x` is already aligned to `a`, it is returned unchanged.
/// - This function does not perform runtime checks for performance reasons.
///
/// ### Examples
/// ```rust
/// # use kernel_vmem::align_up;
/// assert_eq!(align_up(0,       4096), 0);
/// assert_eq!(align_up(1,       4096), 4096);
/// assert_eq!(align_up(4095,    4096), 4096);
/// assert_eq!(align_up(4096,    4096), 4096);
/// assert_eq!(align_up(4097,    4096), 8192);
/// assert_eq!(align_up(0x12345,   16), 0x12350);
/// ```
#[inline(always)]
#[must_use]
pub const fn align_up(x: u64, a: u64) -> u64 {
    (x + a - 1) & !(a - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address_space::AddressSpace;
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
        let aspace = AddressSpace::new(&phys, root_pa);

        // Pick a virtual+physical pair for mapping (must be 4 KiB aligned).
        let va = VirtAddr(0xffff_8000_0000_0000); // arbitrary higher-half VA
        let pa = PhysAddr(0x0000_0000_0030_0000); // 3 * 2^20 = 3 MiB, aligned to 4 KiB

        aspace
            .map_one(
                &mut alloc,
                va,
                pa,
                PageSize::Size4K,
                Flags::WRITABLE | Flags::GLOBAL | Flags::NX,
            )
            .expect("map_one");

        unsafe {
            // Walk the tables again and verify entries were created and look sane.

            // PML4
            let pml4 = get_table(&phys, root_pa);
            let e4 = pml4.entry(va.pml4_index());
            assert!(e4.present());
            let pdpt_pa = PhysAddr(e4.addr());

            // PDPT
            let pdpt = get_table(&phys, pdpt_pa);
            let e3 = pdpt.entry(va.pdpt_index());
            assert!(e3.present());
            assert!(!e3.ps());
            let pd_pa = PhysAddr(e3.addr());

            // PD
            let pd = get_table(&phys, pd_pa);
            let e2 = pd.entry(va.pd_index());
            assert!(e2.present());
            assert!(!e2.ps());
            let pt_pa = PhysAddr(e2.addr());

            // PT (leaf)
            let pt = get_table(&phys, pt_pa);
            let e1 = pt.entry(va.pt_index());
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
        let aspace = AddressSpace::new(&phys, root_pa);

        let va = VirtAddr(0xffff_8000_2000_0000); // arbitrary VA aligned to 2 MiB
        let pa = PhysAddr(0x0000_0000_0400_0000); // 64 MiB (aligned to 2 MiB)
        aspace
            .map_one(&mut alloc, va, pa, PageSize::Size2M, Flags::WRITABLE)
            .expect("map_one");

        unsafe {
            let pml4 = get_table(&phys, root_pa);
            let pdpt_pa = PhysAddr(pml4.entry(va.pml4_index()).addr());
            let pdpt = get_table(&phys, pdpt_pa);
            let pd_pa = PhysAddr(pdpt.entry(va.pdpt_index()).addr());
            let pd = get_table(&phys, pd_pa);
            let pde = pd.entry(va.pd_index());
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
        let aspace = AddressSpace::new(&phys, root_pa);

        let va = VirtAddr(0x0000_4000_0000_0000); // arbitrary VA aligned to 1 GiB
        let pa = PhysAddr(0x0000_0000_4000_0000); // 1 GiB (aligned to 1 GiB)
        aspace
            .map_one(&mut alloc, va, pa, PageSize::Size1G, Flags::WRITABLE)
            .expect("map_one");

        unsafe {
            // Walk to PDPT and verify leaf with PS=1.
            let pml4 = get_table(&phys, root_pa);
            let pdpt_pa = PhysAddr(pml4.entry(va.pml4_index()).addr());
            let pdpt = get_table(&phys, pdpt_pa);
            let pdpte = pdpt.entry(va.pdpt_index());
            assert!(pdpte.present());
            assert!(pdpte.ps());
            assert!(pdpte.writable());
            assert_eq!(pdpte.addr(), pa.0);
        }
    }
}
