use crate::PageEntryBits;
use crate::addr2::{PhysicalPage, Size1G, Size2M, Size4K, VirtualAddress};

/// PML4 index (bits 47..39)
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L4Index(u16);

/// PDPT index (bits 38..30)
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L3Index(u16);

/// PD index (bits 29..21)
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L2Index(u16);

/// PT index (bits 20..12)
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L1Index(u16);

impl L4Index {
    #[inline]
    #[must_use]
    pub const fn from(va: VirtualAddress) -> Self {
        Self::new(((va.as_u64() >> 39) & 0x1FF) as u16)
    }

    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        debug_assert!(v < 512);
        Self(v)
    }

    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl L3Index {
    #[inline]
    #[must_use]
    pub const fn from(va: VirtualAddress) -> Self {
        Self::new(((va.as_u64() >> 30) & 0x1FF) as u16)
    }

    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        debug_assert!(v < 512);
        Self(v)
    }

    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl L2Index {
    #[inline]
    #[must_use]
    pub const fn from(va: VirtualAddress) -> Self {
        Self::new(((va.as_u64() >> 21) & 0x1FF) as u16)
    }

    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        debug_assert!(v < 512);
        Self(v)
    }

    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl L1Index {
    #[inline]
    #[must_use]
    pub const fn from(va: VirtualAddress) -> Self {
        Self::new(((va.as_u64() >> 12) & 0x1FF) as u16)
    }

    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        debug_assert!(v < 512);
        Self(v)
    }

    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

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

/// Points to an L3 table (PDPT). PS must be 0.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Pml4Entry(PageEntryBits);

