use crate::{MemoryAddress, MemoryAddressOffset, PageSize};
use core::fmt;
use core::marker::PhantomData;

/// A page base address (lower `S::SHIFT` bits are zero).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MemoryPage<S: PageSize> {
    value: u64,
    _phantom: PhantomData<S>,
}

impl<S> fmt::Display for MemoryPage<S>
where
    S: PageSize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016X}/{}", self.value, S::as_str())
    }
}

impl<S: PageSize> MemoryPage<S> {
    /// Create from a raw value, aligning down to the page boundary.
    #[inline]
    #[must_use]
    pub const fn from_addr(addr: MemoryAddress) -> Self {
        let value = addr.as_u64() & !(S::SIZE - 1);
        Self {
            value,
            _phantom: PhantomData,
        }
    }

    /// Page that contains `addr` (aligns down).
    #[inline]
    #[must_use]
    pub const fn containing(addr: u64) -> Self {
        let mask = !(S::SIZE - 1);
        Self::from_addr(MemoryAddress::new(addr & mask))
    }

    /// Create from a raw value that must already be aligned.
    /// Panics in debug if unaligned (no runtime cost in release).
    #[inline]
    #[must_use]
    pub fn new_aligned(addr: MemoryAddress) -> Self {
        debug_assert_eq!(addr.as_u64() & (S::SIZE - 1), 0, "unaligned page address");
        let value = addr.as_u64();
        Self {
            value,
            _phantom: PhantomData,
        }
    }

    /// Return the base as `MemoryAddress`.
    #[inline]
    #[must_use]
    pub const fn base(self) -> MemoryAddress {
        MemoryAddress::new(self.value)
    }

    /// Combine with an offset to form a full address.
    #[inline]
    #[must_use]
    pub const fn join(self, off: MemoryAddressOffset<S>) -> MemoryAddress {
        MemoryAddress::new(self.value + off.as_u64())
    }

    /// Checked add of an offset, returning `None` on overflow.
    #[inline]
    #[must_use]
    pub const fn checked_join(self, off: MemoryAddressOffset<S>) -> Option<MemoryAddress> {
        match self.value.checked_add(off.as_u64()) {
            Some(v) => Some(MemoryAddress::new(v)),
            None => None,
        }
    }

    #[inline(always)]
    #[must_use]
    pub(crate) const fn into_inner(self) -> u64 {
        self.value
    }
}

impl<S: PageSize> fmt::Debug for MemoryPage<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemoryPage<{}>(0x{:016X})",
            core::any::type_name::<S>(),
            self.value
        )
    }
}
