use crate::PageEntryBits;
use crate::addr2::{PhysicalPage, Size2M, Size4K, VirtualAddress};

/// PD index (bits 29..21)
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L2Index(u16);

/// Either → PT table (PS=0) or 2MiB leaf (PS=1)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PdEntry(PageEntryBits);

pub enum PdEntryKind {
    NextPageTable(PhysicalPage<Size4K>, PageEntryBits),
    Leaf2MiB(PhysicalPage<Size2M>, PageEntryBits),
}

#[repr(C, align(4096))]
pub struct PageDirectory {
    entries: [PdEntry; 512],
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
