use crate::{MemoryAddressOffset, MemoryPage, PageSize};
use core::fmt;
use core::ops::{Add, AddAssign};
use core::ptr::NonNull;

/// Principal raw memory address ([virtual](super::VirtualAddress) or [physical](super::PhysicalAddress)).
#[repr(transparent)]
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MemoryAddress(u64);

impl MemoryAddress {
    #[inline]
    #[must_use]
    pub const fn from_nonnull<T>(ptr: NonNull<T>) -> Self {
        Self::from_ptr(ptr.as_ptr())
    }

    #[inline]
    #[must_use]
    pub const fn from_ptr<T>(ptr: *const T) -> Self {
        const _: () = assert!(
            size_of::<*const ()>() == size_of::<u64>(),
            "pointer size mismatch"
        );

        // using a union to const-time convert a pointer to an u64
        union Ptr<T> {
            ptr: *const T,
            raw: u64,
        }

        let ptr = Ptr { ptr };
        Self::new(unsafe { ptr.raw })
    }

    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// The page for size `S` that contains this address (lower bits zeroed).
    #[inline]
    #[must_use]
    pub const fn page<S: PageSize>(self) -> MemoryPage<S> {
        MemoryPage::<S>::from_addr(self)
    }

    /// The offset within the page of size `S` that contains this address.
    #[inline]
    #[must_use]
    pub const fn offset<S: PageSize>(self) -> MemoryAddressOffset<S> {
        let value = self.0 & (S::SIZE - 1);
        MemoryAddressOffset::new(value)
    }

    /// Split into (`MemoryPage<S>`, `MemoryAddressOffset<S>`).
    #[inline]
    #[must_use]
    pub const fn split<S: PageSize>(self) -> (MemoryPage<S>, MemoryAddressOffset<S>) {
        (self.page::<S>(), self.offset::<S>())
    }

    /// Align down to page boundary `S`.
    #[inline]
    #[must_use]
    pub const fn align_down<S: PageSize>(self) -> Self {
        Self(self.0 & !(S::SIZE - 1))
    }
}

impl fmt::Debug for MemoryAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 0xHHHH_HHHH_HHHH_HHHH style
        write!(f, "MemoryAddress(0x{:016X})", self.0)
    }
}

impl fmt::Display for MemoryAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016X}", self.as_u64())
    }
}

impl From<u64> for MemoryAddress {
    #[inline]
    fn from(v: u64) -> Self {
        Self::new(v)
    }
}

impl From<MemoryAddress> for u64 {
    #[inline]
    fn from(a: MemoryAddress) -> Self {
        a.as_u64()
    }
}

impl<S> From<MemoryPage<S>> for MemoryAddress
where
    S: PageSize,
{
    fn from(value: MemoryPage<S>) -> Self {
        Self(value.into_inner())
    }
}

impl Add<u64> for MemoryAddress {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<u64> for MemoryAddress {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}
