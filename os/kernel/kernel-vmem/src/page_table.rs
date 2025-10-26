//! Page Table Entries

use crate::MemoryPageFlags;
use crate::addresses::{PhysAddr, VirtAddr};
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

/// Implement common functionality for page tables.
macro_rules! table_common {
    ($name:tt, $T:ident, $Index:ident) => {
        #[doc = concat!("The `", stringify!($name), "` page table.")]
        #[repr(transparent)]
        pub struct $T(PageTable);

        #[doc = concat!("An index into the `", stringify!($name), "` page table.")]
        #[derive(Copy, Clone)]
        #[repr(transparent)]
        pub struct $Index(usize);

        impl $Index {
            #[inline]
            pub(crate) const fn new(idx: usize) -> Self {
                Self(idx)
            }
        }

        impl $T {
            #[inline]
            pub fn zero(&mut self) {
                self.0.zero()
            }

            #[inline]
            pub const fn as_page_table_mut(&mut self) -> &mut PageTable {
                &mut self.0
            }

            /// Get a mutable entry reference by index.
            ///
            /// `PageTable` exposes only `as_ptr()`; we provide a small, contained unsafe.
            #[inline]
            unsafe fn entry_mut(&mut self, idx: $Index) -> &mut PageTableEntry {
                debug_assert!(idx.0 < 512);
                unsafe { &mut *self.0.as_ptr().add(idx.0) }
            }
        }

        impl AsMut<PageTable> for $T {
            fn as_mut(&mut self) -> &mut PageTable {
                &mut self.0
            }
        }

        impl From<PageTable> for $T {
            fn from(pt: PageTable) -> Self {
                Self(pt)
            }
        }

        impl From<$Index> for usize {
            fn from(i: $Index) -> Self {
                i.0
            }
        }
    };
}

table_common!(PML4, Pml4PageTable, Pml4Index);
table_common!(PDPT, PdptPageTable, PdptIndex);
table_common!(PD, PdPageTable, PdIndex);
table_common!(PT, PtPageTable, PtIndex);

impl Pml4PageTable {
    #[inline]
    pub fn entry_mut_by_va(&mut self, va: VirtAddr) -> &mut PageTableEntry {
        unsafe { self.entry_mut(va.pml4_index()) }
    }

    /// Initialize a non-leaf entry to point to a PDPT table.
    pub fn link_pdpt(&mut self, va: VirtAddr, pdpt_phys: PhysAddr) {
        let e = self.entry_mut_by_va(va);
        e.set_present(true);
        e.set_writable(true);
        e.set_ps(false);
        e.set_addr(pdpt_phys.0);
    }
}

impl PdptPageTable {
    #[inline]
    pub fn entry_mut_by_va(&mut self, va: VirtAddr) -> &mut PageTableEntry {
        unsafe { self.entry_mut(va.pdpt_index()) }
    }

    /// Non-leaf: link to a PD table.
    pub fn link_pd(&mut self, va: VirtAddr, pd_phys: PhysAddr) {
        let e = self.entry_mut_by_va(va);
        e.set_present(true);
        e.set_writable(true);
        e.set_ps(false);
        e.set_addr(pd_phys.0);
    }

    /// **Leaf (1 GiB):** set PDPTE as 1 GiB mapping.
    pub fn map_1g_leaf(&mut self, va: VirtAddr, pa: PhysAddr, flags: MemoryPageFlags) {
        let e = self.entry_mut_by_va(va);
        e.set_addr(pa.0);
        e.set_present(true);
        e.set_ps(true);
        e.set_writable(flags.contains(MemoryPageFlags::WRITABLE));
        e.set_user(flags.contains(MemoryPageFlags::USER));
        e.set_write_through(flags.contains(MemoryPageFlags::WT));
        e.set_cache_disable(flags.contains(MemoryPageFlags::CD));
        e.set_global(flags.contains(MemoryPageFlags::GLOBAL));
        e.set_nx(flags.contains(MemoryPageFlags::NX));
    }
}

impl PdPageTable {
    #[inline]
    pub fn entry_mut_by_va(&mut self, va: VirtAddr) -> &mut PageTableEntry {
        unsafe { self.entry_mut(va.pd_index()) }
    }

    /// Non-leaf: link to a PT table.
    pub fn link_pt(&mut self, va: VirtAddr, pt_phys: PhysAddr) {
        let e = self.entry_mut_by_va(va);
        e.set_present(true);
        e.set_writable(true);
        e.set_ps(false);
        e.set_addr(pt_phys.0);
    }

    /// **Leaf (2 MiB):** set PDE as 2 MiB mapping.
    pub fn map_2m_leaf(&mut self, va: VirtAddr, pa: PhysAddr, flags: MemoryPageFlags) {
        let e = self.entry_mut_by_va(va);
        e.set_addr(pa.0);
        e.set_present(true);
        e.set_ps(true);
        e.set_writable(flags.contains(MemoryPageFlags::WRITABLE));
        e.set_user(flags.contains(MemoryPageFlags::USER));
        e.set_write_through(flags.contains(MemoryPageFlags::WT));
        e.set_cache_disable(flags.contains(MemoryPageFlags::CD));
        e.set_global(flags.contains(MemoryPageFlags::GLOBAL));
        e.set_nx(flags.contains(MemoryPageFlags::NX));
    }
}

impl PtPageTable {
    #[inline]
    pub fn entry_mut_by_va(&mut self, va: VirtAddr) -> &mut PageTableEntry {
        unsafe { self.entry_mut(va.pt_index()) }
    }

    /// **Leaf (4 KiB):** set PTE as 4 KiB mapping (no PS).
    pub fn map_4k_leaf(&mut self, va: VirtAddr, pa: PhysAddr, flags: MemoryPageFlags) {
        let e = self.entry_mut_by_va(va);
        e.set_addr(pa.0);
        e.set_present(true);
        e.set_ps(false);
        e.set_writable(flags.contains(MemoryPageFlags::WRITABLE));
        e.set_user(flags.contains(MemoryPageFlags::USER));
        e.set_write_through(flags.contains(MemoryPageFlags::WT));
        e.set_cache_disable(flags.contains(MemoryPageFlags::CD));
        e.set_global(flags.contains(MemoryPageFlags::GLOBAL));
        e.set_nx(flags.contains(MemoryPageFlags::NX));
    }
}
