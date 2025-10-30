//! # Memory Page Table

mod bits2;
pub mod pd;
pub mod pdpt;
pub mod pml4;
pub mod pt;
pub mod unified2;

use crate::addresses::VirtualAddress;
use crate::page_table::pd::L2Index;
use crate::page_table::pdpt::L3Index;
use crate::page_table::pml4::L4Index;
use crate::page_table::pt::L1Index;

#[inline]
#[must_use]
pub const fn split_indices(va: VirtualAddress) -> (L4Index, L3Index, L2Index, L1Index) {
    (
        L4Index::from(va),
        L3Index::from(va),
        L2Index::from(va),
        L1Index::from(va),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_ok() {
        let va = VirtualAddress::new(0xFFFF_8888_0123_4567);
        let (i4, i3, i2, i1) = split_indices(va);
        assert!(i4.as_usize() < 512);
        assert!(i3.as_usize() < 512);
        assert!(i2.as_usize() < 512);
        assert!(i1.as_usize() < 512);
    }
}
