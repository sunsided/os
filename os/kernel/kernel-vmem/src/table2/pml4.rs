use crate::PageEntryBits;
use crate::addr2::{PhysicalPage, Size4K, VirtualAddress};

/// PML4 index (bits 47..39)
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L4Index(u16);

/// Points to an L3 table (PDPT). PS must be 0.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Pml4Entry(PageEntryBits);

#[repr(C, align(4096))]
pub struct PageMapLevel4 {
    entries: [Pml4Entry; 512],
}

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
