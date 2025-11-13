use crate::{MemoryAddress, MemoryAddressOffset, PageSize, PhysicalPage};
use core::fmt;
use core::ops::{Add, AddAssign};
use core::ptr::NonNull;

/// Physical memory address.
///
/// A thin wrapper around [`MemoryAddress`] that denotes **physical** addresses
/// (host RAM / MMIO). Like [`VirtualAddress`](super::VirtualAddress), this type carries intent and
/// prevents accidental VA↔PA mix-ups.
///
/// ### Semantics
/// - Use [`PhysicalAddress::page`] / [`PhysicalAddress::offset`] / [`PhysicalAddress::split`]
///   to derive the page base and in-page offset for a concrete [`PageSize`].
/// - Combine a [`PhysicalPage<S>`] with a [`MemoryAddressOffset<S>`] using
///   [`PhysicalPage::join`] to reconstruct the original `PhysicalAddress`.
///
/// ### Notes
/// - Page-table entries often store a **page-aligned** physical base (low
///   `S::SHIFT` bits cleared) plus per-entry flag bits; use `split::<S>()` to
///   reason about base vs. offset explicitly.
///
/// ### Examples
/// ```rust
/// # use kernel_memory_addresses::*;
/// let pa = PhysicalAddress::new(0x0000_0010_2000_0042);
/// let (pp, off) = pa.split::<Size4K>();
/// assert_eq!(pp.base().as_u64() & (Size4K::SIZE - 1), 0);
/// assert_eq!(pp.join(off).as_u64(), pa.as_u64());
/// ```
#[repr(transparent)]
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PhysicalAddress(pub(crate) MemoryAddress);

impl PhysicalAddress {
    #[inline]
    #[must_use]
    pub const fn from_nonnull<T>(ptr: NonNull<T>) -> Self {
        Self::from_ptr(ptr.as_ptr())
    }

    #[inline]
    #[must_use]
    pub const fn from_ptr<T>(ptr: *const T) -> Self {
        Self(MemoryAddress::from_ptr(ptr))
    }

    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0)
    }

    #[inline]
    #[must_use]
    pub const fn new(v: u64) -> Self {
        Self(MemoryAddress::new(v))
    }

    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.as_u64()
    }

    #[inline]
    #[must_use]
    pub const fn page<S: PageSize>(self) -> PhysicalPage<S> {
        PhysicalPage::<S>(self.0.page::<S>())
    }

    #[inline]
    #[must_use]
    pub const fn offset<S: PageSize>(self) -> MemoryAddressOffset<S> {
        self.0.offset::<S>()
    }

    #[inline]
    #[must_use]
    pub const fn split<S: PageSize>(self) -> (PhysicalPage<S>, MemoryAddressOffset<S>) {
        (self.page::<S>(), self.offset::<S>())
    }
}

impl fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PA(0x{:016X})", self.as_u64())
    }
}

impl fmt::Display for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016X}", self.as_u64())
    }
}

impl From<u64> for PhysicalAddress {
    #[inline]
    fn from(v: u64) -> Self {
        Self::new(v)
    }
}

impl<S> From<PhysicalPage<S>> for PhysicalAddress
where
    S: PageSize,
{
    fn from(value: PhysicalPage<S>) -> Self {
        value.base()
    }
}

impl Add<u64> for PhysicalAddress {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<u64> for PhysicalAddress {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}
