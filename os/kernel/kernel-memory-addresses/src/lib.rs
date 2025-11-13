//! # Virtual and Physical Memory Address Types
//!
//! Strongly typed wrappers for raw memory addresses and page bases used in
//! paging and memory management code.
//!
//! ## Overview
//!
//! This module defines a minimal set of types that prevent mixing virtual and
//! physical addresses at compile time while remaining zero-cost wrappers around
//! `u64` values.
//!
//! The core idea is to build all higher-level memory abstractions from a few
//! principal types:
//!
//! | Concept | Generic | Description |
//! |----------|----------|-------------|
//! | [`MemoryAddress`] | – | A raw 64-bit address, either physical or virtual. |
//! | [`MemoryPage<S>`] | [`S: PageSize`](PageSize) | A page-aligned base address of a page of size `S`. |
//! | [`MemoryAddressOffset<S>`] | [`S: PageSize`](PageSize) | An offset within a page of size `S`. |
//!
//! These are then wrapped to distinguish between virtual and physical spaces:
//!
//! | Wrapper | Meaning |
//! |----------|----------|
//! | [`VirtualAddress`] / [`VirtualPage<S>`] | Refer to virtual (page-table translated) memory. |
//! | [`PhysicalAddress`] / [`PhysicalPage<S>`] | Refer to physical memory or MMIO regions. |
//!
//! ## Page Sizes
//!
//! Three standard x86-64 page sizes are supported out of the box via marker
//! types that implement [`PageSize`]:
//!
//! - [`Size4K`] — 4 KiB pages (base granularity)
//! - [`Size2M`] — 2 MiB huge pages
//! - [`Size1G`] — 1 GiB giant pages
//!
//! The [`PageSize`] trait defines constants [`SIZE`](PageSize::SIZE) and
//! [`SHIFT`](PageSize::SHIFT) used throughout the helpers.
//!
//! ## Typical Usage
//!
//! ```rust
//! # use kernel_memory_addresses::*;
//! // Create a virtual address
//! let va = VirtualAddress::new(0xFFFF_FFFF_8000_1234);
//!
//! // Split it into a page base and an in-page offset
//! let (page, off) = va.split::<Size4K>();
//! assert_eq!(page.base().as_u64() & (Size4K::SIZE - 1), 0);
//!
//! // Join them back to the same address
//! assert_eq!(page.join(off).as_u64(), va.as_u64());
//!
//! // Do the same for physical addresses
//! let pa = PhysicalAddress::new(0x0000_0010_2000_0042);
//! let (pp, po) = pa.split::<Size4K>();
//! assert_eq!(pp.join(po).as_u64(), pa.as_u64());
//! ```
//!
//! ## Design Notes
//!
//! - The types are `#[repr(transparent)]` and implement `Copy`, `Eq`, `Ord`, and
//!   `Hash`, making them suitable as map keys or for FFI use.
//! - All alignment and offset calculations are `const fn` and zero-cost in
//!   release builds.
//! - The phantom marker `S` enforces the page size at the type level instead of
//!   using constants, ensuring all conversions are explicit.
//!
//! This forms the foundation for paging, virtual memory mapping, and kernel
//! address-space management code.

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code, clippy::inline_always)]

use core::fmt;
use core::hash::Hash;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign};
use core::ptr::NonNull;

/// Sealed trait pattern to restrict `PageSize` impls to our markers.
mod sealed {
    pub trait Sealed {}
}

/// Marker trait for supported page sizes.
pub trait PageSize:
    sealed::Sealed + Clone + Copy + Eq + PartialEq + Ord + PartialOrd + Hash + fmt::Display + fmt::Debug
{
    /// Page size in bytes (power of two).
    const SIZE: u64;
    /// log2(SIZE), i.e., number of low bits used for the offset.
    const SHIFT: u32;

    fn as_str() -> &'static str;
}

/// 4 KiB page (4096 bytes).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Size4K;
impl sealed::Sealed for Size4K {}
impl PageSize for Size4K {
    const SIZE: u64 = 4096;
    const SHIFT: u32 = 12;

    fn as_str() -> &'static str {
        "4K"
    }
}

/// 2 MiB page (`2_097_152` bytes).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Size2M;
impl sealed::Sealed for Size2M {}
impl PageSize for Size2M {
    const SIZE: u64 = 2 * 1024 * 1024;
    const SHIFT: u32 = 21;

    fn as_str() -> &'static str {
        "2M"
    }
}

