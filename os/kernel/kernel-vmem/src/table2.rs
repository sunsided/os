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
    use crate::PageEntryBits;
    use crate::addr2::{PhysicalAddress, PhysicalPage, Size1G, Size2M, Size4K};
    use crate::table2::pd::{PdEntry, PdEntryKind};
    use crate::table2::pdpt::{PdptEntry, PdptEntryKind};
    use crate::table2::pml4::Pml4Entry;
    use crate::table2::pt::PtEntry;

    #[test]
    fn indices_ok() {
        let va = VirtualAddress::new(0xFFFF_8888_0123_4567);
        let (i4, i3, i2, i1) = split_indices(va);
        assert!(i4.as_usize() < 512);
        assert!(i3.as_usize() < 512);
        assert!(i2.as_usize() < 512);
        assert!(i1.as_usize() < 512);
    }

    #[test]
    fn pml4_points_to_pdpt() {
        let pdpt_page = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x1234_5000));
        let mut f = PageEntryBits::new();
        f.set_writable(true);
        f.set_user_access(false);
        let e = Pml4Entry::make(pdpt_page, f);
        assert!(e.is_present());
        assert!(!e.flags().large_page());
        assert_eq!(e.next_table().unwrap().base().as_u64(), 0x1234_5000);
    }

    #[test]
    fn pdpt_table_vs_1g() {
        // next-level PD
        let pd = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x2000_0000));
        let e_tbl = PdptEntry::make_next(pd, PageEntryBits::new_common_rw());
        match e_tbl.kind().unwrap() {
            PdptEntryKind::NextPageDirectory(p, f) => {
                assert_eq!(p.base().as_u64(), 0x2000_0000);
                assert!(!f.large_page());
            }
            _ => panic!("expected next PD"),
        }

        // 1 GiB leaf
        let g1 = PhysicalPage::<Size1G>::from_addr(PhysicalAddress::new(0x8000_0000));
        let e_1g = PdptEntry::make_1g(g1, PageEntryBits::new_common_rw());
        match e_1g.kind().unwrap() {
            PdptEntryKind::Leaf1GiB(p, f) => {
                assert_eq!(p.base().as_u64(), 0x8000_0000);
                assert!(f.large_page());
            }
            _ => panic!("expected 1GiB leaf"),
        }
    }

    #[test]
    fn pd_table_vs_2m() {
        let pt = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x3000_0000));
        let e_tbl = PdEntry::make_next(pt, PageEntryBits::new_common_rw());
        match e_tbl.kind().unwrap() {
            PdEntryKind::NextPageTable(p, f) => {
                assert_eq!(p.base().as_u64(), 0x3000_0000);
                assert!(!f.large_page());
            }
            _ => panic!("expected next PT"),
        }

        let m2 = PhysicalPage::<Size2M>::from_addr(PhysicalAddress::new(0x4000_0000));
        let e_2m = PdEntry::make_2m(m2, PageEntryBits::new_common_rw());
        match e_2m.kind().unwrap() {
            PdEntryKind::Leaf2MiB(p, f) => {
                assert_eq!(p.base().as_u64(), 0x4000_0000);
                assert!(f.large_page());
            }
            _ => panic!("expected 2MiB leaf"),
        }
    }

    #[test]
    fn pte_4k_leaf() {
        let k4 = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x5555_0000));
        let e = PtEntry::make_4k(k4, PageEntryBits::new_user_ro_nx());
        let (p, fl) = e.page_4k().unwrap();
        assert_eq!(p.base().as_u64(), 0x5555_0000);
        assert!(!fl.large_page());
        assert!(fl.no_execute());
        assert!(fl.user_access());
        assert!(!fl.writable());
    }
}
