pub mod pd;
pub mod pdpt;
pub mod pml4;
pub mod pt;

use crate::addr2::VirtualAddress;
use crate::table2::pd::L2Index;
use crate::table2::pdpt::L3Index;
use crate::table2::pml4::L4Index;
use crate::table2::pt::L1Index;

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
