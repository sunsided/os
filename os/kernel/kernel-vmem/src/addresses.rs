//! # Virtual and Physical Memory Addresses

use crate::page_table::{PdIndex, PdptIndex, Pml4Index, PtIndex};

/// A memory address as it is used in pointers.
///
/// See [`PhysAddr`] and [`VirtAddr`] for usages.
pub type MemoryAddress = u64;

/// A **physical** memory address (machine bus address).
///
/// Newtype over `u64` to prevent mixing with virtual addresses.
/// No alignment guarantees by itself.
///
/// ### Notes
/// - When used inside page-table entries, the low N bits must be zeroed
///   (N ∈ {12, 21, 30} for 4 KiB/2 MiB/1 GiB).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct PhysAddr(pub MemoryAddress);

/// A **virtual** memory address (process/kernel address space).
///
/// Newtype over `u64` to prevent mixing with physical addresses.
/// No alignment guarantees by itself.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct VirtAddr(pub MemoryAddress);

impl VirtAddr {
    /// Extract the PML4 index (bits 47-39 of the virtual address).
    #[inline]
    pub(crate) const fn pml4_index(self) -> Pml4Index {
        Pml4Index::new(((self.0 >> 39) & 0x1ff) as usize)
    }

    /// Extract the PDPT index (bits 38-30 of the virtual address).
    #[inline]
    pub(crate) const fn pdpt_index(self) -> PdptIndex {
        PdptIndex::new(((self.0 >> 30) & 0x1ff) as usize)
    }

    /// Extract the PD index (bits 29-21 of the virtual address).
    #[inline]
    pub(crate) const fn pd_index(self) -> PdIndex {
        PdIndex::new(((self.0 >> 21) & 0x1ff) as usize)
    }

    /// Extract the PT index (bits 20-12 of the virtual address).
    #[inline]
    pub(crate) const fn pt_index(self) -> PtIndex {
        PtIndex::new(((self.0 >> 12) & 0x1ff) as usize)
    }
}
