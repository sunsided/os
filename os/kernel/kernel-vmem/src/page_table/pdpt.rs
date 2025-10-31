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

use crate::addresses::{PhysicalAddress, PhysicalPage, Size1G, Size4K, VirtualAddress};
use crate::page_table::{PRESENT_BIT, PS_BIT};
use bitfield_struct::bitfield;

/// **Borrowed view** into an L3 PDPTE.
///
/// Returned by [`PdptEntry::view`].
pub enum L3View {
    /// Non-leaf PDPTE view (PS=0).
    Entry(Pdpte),
    /// 1 GiB leaf PDPTE view (PS=1).
    Leaf1G(Pdpte1G),
}

/// **L3 PDPTE union** — overlays non-leaf [`Pdpte`] and leaf [`Pdpte1G`]
/// on the same 64-bit storage.
///
/// Use [`PdptEntry::view`] to obtain a **typed**
/// reference. These methods inspect the **PS** bit to decide which variant is
/// active and return a safe borrowed view.
///
/// Storing/retrieving raw bits is possible via `from_bits`/`into_bits`.
#[derive(Copy, Clone)]
#[repr(C)]
pub union PdptEntry {
    /// Raw 64-bit storage of the entry.
    bits: u64,
    /// Non-leaf form: next-level Page Directory (PS=0).
    entry: Pdpte,
    /// Leaf form: 1 GiB mapping (PS=1).
    leaf_1g: Pdpte1G,
}

/// L3 **PDPTE** — pointer to a **Page Directory** (non-leaf; PS **= 0**).
///
/// - Physical address (bits **51:12**) is a 4 KiB-aligned PD.
/// - Leaf-only fields (Dirty/Global) are ignored.
/// - Setting PS here would mean a 1 GiB leaf; use [`Pdpte1G`] for that.
#[bitfield(u64)]
pub struct Pdpte {
    /// Present (bit 0): valid entry if set.
    pub present: bool,
    /// Writable (bit 1): write permission.
    pub writable: bool,
    /// User (bit 2): user-mode access if set.
    pub user: bool,
    /// Write-Through (bit 3).
    pub write_through: bool,
    /// Cache Disable (bit 4).
    pub cache_disable: bool,
    /// Accessed (bit 5).
    pub accessed: bool,
    /// Dirty (bit 6): **ignored** in non-leaf form.
    #[bits(1)]
    __d_ignored: u8,
    /// PS (bit 7): **must be 0** in non-leaf.
    #[bits(1)]
    __ps_must_be_0: u8,
    /// Global (bit 8): **ignored** in non-leaf.
    #[bits(1)]
    __g_ignored: u8,
    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,
    /// Next-level table physical address (bits 12..51, 4 KiB-aligned).
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

/// L3 **PDPTE (1 GiB leaf)** — maps a single 1 GiB page (`PS = 1`).
///
/// - **PAT** (Page Attribute Table) selector lives at bit **12** in this form.
/// - Physical address uses bits **51:30** and must be **1 GiB aligned**.
/// - `Dirty` is set by CPU on first write; `Global` keeps TLB entries across
///   CR3 reload unless explicitly invalidated.
///
/// This is a terminal mapping (leaf).
#[bitfield(u64)]
pub struct Pdpte1G {
    /// Present (bit 0).
    pub present: bool,
    /// Writable (bit 1).
    pub writable: bool,
    /// User (bit 2).
    pub user: bool,
    /// Write-Through (bit 3).
    pub write_through: bool,
    /// Cache Disable (bit 4).
    pub cache_disable: bool,
    /// Accessed (bit 5).
    pub accessed: bool,
    /// **Dirty** (bit 6): set by CPU on first write to this 1 GiB page.
    pub dirty: bool,
    /// **Page Size** (bit 7): **must be 1** for 1 GiB leaf.
    #[bits(default = true)]
    page_size: bool,
    /// **Global** (bit 8): TLB entry not flushed on CR3 reload.
    pub global: bool,
    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,
    /// **PAT** (Page Attribute Table) selector for 1 GiB mappings (bit 12).
    pub pat_large: bool,
    /// Reserved (bits 13..29): must be 0.
    #[bits(17)]
    __res_13_29: u32,
    /// Physical address bits **51:30** (1 GiB-aligned base).
    #[bits(22)]
    phys_addr_51_30: u32,
    /// OS-available high (bits 52..58).
    #[bits(7)]
    pub os_available_high: u8,
    /// Protection Key / OS use (59..62).
    #[bits(4)]
    pub protection_key: u8,
    /// No-Execute (bit 63).
    pub no_execute: bool,
}

impl Pdpte {
    /// Set the Page Directory base (4 KiB-aligned).
    #[inline]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        debug_assert!(phys.is_aligned_to(0x1000));
        self.set_phys_addr_51_12(phys.as_u64() >> 12);
    }

