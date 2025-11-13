use crate::{MemoryAddressOffset, MemoryPage, PageSize, PhysicalAddress};
use core::fmt;

/// Physical memory page base for size `S`.
///
/// A `PhysicalPage<S>` represents the **page-aligned base** of a physical page
/// of size `S` (`S::SIZE` bytes). It is a thin wrapper over [`MemoryPage<S>`]
/// with physical-address intent.
///
/// ### Semantics
/// - `base()` returns the page base as a [`PhysicalAddress`].
/// - `join(off)` combines this base with a [`MemoryAddressOffset<S>`] to form a
///   full [`PhysicalAddress`].
///
/// ### Invariants
/// - The low `S::SHIFT` bits of the base are always zero (page aligned).
///
/// ### Examples
/// ```rust
/// # use kernel_memory_addresses::*;
/// let pa = PhysicalAddress::new(0x0000_0008_1234_5678);
/// let pp = pa.page::<Size2M>();
/// assert_eq!(pp.base().as_u64() & (Size2M::SIZE - 1), 0);
/// let pa2 = pp.join(pa.offset::<Size2M>());
/// assert_eq!(pa2.as_u64(), pa.as_u64());
/// ```
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PhysicalPage<S: PageSize>(pub(crate) MemoryPage<S>);

impl<S: PageSize> PhysicalPage<S> {
    #[inline]
    #[must_use]
    pub const fn from_addr(p: PhysicalAddress) -> Self {
        Self::from_page(MemoryPage::from_addr(p.0))
    }

    #[inline]
    #[must_use]
    pub const fn from_page(p: MemoryPage<S>) -> Self {
        Self(p)
    }

    #[inline]
    #[must_use]
    pub const fn base(self) -> PhysicalAddress {
        PhysicalAddress(self.0.base())
    }

    #[inline]
    #[must_use]
    pub const fn join(self, off: MemoryAddressOffset<S>) -> PhysicalAddress {
        PhysicalAddress(self.0.join(off))
    }
}

impl<S> fmt::Display for PhysicalPage<S>
where
    S: PageSize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<S: PageSize> fmt::Debug for PhysicalPage<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PhysicalPage<{}>({:#018X})",
            core::any::type_name::<S>(),
            self.0.base().as_u64()
        )
    }
}

impl<S> From<MemoryPage<S>> for PhysicalPage<S>
where
    S: PageSize,
{
    #[inline]
    fn from(p: MemoryPage<S>) -> Self {
        Self(p)
    }
}
