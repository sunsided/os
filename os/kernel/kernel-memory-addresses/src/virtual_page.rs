use crate::{MemoryAddressOffset, MemoryPage, PageSize, VirtualAddress};
use core::fmt;

/// Virtual memory page base for size `S`.
///
/// A `VirtualPage<S>` represents the **page-aligned base** of a virtual page of
/// size `S` (`S::SIZE` bytes). It is a thin wrapper over [`MemoryPage<S>`] with
/// virtual-address intent.
///
/// ### Semantics
/// - `base()` returns the page base as a [`VirtualAddress`].
/// - `join(off)` combines this base with a [`MemoryAddressOffset<S>`] to form a
///   full [`VirtualAddress`].
///
/// ### Invariants
/// - The low `S::SHIFT` bits of the base are always zero (page aligned).
///
/// ### Examples
/// ```rust
/// # use kernel_memory_addresses::*;
/// let va = VirtualAddress::new(0xFFFF_FFFF_8000_1234);
/// let vp = va.page::<Size4K>();
/// assert_eq!(vp.base().as_u64() & (Size4K::SIZE - 1), 0);
/// let va2 = vp.join(va.offset::<Size4K>());
/// assert_eq!(va2.as_u64(), va.as_u64());
/// ```
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VirtualPage<S: PageSize>(pub(crate) MemoryPage<S>);

impl<S: PageSize> VirtualPage<S> {
    #[inline]
    #[must_use]
    pub const fn from_page(p: MemoryPage<S>) -> Self {
        Self(p)
    }

    /// Page that contains `addr` (aligns down to page boundary).
    #[inline]
    #[must_use]
    pub const fn containing_address(addr: VirtualAddress) -> Self {
        Self(MemoryPage::<S>::containing(addr.as_u64()))
    }

    #[inline]
    #[must_use]
    pub const fn base(self) -> VirtualAddress {
        VirtualAddress(self.0.base())
    }

    #[inline]
    #[must_use]
    pub const fn join(self, off: MemoryAddressOffset<S>) -> VirtualAddress {
        VirtualAddress(self.0.join(off))
    }
}

impl<S> fmt::Display for VirtualPage<S>
where
    S: PageSize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<S: PageSize> fmt::Debug for VirtualPage<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VirtualPage<{}>({:#018X})",
            core::any::type_name::<S>(),
            self.0.base().as_u64()
        )
    }
}

impl<S: PageSize> TryFrom<VirtualAddress> for VirtualPage<S> {
    type Error = ();

    #[inline]
    fn try_from(va: VirtualAddress) -> Result<Self, ()> {
        if (va.as_u64() & (S::SIZE - 1)) == 0 {
            Ok(va.page())
        } else {
            Err(())
        }
    }
}

impl<S> From<MemoryPage<S>> for VirtualPage<S>
where
    S: PageSize,
{
    #[inline]
    fn from(p: MemoryPage<S>) -> Self {
        Self(p)
    }
}
