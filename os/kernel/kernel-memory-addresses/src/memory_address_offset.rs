use crate::{MemoryAddress, MemoryPage, PageSize};
use core::fmt;
use core::marker::PhantomData;
use core::ops::Add;

/// The offset within a page of size `S` (`0..S::SIZE-1`).
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MemoryAddressOffset<S: PageSize> {
    value: u64,
    _phantom: PhantomData<S>,
}

impl<S: PageSize> MemoryAddressOffset<S> {
    /// Create from a raw value, asserting it is < `S::SIZE` in debug.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        debug_assert!(value < S::SIZE, "offset must be < page size");
        let value = value & (S::SIZE - 1);
        Self {
            value,
            _phantom: PhantomData,
        }
    }

    /// Construct from a full address's offset bits.
    #[inline]
    #[must_use]
    pub const fn from_addr(addr: MemoryAddress) -> Self {
        let value = addr.as_u64() & (S::SIZE - 1);
        Self {
            value,
            _phantom: PhantomData,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.value
    }
}

impl<S: PageSize> fmt::Debug for MemoryAddressOffset<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Offset<{}>({:#X})",
            core::any::type_name::<S>(),
            self.value
        )
    }
}

impl<S: PageSize> Add<MemoryAddressOffset<S>> for MemoryPage<S> {
    type Output = MemoryAddress;
    #[inline]
    fn add(self, rhs: MemoryAddressOffset<S>) -> Self::Output {
        self.join(rhs)
    }
}

impl<S: PageSize> From<MemoryAddress> for MemoryPage<S> {
    #[inline]
    fn from(addr: MemoryAddress) -> Self {
        Self::from_addr(addr)
    }
}

impl<S: PageSize> From<MemoryAddress> for MemoryAddressOffset<S> {
    #[inline]
    fn from(addr: MemoryAddress) -> Self {
        Self::from_addr(addr)
    }
}