impl Pml4Entry {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(PageEntryBits::new())
    }

    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    #[inline]
    #[must_use]
    pub const fn flags(self) -> PageEntryBits {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn next_table(self) -> Option<PhysicalPage<Size4K>> {
        if !self.is_present() {
            return None;
        }
        Some(PhysicalPage::from_addr(self.0.physical_address()))
    }

    #[inline]
    #[must_use]
    pub fn make(next_pdpt_page: PhysicalPage<Size4K>, mut flags: PageEntryBits) -> Self {
        debug_assert!(!flags.large_page(), "PML4E must have PS=0");
        flags.set_present(true);
        flags.set_physical_address(next_pdpt_page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0.into()
    }

    #[inline]
    #[must_use]
    pub fn from_raw(v: u64) -> Self {
        Self(PageEntryBits::from(v))
    }
}

/// Either → PD table (PS=0) or 1GiB leaf (PS=1)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PdptEntry(PageEntryBits);

pub enum PdptEntryKind {
    NextPageDirectory(PhysicalPage<Size4K>, PageEntryBits),
    Leaf1GiB(PhysicalPage<Size1G>, PageEntryBits),
}

impl PdptEntry {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(PageEntryBits::new())
    }

    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    #[inline]
    #[must_use]
    pub const fn flags(self) -> PageEntryBits {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn kind(self) -> Option<PdptEntryKind> {
        if !self.is_present() {
            return None;
        }

        let flags = self.0;
        let base = self.0.physical_address();
        if flags.large_page() {
            Some(PdptEntryKind::Leaf1GiB(
                PhysicalPage::<Size1G>::from_addr(base),
                flags,
            ))
        } else {
            Some(PdptEntryKind::NextPageDirectory(
                PhysicalPage::<Size4K>::from_addr(base),
                flags,
            ))
        }
    }

    #[inline]
    #[must_use]
    pub const fn make_next(pd_page: PhysicalPage<Size4K>, mut flags: PageEntryBits) -> Self {
        flags.set_large_page(false);
        flags.set_present(true);
        flags.set_physical_address(pd_page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub const fn make_1g(page: PhysicalPage<Size1G>, mut flags: PageEntryBits) -> Self {
        flags.set_large_page(true);
        flags.set_present(true);
        flags.set_physical_address(page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0.into_bits()
    }

    #[inline]
    #[must_use]
    pub fn from_raw(v: u64) -> Self {
        Self(PageEntryBits::from(v))
    }
}

/// Either → PT table (PS=0) or 2MiB leaf (PS=1)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PdEntry(PageEntryBits);

pub enum PdEntryKind {
    NextPageTable(PhysicalPage<Size4K>, PageEntryBits),
    Leaf2MiB(PhysicalPage<Size2M>, PageEntryBits),
}

impl PdEntry {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(PageEntryBits::new())
    }

    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    #[inline]
    #[must_use]
    pub const fn flags(self) -> PageEntryBits {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn kind(self) -> Option<PdEntryKind> {
        if !self.is_present() {
            return None;
        }

        let flags = self.0;
        let base = self.0.physical_address();
        if flags.large_page() {
            Some(PdEntryKind::Leaf2MiB(PhysicalPage::from_addr(base), flags))
        } else {
            Some(PdEntryKind::NextPageTable(
                PhysicalPage::from_addr(base),
                flags,
            ))
        }
    }

    #[inline]
    #[must_use]
    pub const fn make_next(pt_page: PhysicalPage<Size4K>, mut flags: PageEntryBits) -> Self {
        flags.set_large_page(false);
        flags.set_present(true);
        flags.set_physical_address(pt_page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub const fn make_2m(page: PhysicalPage<Size2M>, mut flags: PageEntryBits) -> Self {
        flags.set_large_page(true);
        flags.set_present(true);
        flags.set_physical_address(page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0.into()
    }
    #[inline]
    #[must_use]
    pub fn from_raw(v: u64) -> Self {
        Self(PageEntryBits::from(v))
    }
}

/// 4KiB leaf only (PS must be 0)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PtEntry(PageEntryBits);

impl PtEntry {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(PageEntryBits::new())
    }

    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    #[inline]
    #[must_use]
    pub const fn flags(self) -> PageEntryBits {
        self.0
    }

    #[inline]
    #[must_use]
    pub fn page_4k(self) -> Option<(PhysicalPage<Size4K>, PageEntryBits)> {
        if !self.is_present() {
            return None;
        }
        debug_assert!(!self.0.large_page(), "PTE must have PS=0");
        Some((PhysicalPage::from_addr(self.0.physical_address()), self.0))
    }

    #[inline]
    #[must_use]
    pub const fn make_4k(page: PhysicalPage<Size4K>, mut flags: PageEntryBits) -> Self {
        flags.set_large_page(false);
        flags.set_present(true);
        flags.set_physical_address(page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0.into()
    }

    #[inline]
    #[must_use]
    pub fn from_raw(v: u64) -> Self {
        Self(PageEntryBits::from(v))
    }
}

#[repr(C, align(4096))]
pub struct PageMapLevel4 {
    entries: [Pml4Entry; 512],
}

impl PageMapLevel4 {
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [Pml4Entry::zero(); 512],
        }
    }

    #[inline]
    #[must_use]
    pub const fn get(&self, i: L4Index) -> Pml4Entry {
        self.entries[i.as_usize()]
    }

    #[inline]
    pub const fn set(&mut self, i: L4Index, e: Pml4Entry) {
        self.entries[i.as_usize()] = e;
    }

    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L4Index {
        L4Index::from(va)
    }
}

#[repr(C, align(4096))]
pub struct PageDirectoryPointerTable {
    entries: [PdptEntry; 512],
}

impl PageDirectoryPointerTable {
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [PdptEntry::zero(); 512],
        }
    }

    #[inline]
    #[must_use]
    pub const fn get(&self, i: L3Index) -> PdptEntry {
        self.entries[i.as_usize()]
    }

    #[inline]
    pub const fn set(&mut self, i: L3Index, e: PdptEntry) {
        self.entries[i.as_usize()] = e;
    }

    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L3Index {
        L3Index::from(va)
    }
}

#[repr(C, align(4096))]
pub struct PageDirectory {
    entries: [PdEntry; 512],
}

impl PageDirectory {
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [PdEntry::zero(); 512],
        }
    }

    #[inline]
    #[must_use]
    pub const fn get(&self, i: L2Index) -> PdEntry {
        self.entries[i.as_usize()]
    }

    #[inline]
    pub const fn set(&mut self, i: L2Index, e: PdEntry) {
        self.entries[i.as_usize()] = e;
    }

    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L2Index {
        L2Index::from(va)
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PtEntry; 512],
}

impl PageTable {
    #[inline]
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            entries: [PtEntry::zero(); 512],
        }
    }

    #[inline]
    #[must_use]
    pub const fn get(&self, i: L1Index) -> PtEntry {
        self.entries[i.as_usize()]
    }

    #[inline]
    pub const fn set(&mut self, i: L1Index, e: PtEntry) {
        self.entries[i.as_usize()] = e;
    }

    #[inline]
    #[must_use]
    pub const fn index_of(va: VirtualAddress) -> L1Index {
        L1Index::from(va)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addr2::PhysicalAddress;

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
