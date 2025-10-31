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

use crate::VirtualMemoryPageBits;
use crate::addresses::{PhysicalAddress, PhysicalPage, Size2M, Size4K, VirtualAddress};
use crate::page_table::{PRESENT_BIT, PS_BIT};
use bitfield_struct::bitfield;

/// **Borrowed view** into an L2 PDE.
///
/// Returned by [`PdEntry::view`].
pub enum L2View {
    /// Non-leaf PDE view (PS=0).
    Entry(Pde),
    /// 2 MiB leaf PDE view (PS=1).
    Leaf2M(Pde2M),
}

/// **L2 PDE union** — overlays non-leaf [`Pde`] and leaf [`Pde2M`]
/// on the same 64-bit storage.
///
/// Prefer [`PdEntry::view`] for safe typed access.
/// These check the **PS** bit and hand you the correct variant.
#[derive(Copy, Clone)]
#[repr(C)]
pub union PdEntry {
    /// Raw 64-bit storage of the entry.
    bits: u64,
    /// Non-leaf form: next-level Page Table (PS=0).
    entry: Pde,
    /// Leaf form: 2 MiB mapping (PS=1).
    leaf_2m: Pde2M,
}

/// L2 **PDE** — pointer to a **Page Table** (non-leaf; PS **= 0**).
///
/// - Physical address (bits **51:12**) is a 4 KiB-aligned PT.
/// - In non-leaf PDEs, **PAT lives at bit 12 only in the leaf form**;
///   here, all bits 12..51 are the next-level table address.
#[bitfield(u64)]
pub struct Pde {
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
    /// Dirty (bit 6): **ignored** in non-leaf.
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

    /// **Next-level table physical address** (bits 12..51, 4 KiB-aligned).
    ///
    /// Note: Do **not** insert reserved placeholders here; in non-leaf form
    /// these bits are entirely the PT base address.
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

impl Pde {
    /// Set the Page Table base (4 KiB-aligned).
    #[inline]
    #[must_use]
    pub const fn with_physical_page(mut self, phys: PhysicalPage<Size4K>) -> Self {
        self.set_physical_page(phys);
        self
    }

    /// Set the Page Table base (4 KiB-aligned).
    #[inline]
    pub const fn set_physical_page(&mut self, phys: PhysicalPage<Size4K>) {
        self.set_phys_addr_51_12(phys.base().as_u64() >> 12);
    }

    /// Get the Page Table base.
    #[inline]
    #[must_use]
    pub const fn physical_address(self) -> PhysicalPage<Size4K> {
        PhysicalPage::from_addr(PhysicalAddress::new(self.phys_addr_51_12() << 12))
    }

    /// Non-leaf PDE with common kernel RW flags.
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

/// L2 **PDE (2 MiB leaf)** — maps a single 2 MiB page (`PS = 1`).
///
/// - **PAT** (Page Attribute Table) selector lives at bit **12** in this form.
/// - Physical address uses bits **51:21** and must be **2 MiB aligned**.
/// - `Dirty` is set by CPU on first write; `Global` keeps TLB entries across
///   CR3 reload unless explicitly invalidated.
///
/// This is a terminal mapping (leaf).
#[bitfield(u64)]
pub struct Pde2M {
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
    /// **Dirty** (bit 6): set by CPU on first write to this 2 MiB page.
    pub dirty: bool,
    /// **Page Size** (bit 7): **must be 1** for 2 MiB leaf.
    #[bits(default = true)]
    pub(crate) page_size: bool,
    /// **Global** (bit 8): TLB entry not flushed on CR3 reload.
    pub global: bool,
    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,
    /// **PAT** (Page Attribute Table) selector for 2 MiB mappings (bit 12).
    pub pat_large: bool,
    /// Reserved (bits 13..20): must be 0.
    #[bits(8)]
    __res13_20: u8,
    /// Physical address bits **51:21** (2 MiB-aligned base).
    #[bits(31)]
    phys_addr_51_21: u32,
    /// OS-available high (bits 52..58).
    #[bits(7)]
    pub os_available_high: u8,
    /// Protection Key / OS use (59..62).
    #[bits(4)]
    pub protection_key: u8,
    /// No-Execute (bit 63).
    pub no_execute: bool,
}

impl Pde2M {
    /// Set the 2 MiB page base (must be 2 MiB-aligned).
    #[inline]
    #[must_use]
    pub const fn with_physical_page(mut self, phys: PhysicalPage<Size2M>) -> Self {
        self.set_physical_page(phys);
        self
    }

