//! # Virtual and Physical Memory Address Types
//!
//! Strongly typed wrappers for raw memory addresses and page bases used in
//! paging and memory management code.
//!
//! ## Overview
//!
//! This module defines a minimal set of types that prevent mixing virtual and
//! physical addresses at compile time while remaining zero-cost wrappers around
//! `u64` values.
//!
//! The core idea is to build all higher-level memory abstractions from a few
//! principal types:
//!
//! | Concept | Generic | Description |
//! |----------|----------|-------------|
//! | [`MemoryAddress`] | – | A raw 64-bit address, either physical or virtual. |
//! | [`MemoryPage<S>`] | [`S: PageSize`](PageSize) | A page-aligned base address of a page of size `S`. |
//! | [`MemoryAddressOffset<S>`] | [`S: PageSize`](PageSize) | An offset within a page of size `S`. |
//!
//! These are then wrapped to distinguish between virtual and physical spaces:
//!
//! | Wrapper | Meaning |
//! |----------|----------|
//! | [`VirtualAddress`] / [`VirtualPage<S>`] | Refer to virtual (page-table translated) memory. |
//! | [`PhysicalAddress`] / [`PhysicalPage<S>`] | Refer to physical memory or MMIO regions. |
//!
//! ## Page Sizes
//!
//! Three standard x86-64 page sizes are supported out of the box via marker
//! types that implement [`PageSize`]:
//!
//! - [`Size4K`] — 4 KiB pages (base granularity)
//! - [`Size2M`] — 2 MiB huge pages
//! - [`Size1G`] — 1 GiB giant pages
//!
//! The [`PageSize`] trait defines constants [`SIZE`](PageSize::SIZE) and
//! [`SHIFT`](PageSize::SHIFT) used throughout the helpers.
//!
//! ## Typical Usage
//!
//! ```rust
//! # use kernel_memory_addresses::*;
//! // Create a virtual address
//! let va = VirtualAddress::new(0xFFFF_FFFF_8000_1234);
//!
//! // Split it into a page base and an in-page offset
//! let (page, off) = va.split::<Size4K>();
//! assert_eq!(page.base().as_u64() & (Size4K::SIZE - 1), 0);
//!
//! // Join them back to the same address
//! assert_eq!(page.join(off).as_u64(), va.as_u64());
//!
//! // Do the same for physical addresses
//! let pa = PhysicalAddress::new(0x0000_0010_2000_0042);
//! let (pp, po) = pa.split::<Size4K>();
//! assert_eq!(pp.join(po).as_u64(), pa.as_u64());
//! ```
//!
//! ## Design Notes
//!
//! - The types are `#[repr(transparent)]` and implement `Copy`, `Eq`, `Ord`, and
//!   `Hash`, making them suitable as map keys or for FFI use.
//! - All alignment and offset calculations are `const fn` and zero-cost in
//!   release builds.
//! - The phantom marker `S` enforces the page size at the type level instead of
//!   using constants, ensuring all conversions are explicit.
//!
//! This forms the foundation for paging, virtual memory mapping, and kernel
//! address-space management code.

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code, clippy::inline_always)]

mod memory_address;
mod memory_address_offset;
mod memory_page;
mod page_size;
mod physical_address;
mod physical_page;
mod virtual_address;
mod virtual_page;

pub use memory_address::*;
pub use memory_address_offset::MemoryAddressOffset;
pub use memory_page::*;
pub use page_size::*;
pub use physical_address::*;
pub use physical_page::*;
pub use virtual_address::*;
pub use virtual_page::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_join_4k() {
        let a = MemoryAddress::new(0x1234_5678_9ABC_DEF0);
        let (p, o) = a.split::<Size4K>();
        assert_eq!(p.base().as_u64() & 0xFFF, 0);
        assert_eq!(o.as_u64(), a.as_u64() & 0xFFF);
        assert_eq!(p.join(o).as_u64(), a.as_u64());
    }

    #[test]
    fn split_and_join_2m() {
        let a = MemoryAddress::new(0x0000_0008_1234_5678);
        let (p, o) = a.split::<Size2M>();
        assert_eq!(p.base().as_u64() & (Size2M::SIZE - 1), 0);
        assert_eq!(o.as_u64(), a.as_u64() & (Size2M::SIZE - 1));
        assert_eq!(p.join(o).as_u64(), a.as_u64());
    }

    #[test]
    fn split_and_join_1g() {
        let a = MemoryAddress::new(0x0000_0004_1234_5678);
        let (p, o) = a.split::<Size1G>();
        assert_eq!(p.base().as_u64() & (Size1G::SIZE - 1), 0);
        assert_eq!(o.as_u64(), a.as_u64() & (Size1G::SIZE - 1));
        assert_eq!(p.join(o).as_u64(), a.as_u64());
    }

    #[test]
    fn virtual_vs_physical_wrappers() {
        let va = VirtualAddress::new(0xFFFF_FFFF_8000_1234);
        let (vp, vo) = va.split::<Size4K>();
        assert_eq!(vp.base().as_u64() & 0xFFF, 0);
        assert_eq!(vo.as_u64(), 0x1234 & 0xFFF);
        assert_eq!(vp.join(vo).as_u64(), va.as_u64());

        let pa = PhysicalAddress::new(0x0000_0010_2000_0042);
        let (pp, po) = pa.split::<Size4K>();
        assert_eq!(pp.base().as_u64() & 0xFFF, 0);
        assert_eq!(po.as_u64(), 0x42);
        assert_eq!(pp.join(po).as_u64(), pa.as_u64());
    }

    #[test]
    fn alignment_helpers() {
        let a = MemoryAddress::new(0x12345);
        assert_eq!(a.align_down::<Size4K>().as_u64(), 0x12000);
        assert_eq!(a.page::<Size4K>().base().as_u64(), 0x12000);
        assert_eq!(a.offset::<Size4K>().as_u64(), 0x345);
    }
}