    /// Get the Page Directory base (4 KiB-aligned).
    #[inline]
    #[must_use]
    pub const fn physical_address(self) -> PhysicalAddress {
        PhysicalAddress::new(self.phys_addr_51_12() << 12)
    }

    /// Non-leaf PDPTE with common kernel RW flags.
    #[inline]
    #[must_use]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user(false)
            .with_write_through(false)
            .with_cache_disable(false)
            .with_no_execute(false)
    }
}

impl Pdpte1G {
    /// Set the 1 GiB page base (must be 1 GiB-aligned).
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        debug_assert!(phys.is_aligned_to(1 << 30));
        self.set_phys_addr_51_30((phys.as_u64() >> 30) as u32);
        self.set_page_size(true);
    }

    /// Get the 1 GiB page base.
    #[inline]
    #[must_use]
    pub const fn physical_address(self) -> PhysicalAddress {
        PhysicalAddress::new((self.phys_addr_51_30() as u64) << 30)
    }

    /// Leaf PDPTE with common kernel RW flags.
    #[inline]
    #[must_use]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user(false)
            .with_write_through(false)
            .with_cache_disable(false)
            .with_no_execute(false)
            .with_page_size(true)
    }
}

/// Index into the PDPT (derived from virtual-address bits `[38:30]`).
///
/// This strongly-typed index avoids mixing levels and constrains the range
/// to `0..512` (checked in debug builds).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L3Index(u16);

/// Decoded PDPT entry kind.
///
/// - [`NextPageDirectory`](PdptEntryKind::NextPageDirectory): non-leaf; `PS=0`; holds the 4 KiB-aligned PD base.
/// - [`Leaf1GiB`](PdptEntryKind::Leaf1GiB): leaf; `PS=1`; holds the 1 GiB-aligned large-page base.
pub enum PdptEntryKind {
    NextPageDirectory(PhysicalPage<Size4K>, Pdpte),
    Leaf1GiB(PhysicalPage<Size1G>, Pdpte1G),
}

/// The PDPT (L3) table: 512 entries, 4 KiB aligned.
#[doc(alias = "PDPT")]
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

impl Default for PdptEntry {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl PdptEntry {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    #[inline]
    #[must_use]
    pub const fn new_entry(entry: Pdpte) -> Self {
        Self { entry }
    }

    #[inline]
    #[must_use]
    pub const fn new_leaf(leaf: Pdpte1G) -> Self {
        Self { leaf_1g: leaf }
    }

    #[inline]
    #[must_use]
    pub const fn present(self) -> bool {
        unsafe { self.bits & PRESENT_BIT != 0 }
    }

    /// Construct union from raw `bits` (no validation).
    #[inline]
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Extract raw `bits` back from the union.
    #[inline]
    #[must_use]
    pub const fn into_bits(self) -> u64 {
        unsafe { self.bits }
    }

    /// **Typed read-only view** chosen by the **PS** bit.
    ///
    /// - If PS=1 → [`L3View::Leaf1G`]
    /// - If PS=0 → [`L3View::Entry`]
    ///
    /// This function is safe: it returns a view consistent with the PS bit.
    #[inline]
    #[must_use]
    pub const fn view(self) -> L3View {
        unsafe {
            if (self.bits & PS_BIT) != 0 {
                L3View::Leaf1G(self.leaf_1g)
            } else {
                L3View::Entry(self.entry)
            }
        }
    }

