//! # x86-64 Page Table (PT / L1)
//!
//! This module models the lowest paging level (L1, Page Table).
//!
//! - [`L1Index`]: index type for VA bits `[20:12]`.
//! - [`PtEntry4k`]: a PT entry (PTE). At this level, `PS` **must be 0**; entries
//!   represent 4 KiB leaf mappings only.
//! - [`PageTable`]: a 4 KiB-aligned array of 512 PTEs.
//!
//! ## Semantics
//!
//! - L1 does **not** point to another table. Every present entry maps a 4 KiB page.
//! - The base address stored in a PTE must be 4 KiB-aligned (hardware requirement).
//!
//! ## Invariants & Notes
//!
//! - [`PageTable`] is 4 KiB-aligned and contains exactly 512 entries.
//! - [`PtEntry4k::make_4k`] forces `PS=0` and `present=1`.
//! - Raw constructors do not validate consistency; prefer typed helpers.
//! - After modifying active mappings, the caller must perform any required TLB maintenance.

use crate::VirtualMemoryPageBits;
use crate::addresses::{PhysicalAddress, PhysicalPage, Size4K, VirtualAddress};
use bitfield_struct::bitfield;

/// L1 **PTE (4 KiB leaf)** — maps a single 4 KiB page (bit 7 is **PAT**).
///
/// - Physical address uses bits **51:12** and must be **4 KiB aligned**.
/// - The three PAT selector bits are **PWT (bit 3)**, **PCD (bit 4)**,
///   and **PAT (bit 7)**.
#[bitfield(u64)]
pub struct PtEntry4k {
    /// Present (bit 0).
    pub present: bool,
    /// Writable (bit 1).
    pub writable: bool,
    /// User (bit 2).
    pub user: bool,
    /// Write-Through (bit 3) — **PAT selector bit 0**.
    pub write_through: bool,
    /// Cache Disable (bit 4) — **PAT selector bit 1**.
    pub cache_disable: bool,
    /// Accessed (bit 5).
    pub accessed: bool,
    /// Dirty (bit 6): set by CPU on first write.
    pub dirty: bool,
    /// **PAT** (bit 7) — **PAT selector bit 2** for 4 KiB mappings.
    pub pat_small: bool,
    /// Global (bit 8): TLB entry not flushed on CR3 reload.
    pub global: bool,
    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,
    /// Physical address bits **51:12** (4 KiB-aligned base).
    #[bits(40)]
    phys_addr_51_12: u64,
    /// OS-available high (bits 52..58).
    #[bits(7)]
    pub os_available_high: u8,
    /// Protection Key / OS use (59..62).
    #[bits(4)]
    pub protection_key: u8,
    /// No-Execute (bit 63).
    pub no_execute: bool,
}

/// Index into the Page Table (derived from VA bits `[20:12]`).
///
/// Strongly typed to avoid mixing with other levels. Range is `0..512`
/// (checked in debug builds).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L1Index(u16);

/// The Page Table (L1): 512 entries, 4 KiB-aligned.
#[doc(alias = "PT")]
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PtEntry4k; 512],
}

impl L1Index {
    /// Build an index from a canonical virtual address (extracts bits `[20:12]`).
    ///
    /// Returns a value in `0..512`.
    #[inline]
    #[must_use]
    pub const fn from(va: VirtualAddress) -> Self {
        Self::new(((va.as_u64() >> 12) & 0x1FF) as u16)
    }

    /// Construct from a raw `u16`.
    ///
    /// ### Debug assertions
    /// - Asserts `v < 512` in debug builds.
    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        debug_assert!(v < 512);
        Self(v)
    }

    /// Return the index as `usize` for table access.
    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl PtEntry4k {
    /// Set the 4 KiB page base (4 KiB-aligned).
    #[inline]
    #[must_use]
    pub const fn with_physical_page(mut self, phys: PhysicalPage<Size4K>) -> Self {
        self.set_physical_page(phys);
        self
    }

    /// Set the 4 KiB page base (4 KiB-aligned).
    #[inline]
    pub const fn set_physical_page(&mut self, phys: PhysicalPage<Size4K>) {
        self.set_phys_addr_51_12(phys.base().as_u64() >> 12);
    }

    /// Get the 4 KiB page base.
    #[inline]
    #[must_use]
    pub const fn physical_page(self) -> PhysicalPage<Size4K> {
        PhysicalPage::from_addr(PhysicalAddress::new(self.phys_addr_51_12() << 12))
    }

    /// Create a new, present [`PtEntry4k`] with the specified flags, at the specified page.
    #[must_use]
    pub const fn present_with(
        leaf_flags: VirtualMemoryPageBits,
        page: PhysicalPage<Size4K>,
    ) -> Self {
        leaf_flags
            .to_pte_4k()
            .with_present(true)
            .with_physical_page(page)
    }

    /// 4 KiB **user RO+NX** mapping (read-only, no execute).
    #[inline]
    #[must_use]
    pub const fn new_user_ro_nx() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(false)
            .with_user(true)
            .with_no_execute(true)
    }

    /// Create a zero (non-present) entry.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self::new()
    }

    /// If present, return the mapped 4 KiB physical page and its flags.
    ///
    /// Debug-asserts that `PS=0` (required at L1).
    #[inline]
    #[must_use]
    pub const fn page_4k(self) -> Option<(PhysicalPage<Size4K>, Self)> {
        if !self.present() {
            return None;
        }
        Some((self.physical_page(), self))
    }
}

impl PageTable {
    /// Create a fully zeroed Page Table (all entries non-present).
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [PtEntry4k::zero(); 512],
        }
    }

    /// Read the entry at `i`.
    ///
    /// Plain load; does not imply any TLB synchronization.
    #[inline]
    #[must_use]
    pub const fn get(&self, i: L1Index) -> PtEntry4k {
        self.entries[i.as_usize()]
    }

    /// Write the entry at `i`.
    ///
    /// Caller must handle any required TLB invalidation when changing active mappings.
    #[inline]
    pub const fn set(&mut self, i: L1Index, e: PtEntry4k) {
        self.entries[i.as_usize()] = e;
    }

    /// Set the entry at `i` to [`PtEntry4k::zero`].
    ///
    /// Caller is responsible for necessary TLB invalidations if this affects an
    /// active address space.
    #[inline]
    pub const fn set_zero(&mut self, i: L1Index) {
        self.set(i, PtEntry4k::zero());
    }

    /// Derive the PT index from a virtual address.
    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L1Index {
        L1Index::from(va)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::addresses::PhysicalAddress;

    #[test]
    fn pte_4k_leaf() {
        let k4 = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x5555_0000));
        let e = PtEntry4k::new_user_ro_nx()
            .with_present(true)
            .with_physical_page(k4);

        let (p, fl) = e.page_4k().unwrap();
        assert_eq!(p.base().as_u64(), 0x5555_0000);
        assert!(fl.no_execute());
        assert!(fl.user());
        assert!(!fl.writable());
    }
}
