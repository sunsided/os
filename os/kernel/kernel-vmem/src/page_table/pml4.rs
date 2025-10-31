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

use crate::VirtualMemoryPageBits;
use crate::addresses::{PhysicalAddress, PhysicalPage, Size4K, VirtualAddress};
use bitfield_struct::bitfield;

/// L4 **PML4E** — pointer to a **PDPT** (non-leaf; PS **must be 0**).
///
/// This entry never maps memory directly. Bits that are meaningful only on
/// leaf entries (e.g., `dirty`, `global`) are ignored here.
///
/// - Physical address (bits **51:12**) is a 4 KiB-aligned PDPT.
/// - `NX` participates in permission intersection across the walk.
/// - `PKU` may be repurposed as OS-available when not supported.
///
/// Reference: AMD APM / Intel SDM paging structures (x86-64).
#[doc(alias = "PML4E")]
#[bitfield(u64)]
pub struct Pml4Entry {
    /// **Present** (bit 0): valid entry if set.
    ///
    /// When clear, the entry is not present and most other fields are ignored.
    pub present: bool,

    /// **Writable** (bit 1): write permission.
    ///
    /// Intersects with lower-level permissions; supervisor write protection,
    /// SMEP/SMAP, CR0.WP, and U/S checks apply.
    pub writable: bool,

    /// **User/Supervisor** (bit 2): allow user-mode access if set.
    ///
    /// If clear, access is restricted to supervisor (ring 0).
    pub user: bool,

    /// **Page Write-Through** (PWT, bit 3): write-through caching policy.
    ///
    /// Effective only if caching isn’t disabled for the mapping.
    pub write_through: bool,

    /// **Page Cache Disable** (PCD, bit 4): disable caching if set.
    ///
    /// Strongly impacts performance; use for MMIO or compliance with device
    /// requirements. Effective policy is the intersection across the walk.
    pub cache_disable: bool,

    /// **Accessed** (A, bit 5): set by CPU on first access via this entry.
    ///
    /// Software may clear to track usage; not a permission bit.
    pub accessed: bool,

    /// (bit 6): **ignored** for non-leaf entries at L4.
    #[bits(1)]
    __d_ignored: u8,

    /// **Page Size** (bit 7): **must be 0** for PML4E (non-leaf).
    #[bits(1)]
    __ps_must_be_0: u8,

    /// **Global** (bit 8): **ignored** for non-leaf entries.
    #[bits(1)]
    __g_ignored: u8,

    /// **OS-available low** (bits 9..11): not interpreted by hardware.
    #[bits(3)]
    pub os_available_low: u8,

    /// **Next-level table physical address** (bits 12..51).
    ///
    /// Stores the PDPT base (4 KiB-aligned). The low 12 bits are omitted.
    #[bits(40)]
    phys_addr_51_12: u64,

    /// **OS-available high** (bits 52..58): not interpreted by hardware.
    #[bits(7)]
    pub os_available_high: u8,

    /// **Protection Key / OS use** (bits 59..62).
    ///
    /// If PKU is supported and enabled, these bits select the protection key;
    /// otherwise they may be used by the OS.
    #[bits(4)]
    pub protection_key: u8,

    /// **No-Execute** (NX, bit 63 / XD on Intel).
    ///
    /// When set and EFER.NXE is enabled, instruction fetch is disallowed
    /// through this entry (permission intersection applies).
    pub no_execute: bool,
}

/// Index into the PML4 table (derived from virtual-address bits `[47:39]`).
///
/// This newtype prevents accidental mixing with other indices and allows
/// compile-time checking of valid index ranges (0..512).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L4Index(u16);

/// The top-level page map (PML4).
///
/// Layout:
/// - 512 entries, 8 bytes each (4096 bytes total).
/// - 4 KiB aligned, as required by the hardware.
///
/// Use [`PageMapLevel4::get`] and [`PageMapLevel4::set`] to read/write entries.
#[doc(alias = "PML4")]
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
        Self::new()
    }

    /// If present, return the physical page of the next-level PDPT.
    ///
    /// Returns `None` if the entry is not present. The returned page is always
    /// 4 KiB-aligned as required for page-table bases.
    #[inline]
    #[must_use]
    pub const fn next_table(self) -> Option<PhysicalPage<Size4K>> {
        if !self.present() {
            return None;
        }
        Some(self.physical_address())
    }

    /// Build a PML4 entry that points to the given PDPT page and applies the provided flags.
    ///
    /// ### Requirements
    /// - `flags.large_page()` **must be false** (`PS=0`). Enforced via `debug_assert!`.
    /// - This function sets `present=1` and the physical base to `next_pdpt_page.base()`.
    #[inline]
    #[must_use]
    pub const fn present_with(
        flags: VirtualMemoryPageBits,
        next_pdpt_page: PhysicalPage<Size4K>,
    ) -> Self {
        flags
            .to_pml4e()
            .with_present(true)
            .with_physical_address(next_pdpt_page)
    }

    /// Set the PDPT base address (must be 4 KiB-aligned).
    #[inline]
    #[must_use]
    pub const fn with_physical_address(mut self, phys: PhysicalPage<Size4K>) -> Self {
        self.set_physical_address(phys);
        self
    }

    /// Set the PDPT base address (must be 4 KiB-aligned).
    #[inline]
    pub const fn set_physical_address(&mut self, phys: PhysicalPage<Size4K>) {
        self.set_phys_addr_51_12(phys.base().as_u64() >> 12);
    }

    /// Get the PDPT base address (4 KiB-aligned).
    #[inline]
    #[must_use]
    pub const fn physical_address(self) -> PhysicalPage<Size4K> {
        PhysicalPage::from_addr(PhysicalAddress::new(self.phys_addr_51_12() << 12))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addresses::PhysicalAddress;

    #[test]
    fn pml4_points_to_pdpt() {
        let pdpt_page = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x1234_5000));
        let f = Pml4Entry::new().with_writable(true).with_user(false);
        let e = Pml4Entry::present_with(f.into(), pdpt_page);
        assert!(e.present());
        assert_eq!(e.next_table().unwrap().base().as_u64(), 0x1234_5000);
    }
}