    /// Set the 2 MiB page base (must be 2 MiB-aligned).
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn set_physical_page(&mut self, phys: PhysicalPage<Size2M>) {
        self.set_phys_addr_51_21((phys.base().as_u64() >> 21) as u32);
        self.set_page_size(true);
    }

    /// Get the 2 MiB page base.
    #[inline]
    #[must_use]
    pub const fn physical_page(self) -> PhysicalPage<Size2M> {
        PhysicalPage::from_addr(PhysicalAddress::new((self.phys_addr_51_21() as u64) << 21))
    }

    /// Leaf PDE with common kernel RW flags.
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

/// Index into the Page Directory (derived from VA bits `[29:21]`).
///
/// Strongly-typed to avoid mixing with other levels. Range is `0..512`
/// (checked in debug builds).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L2Index(u16);

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

impl Default for PdEntry {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl PdEntry {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    #[inline]
    #[must_use]
    pub const fn new_entry(entry: Pde) -> Self {
        Self { entry }
    }

    #[inline]
    #[must_use]
    pub const fn new_leaf(leaf: Pde2M) -> Self {
        Self { leaf_2m: leaf }
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
    /// - If PS=1 → [`L2View::Leaf2M`]
    /// - If PS=0 → [`L2View::Entry`]
    #[inline]
    #[must_use]
    pub const fn view(self) -> L2View {
        unsafe {
            if (self.bits & PS_BIT) != 0 {
                L2View::Leaf2M(self.leaf_2m)
            } else {
                L2View::Entry(self.entry)
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
    /// - When `PS=1`, returns [`PdEntryKind::Leaf2MiB`] with a 2 MiB page base.
    /// - When `PS=0`, returns [`PdEntryKind::NextPageTable`] with a PT base.
    #[inline]
    #[must_use]
    pub const fn kind(self) -> Option<PdEntryKind> {
        if !self.present() {
            return None;
        }

        Some(match self.view() {
            L2View::Entry(entry) => {
                let base = entry.physical_address();
                PdEntryKind::NextPageTable(base, entry)
            }
            L2View::Leaf2M(entry) => {
                let base = entry.physical_page();
                PdEntryKind::Leaf2MiB(base, entry)
            }
        })
    }

    /// Create a non-leaf PDE that points to a Page Table (`PS=0`).
    ///
    /// Sets `present=1`, forces `PS=0`, and writes the PT base address.
    /// The PT base must be 4 KiB-aligned.
    #[must_use]
    pub const fn present_next_with(
        leaf_flags: VirtualMemoryPageBits,
        page: PhysicalPage<Size4K>,
    ) -> Self {
        Self::new_entry(
            leaf_flags
                .to_pde()
                .with_present(true)
                .with_physical_page(page),
        )
    }

    /// Create a new, present [`PtEntry4k`] with the specified flags, at the specified page.
    #[must_use]
    pub const fn present_leaf_with(
        leaf_flags: VirtualMemoryPageBits,
        page: PhysicalPage<Size2M>,
    ) -> Self {
        Self::new_leaf(
            leaf_flags
                .to_pde_2m()
                .with_present(true)
                .with_page_size(true)
                .with_physical_page(page),
        )
    }
}

impl From<Pde> for PdEntry {
    #[inline]
    fn from(e: Pde) -> Self {
        Self::new_entry(e)
    }
}

impl From<Pde2M> for PdEntry {
    #[inline]
    fn from(e: Pde2M) -> Self {
        Self::new_leaf(e)
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
        let e_tbl = PdEntry::present_next_with(Pde::new_common_rw().into(), pt);
        match e_tbl.kind().unwrap() {
            PdEntryKind::NextPageTable(p, f) => {
                assert_eq!(p.base().as_u64(), 0x3000_0000);
                assert_eq!(f.into_bits() & (1 << 7), 0, "must be PS=0");
            }
            _ => panic!("expected next PT"),
        }

        let m2 = PhysicalPage::<Size2M>::from_addr(PhysicalAddress::new(0x4000_0000));
        let e_2m = PdEntry::present_leaf_with(Pde2M::new_common_rw().into(), m2);
        match e_2m.kind().unwrap() {
            PdEntryKind::Leaf2MiB(p, f) => {
                assert_eq!(p.base().as_u64(), 0x4000_0000);
                assert_ne!(f.into_bits() & (1 << 7), 0, "must be PS=1");
            }
            _ => panic!("expected 2MiB leaf"),
        }
    }
}