/// 1 GiB page (`1_073_741_824` bytes).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Size1G;
impl sealed::Sealed for Size1G {}
impl PageSize for Size1G {
    const SIZE: u64 = 1024 * 1024 * 1024;
    const SHIFT: u32 = 30;

    fn as_str() -> &'static str {
        "1G"
    }
}

impl fmt::Display for Size4K {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(Self::as_str())
    }
}

impl fmt::Display for Size2M {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(Self::as_str())
    }
}

impl fmt::Display for Size1G {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(Self::as_str())
    }
}

impl fmt::Debug for Size4K {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self, f)
    }
}

impl fmt::Debug for Size2M {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self, f)
    }
}

impl fmt::Debug for Size1G {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self, f)
    }
}

/// Principal raw memory address ([virtual](VirtualAddress) or [physical](PhysicalAddress)).
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
        let value = self.align_down::<S>().0;
        MemoryPage {
            value,
            _phantom: PhantomData,
        }
    }

    /// The offset within the page of size `S` that contains this address.
    #[inline]
    #[must_use]
    pub const fn offset<S: PageSize>(self) -> MemoryAddressOffset<S> {
        let value = self.0 & (S::SIZE - 1);
        MemoryAddressOffset {
            value,
            _phantom: PhantomData,
        }
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
    pub fn new(value: u64) -> Self {
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
pub struct VirtualAddress(MemoryAddress);

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

/// Physical memory address.
///
/// A thin wrapper around [`MemoryAddress`] that denotes **physical** addresses
/// (host RAM / MMIO). Like [`VirtualAddress`], this type carries intent and
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
pub struct PhysicalAddress(MemoryAddress);

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
pub struct VirtualPage<S: PageSize>(MemoryPage<S>);

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
pub struct PhysicalPage<S: PageSize>(MemoryPage<S>);

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
        Self(value.value)
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

impl<S> From<MemoryPage<S>> for VirtualPage<S>
where
    S: PageSize,
{
    #[inline]
    fn from(p: MemoryPage<S>) -> Self {
        Self(p)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_join_4k() {
        let a = MemoryAddress::new(0x1234_5678_9ABC_DEF0);
        let (p, o) = a.split::<Size4K>();
        assert_eq!(p.base().as_u64() & 0xFFF, 0);
        assert_eq!(o.as_u64(), a.as_u64() & 0xFFF);
        assert_eq!(p.join(o).as_u64(), a.as_u64());
    }

    #[test]
    fn split_and_join_2m() {
        let a = MemoryAddress::new(0x0000_0008_1234_5678);
        let (p, o) = a.split::<Size2M>();
        assert_eq!(p.base().as_u64() & (Size2M::SIZE - 1), 0);
        assert_eq!(o.as_u64(), a.as_u64() & (Size2M::SIZE - 1));
        assert_eq!(p.join(o).as_u64(), a.as_u64());
    }

    #[test]
    fn split_and_join_1g() {
        let a = MemoryAddress::new(0x0000_0004_1234_5678);
        let (p, o) = a.split::<Size1G>();
        assert_eq!(p.base().as_u64() & (Size1G::SIZE - 1), 0);
        assert_eq!(o.as_u64(), a.as_u64() & (Size1G::SIZE - 1));
        assert_eq!(p.join(o).as_u64(), a.as_u64());
    }

    #[test]
    fn virtual_vs_physical_wrappers() {
        let va = VirtualAddress::new(0xFFFF_FFFF_8000_1234);
        let (vp, vo) = va.split::<Size4K>();
        assert_eq!(vp.base().as_u64() & 0xFFF, 0);
        assert_eq!(vo.as_u64(), 0x1234 & 0xFFF);
        assert_eq!(vp.join(vo).as_u64(), va.as_u64());

        let pa = PhysicalAddress::new(0x0000_0010_2000_0042);
        let (pp, po) = pa.split::<Size4K>();
        assert_eq!(pp.base().as_u64() & 0xFFF, 0);
        assert_eq!(po.as_u64(), 0x42);
        assert_eq!(pp.join(po).as_u64(), pa.as_u64());
    }

    #[test]
    fn alignment_helpers() {
        let a = MemoryAddress::new(0x12345);
        assert_eq!(a.align_down::<Size4K>().as_u64(), 0x12000);
        assert_eq!(a.page::<Size4K>().base().as_u64(), 0x12000);
        assert_eq!(a.offset::<Size4K>().as_u64(), 0x345);
    }
}
