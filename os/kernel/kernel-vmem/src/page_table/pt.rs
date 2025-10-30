//! # x86-64 Page Table (PT / L1)
//!
//! This module models the lowest paging level (L1, Page Table).
//!
//! - [`L1Index`]: index type for VA bits `[20:12]`.
//! - [`PtEntry`]: a PT entry (PTE). At this level, `PS` **must be 0**; entries
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
//! - [`PtEntry::make_4k`] forces `PS=0` and `present=1`.
//! - Raw constructors do not validate consistency; prefer typed helpers.
//! - After modifying active mappings, the caller must perform any required TLB maintenance.

use crate::PageEntryBits;
use crate::addresses::{PhysicalPage, Size4K, VirtualAddress};

/// Index into the Page Table (derived from VA bits `[20:12]`).
///
/// Strongly typed to avoid mixing with other levels. Range is `0..512`
/// (checked in debug builds).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L1Index(u16);

/// A single Page Table entry (PTE).
///
/// Semantics:
///
/// - At L1, `PS` **must be 0** (no large pages here).
/// - A present PTE maps exactly one 4 KiB page.
///
/// All permission/cache/present bits live inside the inner [`PageEntryBits`].
#[doc(alias = "PTE")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PtEntry(PageEntryBits);

/// The Page Table (L1): 512 entries, 4 KiB-aligned.
#[doc(alias = "PT")]
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PtEntry; 512],
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

impl PtEntry {
    /// Create a zero (non-present) entry.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(PageEntryBits::new())
    }

    /// Return `true` if the entry is marked present.
    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    /// Expose the underlying bitfield for advanced inspection/masking.
    ///
    /// Prefer typed helpers when possible.
    #[inline]
    #[must_use]
    pub const fn flags(self) -> PageEntryBits {
        self.0
    }

    /// If present, return the mapped 4 KiB physical page and its flags.
    ///
    /// Debug-asserts that `PS=0` (required at L1).
    #[inline]
    #[must_use]
    pub fn page_4k(self) -> Option<(PhysicalPage<Size4K>, PageEntryBits)> {
        if !self.is_present() {
            return None;
        }
        debug_assert!(!self.0.large_page(), "PTE must have PS=0");
        Some((PhysicalPage::from_addr(self.0.physical_address()), self.0))
    }

    /// Create a 4 KiB leaf PTE (`PS=0`).
    ///
    /// Sets `present=1`, forces `PS=0`, and writes the page base address.
    /// The base must be 4 KiB-aligned.
    #[inline]
    #[must_use]
    pub const fn make_4k(page: PhysicalPage<Size4K>, mut flags: PageEntryBits) -> Self {
        flags.set_large_page(false);
        flags.set_present(true);
        flags.set_physical_address(page.base());
        Self(flags)
    }

    /// Return the raw 64-bit value (flags + address).
    #[inline]
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0.into()
    }

    /// Construct from a raw 64-bit value.
    ///
    /// No validation is performed; callers must ensure `PS=0` at L1.
    #[inline]
    #[must_use]
    pub fn from_raw(v: u64) -> Self {
        Self(PageEntryBits::from(v))
    }
}

impl PageTable {
    /// Create a fully zeroed Page Table (all entries non-present).
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [PtEntry::zero(); 512],
        }
    }

    /// Read the entry at `i`.
    ///
    /// Plain load; does not imply any TLB synchronization.
    #[inline]
    #[must_use]
    pub const fn get(&self, i: L1Index) -> PtEntry {
        self.entries[i.as_usize()]
    }

    /// Write the entry at `i`.
    ///
    /// Caller must handle any required TLB invalidation when changing active mappings.
    #[inline]
    pub const fn set(&mut self, i: L1Index, e: PtEntry) {
        self.entries[i.as_usize()] = e;
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
        let e = PtEntry::make_4k(k4, PageEntryBits::new_user_ro_nx());
        let (p, fl) = e.page_4k().unwrap();
        assert_eq!(p.base().as_u64(), 0x5555_0000);
        assert!(!fl.large_page());
        assert!(fl.no_execute());
        assert!(fl.user_access());
        assert!(!fl.writable());
    }
}
