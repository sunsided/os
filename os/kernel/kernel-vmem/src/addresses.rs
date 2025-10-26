//! # Virtual and Physical Memory Addresses

use crate::page_table::{PdIndex, PdptIndex, Pml4Index, PtIndex};
use core::ops::{Add, AddAssign, Deref, Sub};

/// A memory address as it is used in pointers.
///
/// See [`PhysAddr`] and [`VirtAddr`] for usages.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct MemoryAddress(pub u64);

/// A **physical** memory address (machine bus address).
///
/// Newtype over `u64` to prevent mixing with virtual addresses.
/// No alignment guarantees by itself.
///
/// ### Notes
/// - When used inside page-table entries, the low N bits must be zeroed
///   (N ∈ {12, 21, 30} for 4 KiB/2 MiB/1 GiB).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhysAddr(pub MemoryAddress);

/// The high bits of a physical address.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhysAddrHi(pub MemoryAddress);

/// A **virtual** memory address (process/kernel address space).
///
/// Newtype over `u64` to prevent mixing with physical addresses.
/// No alignment guarantees by itself.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct VirtAddr(pub MemoryAddress);

impl VirtAddr {
    /// Extract the PML4 index (bits 47-39 of the virtual address).
    #[inline]
    pub(crate) const fn pml4_index(self) -> Pml4Index {
        Pml4Index::new(((self.as_u64() >> 39) & 0x1ff) as usize)
    }

    /// Extract the PDPT index (bits 38-30 of the virtual address).
    #[inline]
    pub(crate) const fn pdpt_index(self) -> PdptIndex {
        PdptIndex::new(((self.as_u64() >> 30) & 0x1ff) as usize)
    }

    /// Extract the PD index (bits 29-21 of the virtual address).
    #[inline]
    pub(crate) const fn pd_index(self) -> PdIndex {
        PdIndex::new(((self.as_u64() >> 21) & 0x1ff) as usize)
    }

    /// Extract the PT index (bits 20-12 of the virtual address).
    #[inline]
    pub(crate) const fn pt_index(self) -> PtIndex {
        PtIndex::new(((self.as_u64() >> 12) & 0x1ff) as usize)
    }
}

impl MemoryAddress {
    #[must_use]
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    #[must_use]
    pub fn from_ptr(ptr: *const u8) -> Self {
        Self(ptr as u64)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl VirtAddr {
    #[must_use]
    pub const fn new(addr: MemoryAddress) -> Self {
        Self(addr)
    }

    #[must_use]
    pub const fn from_u64(addr: u64) -> Self {
        Self(MemoryAddress::new(addr))
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.as_u64()
    }

    #[must_use]
    pub const fn as_addr(self) -> MemoryAddress {
        self.0
    }
}

impl PhysAddr {
    #[must_use]
    pub const fn new(addr: MemoryAddress) -> Self {
        Self(addr)
    }

    #[must_use]
    pub const fn from_u64(addr: u64) -> Self {
        Self(MemoryAddress::new(addr))
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.as_u64()
    }

    #[must_use]
    pub const fn as_addr(self) -> MemoryAddress {
        self.0
    }
}

impl PhysAddrHi {
    pub const fn new(addr: MemoryAddress) -> Self {
        Self(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0.as_u64()
    }

    pub const fn as_addr(self) -> MemoryAddress {
        self.0
    }
}

impl PartialEq<VirtAddr> for PhysAddr {
    fn eq(&self, other: &VirtAddr) -> bool {
        self.as_u64() == other.as_u64()
    }
}

impl PartialEq<PhysAddr> for VirtAddr {
    fn eq(&self, other: &PhysAddr) -> bool {
        other.eq(self)
    }
}

impl PartialEq<MemoryAddress> for VirtAddr {
    fn eq(&self, other: &MemoryAddress) -> bool {
        self.as_u64() == other.as_u64()
    }
}

impl PartialEq<MemoryAddress> for PhysAddr {
    fn eq(&self, other: &MemoryAddress) -> bool {
        self.as_u64() == other.as_u64()
    }
}

impl core::fmt::Display for MemoryAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:018x}", self.0)
    }
}

impl core::fmt::Debug for MemoryAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:018x} (@{} MiB)", self.0, self.0 / 1024 / 1024)
    }
}

impl core::fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, f)
    }
}

impl core::fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "0x{:018x} (Virtual @{} MiB)",
            self.0.0,
            self.0.0 / 1024 / 1024
        )
    }
}

impl core::fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, f)
    }
}

impl core::fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "0x{:018x} (Physical @{} MiB)",
            self.0.0,
            self.0.0 / 1024 / 1024
        )
    }
}

impl core::fmt::Display for PhysAddrHi {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, f)
    }
}

impl core::fmt::Debug for PhysAddrHi {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "0x{:018x} (Physical High Bit @{} MiB)",
            self.0.0,
            self.0.0 / 1024 / 1024
        )
    }
}

impl Deref for MemoryAddress {
    type Target = u64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for VirtAddr {
    type Target = MemoryAddress;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for PhysAddr {
    type Target = MemoryAddress;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Add<u64> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self::from_u64(self.0.checked_add(rhs).expect("VirtAddr add"))
    }
}

impl Add<u64> for PhysAddr {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self::from_u64(self.0.checked_add(rhs).expect("PhysAddr add"))
    }
}

impl Add<u64> for MemoryAddress {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self::new(self.0.checked_add(rhs).expect("PhysAddr add"))
    }
}

impl AddAssign<u64> for VirtAddr {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl AddAssign<u64> for PhysAddr {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl AddAssign<u64> for MemoryAddress {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl PartialEq<u64> for VirtAddr {
    fn eq(&self, other: &u64) -> bool {
        self.as_u64() == *other
    }
}

impl PartialEq<u64> for PhysAddr {
    fn eq(&self, other: &u64) -> bool {
        self.as_u64() == *other
    }
}

impl PartialEq<u64> for PhysAddrHi {
    fn eq(&self, other: &u64) -> bool {
        self.as_u64() == *other
    }
}

impl PartialEq<u64> for MemoryAddress {
    fn eq(&self, other: &u64) -> bool {
        self.as_u64() == *other
    }
}

impl From<u64> for VirtAddr {
    fn from(addr: u64) -> Self {
        Self::new(MemoryAddress::new(addr))
    }
}

impl From<u64> for PhysAddr {
    fn from(addr: u64) -> Self {
        Self::new(MemoryAddress::new(addr))
    }
}

impl From<u64> for PhysAddrHi {
    fn from(addr: u64) -> Self {
        Self::new(MemoryAddress::new(addr))
    }
}

impl From<u64> for MemoryAddress {
    fn from(addr: u64) -> Self {
        Self::new(addr)
    }
}

impl From<MemoryAddress> for VirtAddr {
    fn from(addr: MemoryAddress) -> Self {
        Self::new(addr)
    }
}

impl From<MemoryAddress> for PhysAddr {
    fn from(addr: MemoryAddress) -> Self {
        Self::new(addr)
    }
}

impl Sub<Self> for MemoryAddress {
    type Output = u64;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.checked_sub(rhs.as_u64()).expect("MemoryAddress sub")
    }
}
