use crate::{MemoryAddress, MemoryAddressOffset, PageSize, VirtualPage};
use core::fmt;
use core::ops::{Add, AddAssign};
use core::ptr::NonNull;

/// Virtual memory address.
///
/// A thin wrapper around [`MemoryAddress`] that denotes **virtual** addresses.
/// It does not validate canonicality at runtime; it only carries the *kind* of
/// address at the type level so you don't accidentally mix virtual and physical
/// values.
///
/// ### Semantics
/// - Use [`VirtualAddress::page`] / [`VirtualAddress::offset`] / [`VirtualAddress::split`]
///   to derive the page base and the in-page offset for a concrete [`PageSize`].
/// - Combine a [`VirtualPage<S>`] and a [`MemoryAddressOffset<S>`] with
///   [`VirtualPage::join`] to reconstruct a `VirtualAddress`.
///
/// ### Invariants
/// - No invariant beyond “this is intended to be a virtual address”.
/// - Alignment is only guaranteed for values returned from `page::<S>()`.
///
/// ### Examples
/// ```rust
/// # use kernel_memory_addresses::*;
/// let va = VirtualAddress::new(0xFFFF_FFFF_8000_1234);
/// let (vp, off) = va.split::<Size4K>();
/// assert_eq!(vp.base().as_u64() & (Size4K::SIZE - 1), 0);
/// assert_eq!(vp.join(off).as_u64(), va.as_u64());
/// ```
#[repr(transparent)]
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VirtualAddress(pub(crate) MemoryAddress);

impl VirtualAddress {
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
    pub const fn new(v: u64) -> Self {
        Self(MemoryAddress::new(v))
    }

    /// Alias for [`new`](Self::new).
    #[inline]
    #[must_use]
    #[doc(hidden)]
    pub const fn from_bits(v: u64) -> Self {
        Self::new(v)
    }

    /// Alias for [`as_u64`](Self::as_u64).
    #[inline]
    #[must_use]
    #[doc(hidden)]
    pub const fn into_bits(self) -> u64 {
        self.as_u64()
    }

    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0)
    }

    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.as_u64()
    }

    #[inline]
    #[must_use]
    pub const fn page<S: PageSize>(self) -> VirtualPage<S> {
        VirtualPage::<S>(self.0.page::<S>())
    }

    #[inline]
    #[must_use]
    pub const fn offset<S: PageSize>(self) -> MemoryAddressOffset<S> {
        self.0.offset::<S>()
    }

    #[inline]
    #[must_use]
    pub const fn split<S: PageSize>(self) -> (VirtualPage<S>, MemoryAddressOffset<S>) {
        (self.page::<S>(), self.offset::<S>())
    }
}

impl fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VA(0x{:016X})", self.as_u64())
    }
}

impl fmt::Display for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016X}", self.as_u64())
    }
}

impl From<u64> for VirtualAddress {
    #[inline]
    fn from(v: u64) -> Self {
        Self::new(v)
    }
}

impl<S> From<VirtualPage<S>> for VirtualAddress
where
    S: PageSize,
{
    fn from(value: VirtualPage<S>) -> Self {
        value.base()
    }
}

impl Add<u64> for VirtualAddress {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<u64> for VirtualAddress {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}
