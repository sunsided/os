//! # x86-64 Page Directory Pointer Table (PDPT / L3)
//!
//! This module wraps the third paging level (a.k.a. L3, PDPT):
//!
//! - [`L3Index`]: index type for VA bits `[38:30]`.
//! - [`PdptEntry`]: a single PDPT entry; may be a next-level pointer or a 1 GiB leaf.
//! - [`PdptEntryKind`]: decoded view of an entry (next PD or 1 GiB leaf).
//! - [`PageDirectoryPointerTable`]: a 4 KiB-aligned array of 512 entries.
//!
//! ## Semantics
//!
//! At the PDPT level, the `PS` bit controls whether the entry is a 1 GiB leaf
//! (`PS=1`) or points to a Page Directory (`PS=0`). Leaf entries map 1 GiB and
//! require 1 GiB physical alignment of the base address. Non-leaf entries hold
//! the physical base of the next-level Page Directory (4 KiB aligned).
//!
//! ## Invariants & Safety Notes
//!
//! - [`PageDirectoryPointerTable`] is 4 KiB-aligned and contains exactly 512 entries.
//! - [`PdptEntry::make_next`] enforces `PS=0`; [`PdptEntry::make_1g`] enforces `PS=1`.
//! - Callers must handle TLB maintenance after changing active mappings.
//! - Raw constructors perform no validation; use with care.

use crate::PageEntryBits;
use crate::addr2::{PhysicalPage, Size1G, Size4K, VirtualAddress};

/// Index into the PDPT (derived from virtual-address bits `[38:30]`).
///
/// This strongly-typed index avoids mixing levels and constrains the range
/// to `0..512` (checked in debug builds).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L3Index(u16);

/// A single PDPT entry (PDPTE).
///
/// Semantics:
///
/// - If `PS=0`, the entry points to a Page Directory (PD).
/// - If `PS=1`, the entry is a 1 GiB leaf mapping.
///
/// Other permission/cache/present bits live inside [`PageEntryBits`].
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PdptEntry(PageEntryBits);

/// Decoded PDPT entry kind.
///
/// - [`NextPageDirectory`]: non-leaf; `PS=0`; holds the 4 KiB-aligned PD base.
/// - [`Leaf1GiB`]: leaf; `PS=1`; holds the 1 GiB-aligned large-page base.
pub enum PdptEntryKind {
    NextPageDirectory(PhysicalPage<Size4K>, PageEntryBits),
    Leaf1GiB(PhysicalPage<Size1G>, PageEntryBits),
}

/// The PDPT (L3) table: 512 entries, 4 KiB aligned.
#[repr(C, align(4096))]
pub struct PageDirectoryPointerTable {
    entries: [PdptEntry; 512],
}

impl L3Index {
    /// Build an index from a canonical virtual address (extracts bits `[38:30]`).
    ///
    /// Returns a value in `0..512`.
    #[inline]
    #[must_use]
    pub const fn from(va: VirtualAddress) -> Self {
        Self::new(((va.as_u64() >> 30) & 0x1FF) as u16)
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

impl PdptEntry {
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

    /// Expose the underlying flag/address bitfield for advanced use.
    ///
    /// Prefer using typed helpers where possible.
    #[inline]
    #[must_use]
    pub const fn flags(self) -> PageEntryBits {
        self.0
    }

    /// Decode the entry into its semantic kind, or `None` if not present.
    ///
    /// - When `PS=1`, returns [`PdptEntryKind::Leaf1GiB`] with a 1 GiB page base.
    /// - When `PS=0`, returns [`PdptEntryKind::NextPageDirectory`] with a PD base.
    #[inline]
    #[must_use]
    pub const fn kind(self) -> Option<PdptEntryKind> {
        if !self.is_present() {
            return None;
        }

        let flags = self.0;
        let base = self.0.physical_address();
        if flags.large_page() {
            Some(PdptEntryKind::Leaf1GiB(
                PhysicalPage::<Size1G>::from_addr(base),
                flags,
            ))
        } else {
            Some(PdptEntryKind::NextPageDirectory(
                PhysicalPage::<Size4K>::from_addr(base),
                flags,
            ))
        }
    }

    /// Create a non-leaf PDPTE that points to a Page Directory (`PS=0`).
    ///
    /// Sets `present=1`, forces `PS=0`, and writes the PD base address.
    /// The PD base must be 4 KiB-aligned.
    #[inline]
    #[must_use]
    pub const fn make_next(pd_page: PhysicalPage<Size4K>, mut flags: PageEntryBits) -> Self {
        flags.set_large_page(false);
        flags.set_present(true);
        flags.set_physical_address(pd_page.base());
        Self(flags)
    }

    /// Create a 1 GiB leaf PDPTE (`PS=1`).
    ///
    /// Sets `present=1`, forces `PS=1`, and writes the large-page base address.
    /// The page base must be 1 GiB-aligned.
    #[inline]
    #[must_use]
    pub const fn make_1g(page: PhysicalPage<Size1G>, mut flags: PageEntryBits) -> Self {
        flags.set_large_page(true);
        flags.set_present(true);
        flags.set_physical_address(page.base());
        Self(flags)
    }

    /// Return the raw 64-bit value (flags + address).
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0.into_bits()
    }

    /// Construct from a raw 64-bit value.
    ///
    /// No validation is performed; callers must ensure a consistent `PS`/kind.
    #[inline]
    #[must_use]
    pub fn from_raw(v: u64) -> Self {
        Self(PageEntryBits::from(v))
    }
}

impl PageDirectoryPointerTable {
    /// Create a fully zeroed PDPT (all entries non-present).
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [PdptEntry::zero(); 512],
        }
    }

    /// Read an entry at `i`.
    ///
    /// Plain load; does not imply any TLB maintenance.
    #[inline]
    #[must_use]
    pub const fn get(&self, i: L3Index) -> PdptEntry {
        self.entries[i.as_usize()]
    }

    /// Write an entry at `i`.
    ///
    /// Caller is responsible for necessary TLB invalidations if this affects an
    /// active address space.
    #[inline]
    pub const fn set(&mut self, i: L3Index, e: PdptEntry) {
        self.entries[i.as_usize()] = e;
    }

    /// Derive the PDPT index from a virtual address.
    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L3Index {
        L3Index::from(va)
    }
}
