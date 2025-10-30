//! # x86-64 Page Directory (PD / L2)
//!
//! This module models the second paging level (L2, Page Directory):
//!
//! - [`L2Index`]: index type for virtual-address bits `[29:21]`.
//! - [`PdEntry`]: a PD entry that is either a pointer to a PT (`PS=0`) or a 2 MiB leaf (`PS=1`).
//! - [`PdEntryKind`]: decoded view of an entry (next PT vs 2 MiB leaf).
//! - [`PageDirectory`]: a 4 KiB-aligned array of 512 PD entries.
//!
//! ## Semantics
//!
//! At L2, the `PS` bit selects the role of an entry:
//! - `PS=0`: entry points to a next-level Page Table (PT), whose base is 4 KiB-aligned.
//! - `PS=1`: entry is a 2 MiB leaf mapping; base must be 2 MiB-aligned.
//!
//! ## Invariants & Notes
//!
//! - [`PageDirectory`] is 4 KiB-aligned and contains exactly 512 entries.
//! - [`PdEntry::make_next`] forces `PS=0`; [`PdEntry::make_2m`] forces `PS=1`.
//! - Raw constructors don’t validate consistency; callers must ensure correctness.
//! - TLB maintenance is the caller’s responsibility after mutating active mappings.

use crate::addresses::{PhysicalPage, Size2M, Size4K, VirtualAddress};
use crate::page_table::bits2::{L2View, Pde, Pde2M, PdeUnion};

/// Index into the Page Directory (derived from VA bits `[29:21]`).
///
/// Strongly-typed to avoid mixing with other levels. Range is `0..512`
/// (checked in debug builds).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L2Index(u16);

/// A single Page Directory entry (PDE).
///
/// Semantics:
///
/// - If `PS=0`, points to a Page Table (PT).
/// - If `PS=1`, encodes a 2 MiB leaf mapping.
///
/// All permission/cache/present bits are contained in the inner [`PageEntryBits`].
#[doc(alias = "PDE")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PdEntry(PdeUnion);

/// Decoded PDE kind.
///
/// - [`NextPageTable`](PdEntryKind::NextPageTable): non-leaf (`PS=0`), contains the 4 KiB-aligned PT base.
/// - [`Leaf2MiB`](PdEntryKind::Leaf2MiB): leaf (`PS=1`), contains the 2 MiB-aligned large-page base.
pub enum PdEntryKind {
    NextPageTable(PhysicalPage<Size4K>, Pde),
    Leaf2MiB(PhysicalPage<Size2M>, Pde2M),
}

/// The Page Directory (L2): 512 entries, 4 KiB-aligned.
#[doc(alias = "PD")]
#[repr(C, align(4096))]
pub struct PageDirectory {
    entries: [PdEntry; 512],
}

impl L2Index {
    /// Build an index from a canonical virtual address (extracts bits `[29:21]`).
    ///
    /// Returns a value in `0..512`.
    #[inline]
    #[must_use]
    pub const fn from(va: VirtualAddress) -> Self {
        Self::new(((va.as_u64() >> 21) & 0x1FF) as u16)
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

impl PdEntry {
    /// Create a zero (non-present) entry.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(PdeUnion::new())
    }

    /// Return `true` if the entry is marked present.
    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    /// Expose the underlying bitfield for advanced inspection/masking.
    #[inline]
    #[must_use]
    pub const fn view(&self) -> L2View<'_> {
        self.0.view()
    }

    /// Decode the entry into its semantic kind, or `None` if not present.
    ///
    /// - When `PS=1`, returns [`PdEntryKind::Leaf2MiB`] with a 2 MiB page base.
    /// - When `PS=0`, returns [`PdEntryKind::NextPageTable`] with a PT base.
    #[inline]
    #[must_use]
    pub const fn kind(self) -> Option<PdEntryKind> {
        if !self.is_present() {
            return None;
        }

        Some(match self.view() {
            L2View::Entry(entry) => {
                let base = entry.physical_address();
                PdEntryKind::NextPageTable(PhysicalPage::<Size4K>::from_addr(base), *entry)
            }
            L2View::Leaf2M(entry) => {
                let base = entry.physical_address();
                PdEntryKind::Leaf2MiB(PhysicalPage::<Size2M>::from_addr(base), *entry)
            }
        })
    }

    /// Create a non-leaf PDE that points to a Page Table (`PS=0`).
    ///
    /// Sets `present=1`, forces `PS=0`, and writes the PT base address.
    /// The PT base must be 4 KiB-aligned.
    #[inline]
    #[must_use]
    pub const fn make_next(pt_page: PhysicalPage<Size4K>, mut flags: Pde) -> Self {
        flags.set_present(true);
        flags.set_physical_address(pt_page.base());
        Self(PdeUnion::new_entry(flags))
    }

    /// Create a 2 MiB leaf PDE (`PS=1`).
    ///
    /// Sets `present=1`, forces `PS=1`, and writes the large-page base address.
    /// The base must be 2 MiB-aligned.
    #[inline]
    #[must_use]
    pub const fn make_2m(page: PhysicalPage<Size2M>, mut flags: Pde2M) -> Self {
        flags.set_present(true);
        flags.set_physical_address(page.base());
        flags.set_page_size(true);
        Self(PdeUnion::new_leaf(flags))
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
    pub const fn from_raw(v: u64) -> Self {
        Self(PdeUnion::from_bits(v))
    }
}

impl PageDirectory {
    /// Create a fully zeroed Page Directory (all entries non-present).
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [PdEntry::zero(); 512],
        }
    }

    /// Read the entry at `i`.
    ///
    /// Plain load; does not imply any TLB synchronization.
    #[inline]
    #[must_use]
    pub const fn get(&self, i: L2Index) -> PdEntry {
        self.entries[i.as_usize()]
    }

    /// Write the entry at `i`.
    ///
    /// Caller must handle any required TLB invalidation when changing active mappings.
    #[inline]
    pub const fn set(&mut self, i: L2Index, e: PdEntry) {
        self.entries[i.as_usize()] = e;
    }

    /// Set the entry at `i` to [`PdEntry::zero`].
    ///
    /// Caller is responsible for necessary TLB invalidations if this affects an
    /// active address space.
    #[inline]
    pub const fn set_zero(&mut self, i: L2Index) {
        self.set(i, PdEntry::zero());
    }

    /// Derive the PD index from a virtual address.
    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L2Index {
        L2Index::from(va)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::addresses::PhysicalAddress;

    #[test]
    fn pd_table_vs_2m() {
        let pt = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x3000_0000));
        let e_tbl = PdEntry::make_next(pt, Pde::new_common_rw());
        match e_tbl.kind().unwrap() {
            PdEntryKind::NextPageTable(p, f) => {
                assert_eq!(p.base().as_u64(), 0x3000_0000);
                assert_eq!(f.into_bits() & (1 << 7), 0, "must be PS=0");
            }
            _ => panic!("expected next PT"),
        }

        let m2 = PhysicalPage::<Size2M>::from_addr(PhysicalAddress::new(0x4000_0000));
        let e_2m = PdEntry::make_2m(m2, Pde2M::new_common_rw());
        match e_2m.kind().unwrap() {
            PdEntryKind::Leaf2MiB(p, f) => {
                assert_eq!(p.base().as_u64(), 0x4000_0000);
                assert_ne!(f.into_bits() & (1 << 7), 0, "must be PS=1");
            }
            _ => panic!("expected 2MiB leaf"),
        }
    }
}
