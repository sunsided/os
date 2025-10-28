//! # x86-64 Page Map Level 4 (PML4)
//!
//! This module defines strongly-typed wrappers around the top-level x86-64
//! page-table layer (PML4):
//!
//! - [`L4Index`]: index type for bits 47..39 of a canonical virtual address.
//! - [`Pml4Entry`]: a single PML4 entry (must not be a large page).
//! - [`PageMapLevel4`]: a 4 KiB-aligned array of 512 PML4 entries.
//!
//! ## Background
//!
//! In 4-level paging, the PML4 selects a Page-Directory-Pointer Table ([PDPT](super::pdpt::PageDirectoryPointerTable)).
//! A PML4E must have `PS=0` (no large pages at this level). Each entry holds
//! flags and the physical base address of the next-level table. The index is
//! derived from VA bits `[47:39]`.
//!
//! ## Guarantees & Invariants
//!
//! - [`PageMapLevel4`] is 4 KiB-aligned and has exactly 512 entries.
//! - [`Pml4Entry::make`] enforces `PS=0` for PML4Es.
//! - Accessors avoid unsafe operations and prefer explicit types such as
//!   [`PhysicalPage<Size4K>`] for next-level tables.
//!
//! ## Notation
//!
//! `present`, `large_page (PS)`, and addresses/flags are delegated to
//! [`PageEntryBits`], which encapsulates bit-level manipulation.

use crate::PageEntryBits;
use crate::addr2::{PhysicalPage, Size4K, VirtualAddress};

/// Index into the PML4 table (derived from virtual-address bits `[47:39]`).
///
/// This newtype prevents accidental mixing with other indices and allows
/// compile-time checking of valid index ranges (0..512).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L4Index(u16);

/// A single PML4 entry (PML4E).
///
/// Semantics:
/// - Points to a next-level L3 table (PDPT).
/// - The `PS` (Page Size) bit **must be 0** at this level.
/// - Presence and other permission/cache flags live in the inner
///   [`PageEntryBits`].
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Pml4Entry(PageEntryBits);

/// The top-level page map (PML4).
///
/// Layout:
/// - 512 entries, 8 bytes each (4096 bytes total).
/// - 4 KiB aligned, as required by the hardware.
///
/// Use [`PageMapLevel4::get`] and [`PageMapLevel4::set`] to read/write entries.
#[repr(C, align(4096))]
pub struct PageMapLevel4 {
    entries: [Pml4Entry; 512],
}

impl L4Index {
    /// Construct an index from a canonical virtual address by extracting bits `[47:39]`.
    ///
    /// Returns a value in `0..512`.
    #[inline]
    #[must_use]
    pub const fn from(va: VirtualAddress) -> Self {
        Self::new(((va.as_u64() >> 39) & 0x1FF) as u16)
    }

    /// Construct an index from a raw `u16`.
    ///
    /// ### Panics / Debug assertions
    /// - Debug builds assert `v < 512`.
    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        debug_assert!(v < 512);
        Self(v)
    }

    /// Return the index as `usize` for array indexing.
    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl Pml4Entry {
    /// Create a zero (non-present) entry with all bits cleared.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(PageEntryBits::new())
    }

    /// Check whether the entry is marked present.
    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    /// Return the raw [`PageEntryBits`] for advanced inspection/masking.
    ///
    /// Prefer higher-level helpers where possible.
    #[inline]
    #[must_use]
    pub const fn flags(self) -> PageEntryBits {
        self.0
    }

    /// If present, return the physical page of the next-level PDPT.
    ///
    /// Returns `None` if the entry is not present. The returned page is always
    /// 4 KiB-aligned as required for page-table bases.
    #[inline]
    #[must_use]
    pub const fn next_table(self) -> Option<PhysicalPage<Size4K>> {
        if !self.is_present() {
            return None;
        }
        Some(PhysicalPage::from_addr(self.0.physical_address()))
    }

    /// Build a PML4 entry that points to the given PDPT page and applies the provided flags.
    ///
    /// ### Requirements
    /// - `flags.large_page()` **must be false** (`PS=0`). Enforced via `debug_assert!`.
    /// - This function sets `present=1` and the physical base to `next_pdpt_page.base()`.
    #[inline]
    #[must_use]
    pub fn make(next_pdpt_page: PhysicalPage<Size4K>, mut flags: PageEntryBits) -> Self {
        debug_assert!(!flags.large_page(), "PML4E must have PS=0");
        flags.set_present(true);
        flags.set_physical_address(next_pdpt_page.base());
        Self(flags)
    }

    /// Return the raw 64-bit value of the entry (flags + address).
    #[inline]
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0.into()
    }

    /// Construct an entry from a raw 64-bit value.
    ///
    /// No validation is performed here; callers must ensure `PS=0` for PML4Es.
    #[inline]
    #[must_use]
    pub fn from_raw(v: u64) -> Self {
        Self(PageEntryBits::from(v))
    }
}

impl PageMapLevel4 {
    /// Create a fully zeroed (all entries non-present) PML4 table.
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [Pml4Entry::zero(); 512],
        }
    }

    /// Read the entry at the given index.
    ///
    /// This is a plain fetch; it does not perform TLB synchronization.
    #[inline]
    #[must_use]
    pub const fn get(&self, i: L4Index) -> Pml4Entry {
        self.entries[i.as_usize()]
    }

    /// Write the entry at the given index.
    ///
    /// Caller is responsible for any required TLB invalidation after modifying
    /// mappings that affect active address spaces.
    #[inline]
    pub const fn set(&mut self, i: L4Index, e: Pml4Entry) {
        self.entries[i.as_usize()] = e;
    }

    /// Derive the [`L4Index`] from a virtual address.
    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L4Index {
        L4Index::from(va)
    }
}
