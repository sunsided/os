//! Page Table Entries

use bitfield_struct::bitfield;

/// A 4-level x86-64 **page table page** (512 `u64` entries), 4 KiB aligned.
///
/// This wrapper provides a concrete layout for manipulating entries
/// after mapping the table via `PhysMapper`.
#[repr(align(4096))]
pub struct PageTable([PageTableEntry; 512]);

impl PageTable {
    /// Returns a raw mutable pointer to the first entry.
    ///
    /// Useful when passing to low-level routines or assembly.
    #[inline]
    pub const fn as_ptr(&mut self) -> *mut PageTableEntry {
        self.0.as_mut_ptr()
    }

    /// Zeros all 512 entries.
    ///
    /// Commonly used right after allocating a fresh 4 KiB frame to turn it
    /// into a page table page.
    #[inline]
    pub fn zero(&mut self) {
        for e in &mut self.0 {
            *e = PageTableEntry::default();
        }
    }

    /// Internal helper: get a mutable entry reference by index.
    ///
    /// # Safety
    /// `PageTable` exposes only `as_ptr()`; we provide a small, contained unsafe.
    #[inline]
    pub unsafe fn entry<I>(&mut self, idx: I) -> &PageTableEntry
    where
        I: Into<usize>,
    {
        let idx = idx.into();
        debug_assert!(idx < 512);
        unsafe { &(*self.as_ptr().add(idx)) }
    }

    /// Internal helper: get a mutable entry reference by index.
    ///
    /// # Safety
    /// `PageTable` exposes only `as_ptr()`; we provide a small, contained unsafe.
    #[inline]
    pub unsafe fn entry_mut<I>(&mut self, idx: I) -> &mut PageTableEntry
    where
        I: Into<usize>,
    {
        let idx = idx.into();
        debug_assert!(idx < 512);
        unsafe { &mut *self.as_ptr().add(idx) }
    }
}

/// A 64-bit `x86_64` page-table entry.
///
/// This structure covers any level (PML4E, PDPTE, PDE, or PTE).
/// Some bits are meaningful only at certain levels (e.g. `ps` is used only in PDE/PDPTE).
///
/// Layout reference (Intel SDM Vol. 3A §4.5):
///
/// ```text
/// 63              52 51                     12 11        0
/// +----------------+-------------------------+-----------+
/// | NX | ignored   |   phys addr bits[51:12] | flags     |
/// +----------------+-------------------------+-----------+
/// ```
///
/// `bitfield` lays out fields from least-significant to most-significant bit.
#[bitfield(u64)]
pub struct PageTableEntry {
    // --- low 12 bits: flags ---
    /// Page is present in memory (required for valid mapping).
    pub present: bool, // bit 0

    /// Page is writable.
    pub writable: bool, // bit 1

    /// Page accessible from user mode (CPL=3).
    pub user: bool, // bit 2

    /// Write-through caching enabled.
    pub write_through: bool, // bit 3

    /// Caching disabled.
    pub cache_disable: bool, // bit 4

    /// Page has been accessed.
    pub accessed: bool, // bit 5

    /// Page has been written to (dirty).
    pub dirty: bool, // bit 6

    /// Page size flag (only for PDE/PDPTE).
    pub ps: bool, // bit 7

    /// Global page flag.
    pub global: bool, // bit 8

    /// Ignored by hardware; available for software use.
    #[bits(3)]
    pub avail_lo: u8, // bits 9–11

    // --- middle bits: physical address ---
    /// Bits 12–51 of the physical address of the next-level table or mapped page.
    #[bits(40)]
    pub phys_addr_hi: u64, // bits 12–51

    // --- high bits ---
    /// Ignored by hardware; available for software use.
    #[bits(11)]
    pub avail_hi: u16, // bits 52–62

    /// No-execute flag (requires EFER.NXE=1).
    pub nx: bool, // bit 63
}

impl PageTableEntry {
    /// Returns the 52-bit physical address (shifted left by 12 bits).
    #[must_use]
    pub const fn addr(&self) -> u64 {
        self.phys_addr_hi() << 12
    }

    /// Sets the physical address (must be 4 KiB aligned).
    pub fn set_addr(&mut self, addr: u64) {
        debug_assert_eq!(addr & 0xfff, 0, "address must be 4 KiB aligned");
        self.set_phys_addr_hi(addr >> 12);
    }

    /// Returns `true` if this entry is present and not a huge page pointer.
    #[must_use]
    pub const fn is_present_leaf(&self) -> bool {
        self.present() && !self.ps()
    }
}
