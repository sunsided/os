//! # Page Table

const PAGE_SIZE: u64 = 4096;
const ENTRIES: usize = 512;

#[repr(C, align(4096))]
pub struct PageTable([u64; ENTRIES]);

/// Present bit.
pub const P: u64 = 1 << 0; // present

/// Writable bit.
pub const RW: u64 = 1 << 1; // writable

/// User bit (if present).
pub const US: u64 = 1 << 2; // user (leave 0 for kernel)

/// PWT bit.
pub const PWT: u64 = 1 << 3;

/// PCT bit.
pub const PCD: u64 = 1 << 4;

/// A bit.
pub const A: u64 = 1 << 5;

/// PS bit (2MiB/1GiB page).
pub const PS: u64 = 1 << 7; //

/// If EFER.NXE set (it is on most UEFI)
pub const NX: u64 = 1 << 63;
