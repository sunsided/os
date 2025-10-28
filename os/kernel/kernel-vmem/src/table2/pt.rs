use crate::PageEntryBits;
use crate::addr2::{PhysicalPage, Size4K, VirtualAddress};

/// PT index (bits 20..12)
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L1Index(u16);

/// 4KiB leaf only (PS must be 0)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PtEntry(PageEntryBits);

#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PtEntry; 512],
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
