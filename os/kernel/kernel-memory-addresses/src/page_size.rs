use core::fmt;
use core::hash::Hash;

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
