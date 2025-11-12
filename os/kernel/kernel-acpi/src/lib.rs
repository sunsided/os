//! # System ACPI Support

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code)]

pub mod rsdp;

/// Map a physical region and return a *read-only* byte slice for its contents.
/// You provide the implementation (identity map, kmap, etc.).
pub trait PhysMapRo {
    /// # Safety
    /// The implementor must ensure the returned slice is valid for `len` bytes.
    unsafe fn map_ro<'a>(&self, paddr: u64, len: usize) -> &'a [u8];
}

fn sum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |a, &b| a.wrapping_add(b))
}