    /// Create a zero (non-present) entry.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self::new()
    }

    /// Decode the entry into its semantic kind, or `None` if not present.
    ///
    /// - When `PS=1`, returns [`PdptEntryKind::Leaf1GiB`] with a 1 GiB page base.
    /// - When `PS=0`, returns [`PdptEntryKind::NextPageDirectory`] with a PD base.
    #[inline]
    #[must_use]
    pub const fn kind(self) -> Option<PdptEntryKind> {
        if !self.present() {
            return None;
        }

        Some(match self.view() {
            L3View::Entry(entry) => {
                let base = entry.physical_address();
                PdptEntryKind::NextPageDirectory(PhysicalPage::<Size4K>::from_addr(base), entry)
            }
            L3View::Leaf1G(entry) => {
                let base = entry.physical_address();
                PdptEntryKind::Leaf1GiB(PhysicalPage::<Size1G>::from_addr(base), entry)
            }
        })
    }

    /// Create a non-leaf PDPTE that points to a Page Directory (`PS=0`).
    ///
    /// Sets `present=1`, forces `PS=0`, and writes the PD base address.
    /// The PD base must be 4 KiB-aligned.
    #[inline]
    #[must_use]
    pub const fn make_next(pd_page: PhysicalPage<Size4K>, mut flags: Pdpte) -> Self {
        flags.set_present(true);
        flags.set_physical_address(pd_page.base());
        Self::new_entry(flags)
    }

    /// Create a 1 GiB leaf PDPTE (`PS=1`).
    ///
    /// Sets `present=1`, forces `PS=1`, and writes the large-page base address.
    /// The page base must be 1 GiB-aligned.
    #[inline]
    #[must_use]
    pub const fn make_1g(page: PhysicalPage<Size1G>, mut flags: Pdpte1G) -> Self {
        flags.set_present(true);
        flags.set_physical_address(page.base());
        Self::new_leaf(flags)
    }
}

impl From<Pdpte> for PdptEntry {
    #[inline]
    fn from(e: Pdpte) -> Self {
        Self::new_entry(e)
    }
}

impl From<Pdpte1G> for PdptEntry {
    #[inline]
    fn from(e: Pdpte1G) -> Self {
        Self::new_leaf(e)
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

    /// Set the entry at `i` to [`PdptEntry::zero`].
    ///
    /// Caller is responsible for necessary TLB invalidations if this affects an
    /// active address space.
    #[inline]
    pub const fn set_zero(&mut self, i: L3Index) {
        self.set(i, PdptEntry::zero());
    }

    /// Derive the PDPT index from a virtual address.
    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L3Index {
        L3Index::from(va)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::addresses::PhysicalAddress;

    #[test]
    fn pdpt_table_vs_1g() {
        // next-level PD
        let pd = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x2000_0000));
        let e_tbl = PdptEntry::make_next(pd, Pdpte::new_common_rw());
        match e_tbl.kind().unwrap() {
            PdptEntryKind::NextPageDirectory(p, f) => {
                assert_eq!(p.base().as_u64(), 0x2000_0000);
                assert_eq!(f.into_bits() & (1 << 7), 0, "must be PS=0");
            }
            _ => panic!("expected next PD"),
        }

        // 1 GiB leaf
        let g1 = PhysicalPage::<Size1G>::from_addr(PhysicalAddress::new(0x8000_0000));
        let e_1g = PdptEntry::make_1g(g1, Pdpte1G::new_common_rw());
        match e_1g.kind().unwrap() {
            PdptEntryKind::Leaf1GiB(p, f) => {
                assert_eq!(p.base().as_u64(), 0x8000_0000);
                assert_ne!(f.into_bits() & (1 << 7), 0, "must be PS=1");
            }
            _ => panic!("expected 1GiB leaf"),
        }
    }
}
