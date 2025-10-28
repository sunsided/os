use crate::PageEntryBits;
use crate::addr2::{PhysicalPage, Size1G, Size4K, VirtualAddress};

/// PDPT index (bits 38..30)
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L3Index(u16);

/// Either → PD table (PS=0) or 1GiB leaf (PS=1)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PdptEntry(PageEntryBits);

pub enum PdptEntryKind {
    NextPageDirectory(PhysicalPage<Size4K>, PageEntryBits),
    Leaf1GiB(PhysicalPage<Size1G>, PageEntryBits),
}

#[repr(C, align(4096))]
pub struct PageDirectoryPointerTable {
    entries: [PdptEntry; 512],
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
