use crate::addr2::{PhysicalAddress, PhysicalPage, Size1G, Size2M, Size4K, VirtualAddress};
use bitfield_struct::bitfield;

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
pub const fn l4_index_of(va: VirtualAddress) -> L4Index {
    L4Index::new(((va.as_u64() >> 39) & 0x1FF) as u16)
}

#[inline]
#[must_use]
pub const fn l3_index_of(va: VirtualAddress) -> L3Index {
    L3Index::new(((va.as_u64() >> 30) & 0x1FF) as u16)
}

#[inline]
#[must_use]
pub const fn l2_index_of(va: VirtualAddress) -> L2Index {
    L2Index::new(((va.as_u64() >> 21) & 0x1FF) as u16)
}

#[inline]
#[must_use]
pub const fn l1_index_of(va: VirtualAddress) -> L1Index {
    L1Index::new(((va.as_u64() >> 12) & 0x1FF) as u16)
}

#[inline]
#[must_use]
pub const fn split_indices(va: VirtualAddress) -> (L4Index, L3Index, L2Index, L1Index) {
    (
        l4_index_of(va),
        l3_index_of(va),
        l2_index_of(va),
        l1_index_of(va),
    )
}

/// Represents a single 64-bit x86-64 page table entry in its raw bitfield form.
///
/// This structure models the **common superset** of fields found in all
/// four paging levels (PML4E, PDPTE, PDE, PTE). Each bit corresponds to a
/// hardware-defined flag or address field as specified by the AMD64 and
/// Intel manuals.
///
/// The type allows read/write access to individual bits without manual masking
/// or shifting, using the [`bitfield_struct`](https://docs.rs/bitfield-struct/)
/// derive.
///
/// ### Overview
/// A page table entry (PTE) may either:
/// - point to a **next-level page table**, or
/// - directly map a **physical page (leaf)** when the `large_page` (PS) bit is set.
///
/// Fields such as `dirty` and `global_translation` are meaningful only for
/// **leaf entries**, while others (e.g. `large_page`) have specific validity
/// rules depending on the table level.
///
/// ### Bit layout (canonical)
///
/// | Bits      | Name / Mnemonic   | Meaning |
/// |-----------|-------------------|----------|
/// | 0         | `P` (present)     | Valid entry if set |
/// | 1         | `RW`              | Writable if set |
/// | 2         | `US`              | User-mode accessible if set |
/// | 3         | `PWT`             | Write-through caching |
/// | 4         | `PCD`             | Disable caching |
/// | 5         | `A`               | Accessed |
/// | 6         | `D`               | Dirty (leaf only) |
/// | 7         | `PS`              | Large page flag |
/// | 8         | `G`               | Global (leaf only) |
/// | 9–11      | OS avail low      | Reserved for OS use |
/// | 12–51     | `addr`            | Physical frame bits [51:12] |
/// | 52–58     | OS avail high     | Reserved for OS use |
/// | 59–62     | `PKU` / OS use    | Protection key or OS use |
/// | 63        | `NX`              | Execute disable |
///
/// ### Notes
/// - Non-leaf entries ignore bits `D`, `G`, and `NX`.
/// - `PS` must be 0 in L4 and L1 entries; valid in L3 (1 GiB) and L2 (2 MiB).
/// - The physical address field always omits the lower 12 bits, which are
///   implicitly zero due to alignment.
/// - When PKU is not supported, bits 59–62 are reserved for OS use.
///
/// ### Example
/// ```rust
/// # use kernel_vmem::addr2::PhysicalAddress;
/// use kernel_vmem::table2::*;
/// let mut e = EntryBits::new();
/// e.set_present(true);
/// e.set_writable(true);
/// e.set_physical_address(PhysicalAddress::new(0x12345));
/// assert!(e.present());
/// ```
///
/// This type is typically used as part of higher-level abstractions like
/// `PageTable`, `PageDirectory`, or `AddressSpace` to manage paging structures
/// in a type-safe way.
#[bitfield(u64)]
pub struct EntryBits {
    /// Present (P, bit 0).
    ///
    /// Set if the entry points to a valid next-level table or a valid leaf
    /// mapping (depending on level/PS). Clear implies a not-present entry.
    pub present: bool,

    /// Writable (RW, bit 1).
    ///
    /// Set to allow writes; clear for read-only. Subject to supervisor/user
    /// checks via `user_access` (US) and CR0.WP behavior in supervisor mode.
    pub writable: bool,

    /// User/Supervisor (US, bit 2).
    ///
    /// Set to allow user-mode access; clear restricts to supervisor only.
    /// Combined with CPL and SMEP/SMAP if enabled.
    pub user_access: bool,

    /// Page Write-Through (PWT, bit 3).
    ///
    /// Set to use write-through caching; clear for write-back, when caching
    /// is enabled. Meaningful only when caching is not disabled.
    pub write_through: bool,

    /// Page Cache Disable (PCD, bit 4).
    ///
    /// Set to disable caching for this mapping; clear to allow caching.
    /// May impact performance significantly.
    pub cache_disabled: bool,

    /// Accessed (A, bit 5).
    ///
    /// Set by the CPU on first access (read/write/execute) through this entry.
    /// Software may clear it to track usage. Not a permission bit.
    pub accessed: bool,

    /// Dirty (D, bit 6) — **leaf only**.
    ///
    /// Set by the CPU on first write to a leaf mapping. Ignored for non-leaf
    /// entries (next-level pointers). Software may clear it to track writes.
    pub dirty: bool,

    /// Large Page / Page Size (PS, bit 7).
    ///
    /// For L3 (PDPTE) and L2 (PDE): when **set**, the entry is a **leaf**
    /// mapping to a 1 GiB (L3) or 2 MiB (L2) page. When **clear**, the entry
    /// points to the next-level table.
    ///
    /// For L4 (PML4E) and L1 (PTE): must be **clear** (0). In a 4 KiB PTE
    /// the architectural bit position is repurposed as **PAT**; this unified
    /// “superset” view treats it as PS=0 at L1—handle PAT separately if needed.
    pub large_page: bool,

    /// Global (G, bit 8) — **leaf only**.
    ///
    /// When set on a leaf mapping, the TLB entry is not flushed on CR3 reload,
    /// unless explicitly invalidated. Ignored for non-leaf entries.
    pub global_translation: bool,

    /// OS-available (bits 9..=11).
    ///
    /// Reserved for operating system use; hardware doesn’t interpret these.
    #[bits(3)]
    pub os_available_low: u8,

    /// Physical address bits [51:12] (bits 12..=51).
    ///
    /// Stores the page-aligned physical frame address without the low 12 bits.
    /// Reconstruct the full physical address as: `(bits << 12)`.
    /// For large pages, alignment requirements increase (2 MiB/1 GiB).
    #[bits(40)]
    phys_addr_bits_51_12: u64,

    /// OS-available (bits 52..=58).
    ///
    /// Additional operating system–defined storage; ignored by hardware.
    #[bits(7)]
    pub os_available_high: u8,

    /// Protection Key (PKU, bits 59..=62) if supported; otherwise OS use.
    ///
    /// With Intel PKU enabled, selects one of up to 16 protection keys whose
    /// access is controlled by PKRU. If PKU isn’t supported/active, hardware
    /// ignores these bits and they may be used by the OS.
    #[bits(4)]
    pub protection_key: u8,

    /// No-Execute (NX, bit 63) — Execute Disable.
    ///
    /// When set, instruction fetches are disallowed through this entry.
    /// Requires `EFER.NXE` support; otherwise the bit is reserved.
    /// Note: permissions are the intersection over the walk; a single NX in
    /// the path suffices to block execution.
    pub no_execute: bool,
}

impl EntryBits {
    #[inline]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        // store bits [51:12]
        self.set_phys_addr_bits_51_12(phys.as_u64() >> 12);
    }

    #[inline]
    #[must_use]
    pub const fn physical_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.phys_addr_bits_51_12() << 12)
    }

    #[inline]
    #[must_use]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user_access(false)
            .with_write_through(false)
            .with_cache_disabled(false)
            .with_no_execute(false)
    }

    #[inline]
    #[must_use]
    pub const fn flags_user_rx() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(false)
            .with_user_access(true)
            .with_write_through(false)
            .with_cache_disabled(false)
            .with_no_execute(false)
    }

    #[inline]
    #[must_use]
    pub const fn new_user_ro_nx() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(false)
            .with_user_access(true)
            .with_no_execute(true)
    }
}

/// Points to an L3 table (PDPT). PS must be 0.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Pml4Entry(EntryBits);

impl Pml4Entry {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(EntryBits::new())
    }

    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    #[inline]
    #[must_use]
    pub const fn flags(self) -> EntryBits {
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
    pub fn make(next_pdpt_page: PhysicalPage<Size4K>, mut flags: EntryBits) -> Self {
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
        Self(EntryBits::from(v))
    }
}

/// Either → PD table (PS=0) or 1GiB leaf (PS=1)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PdptEntry(EntryBits);

pub enum PdptEntryKind {
    NextPageDirectory(PhysicalPage<Size4K>, EntryBits),
    Leaf1GiB(PhysicalPage<Size1G>, EntryBits),
}

impl PdptEntry {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(EntryBits::new())
    }

    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    #[inline]
    #[must_use]
    pub const fn flags(self) -> EntryBits {
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
    pub const fn make_next(pd_page: PhysicalPage<Size4K>, mut flags: EntryBits) -> Self {
        flags.set_large_page(false);
        flags.set_present(true);
        flags.set_physical_address(pd_page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub const fn make_1g(page: PhysicalPage<Size1G>, mut flags: EntryBits) -> Self {
        flags.set_large_page(true);
        flags.set_present(true);
        flags.set_physical_address(page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0.0
    }

    #[inline]
    #[must_use]
    pub fn from_raw(v: u64) -> Self {
        Self(EntryBits::from(v))
    }
}

/// Either → PT table (PS=0) or 2MiB leaf (PS=1)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PdEntry(EntryBits);

pub enum PdEntryKind {
    NextPageTable(PhysicalPage<Size4K>, EntryBits),
    Leaf2MiB(PhysicalPage<Size2M>, EntryBits),
}

impl PdEntry {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(EntryBits::new())
    }

    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    #[inline]
    #[must_use]
    pub const fn flags(self) -> EntryBits {
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
    pub const fn make_next(pt_page: PhysicalPage<Size4K>, mut flags: EntryBits) -> Self {
        flags.set_large_page(false);
        flags.set_present(true);
        flags.set_physical_address(pt_page.base());
        Self(flags)
    }

    #[inline]
    #[must_use]
    pub const fn make_2m(page: PhysicalPage<Size2M>, mut flags: EntryBits) -> Self {
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
        Self(EntryBits::from(v))
    }
}

/// 4KiB leaf only (PS must be 0)
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PtEntry(EntryBits);

impl PtEntry {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(EntryBits::new())
    }

    #[inline]
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0.present()
    }

    #[inline]
    #[must_use]
    pub const fn flags(self) -> EntryBits {
        self.0
    }

    #[inline]
    #[must_use]
    pub fn page_4k(self) -> Option<(PhysicalPage<Size4K>, EntryBits)> {
        if !self.is_present() {
            return None;
        }
        debug_assert!(!self.0.large_page(), "PTE must have PS=0");
        Some((PhysicalPage::from_addr(self.0.physical_address()), self.0))
    }

    #[inline]
    #[must_use]
    pub const fn make_4k(page: PhysicalPage<Size4K>, mut flags: EntryBits) -> Self {
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
        Self(EntryBits::from(v))
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
        l4_index_of(va)
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
        l3_index_of(va)
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
        l2_index_of(va)
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
        l1_index_of(va)
    }
}

/* ------------------------------ Tests -------------------------------- */

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

    #[test]
    fn pml4_points_to_pdpt() {
        let pdpt_page = PhysicalPage::<Size4K>::from_addr(PhysicalAddress::new(0x1234_5000));
        let mut f = EntryBits::new();
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
        let e_tbl = PdptEntry::make_next(pd, EntryBits::new_common_rw());
        match e_tbl.kind().unwrap() {
            PdptEntryKind::NextPageDirectory(p, f) => {
                assert_eq!(p.base().as_u64(), 0x2000_0000);
                assert!(!f.large_page());
            }
            _ => panic!("expected next PD"),
        }

        // 1 GiB leaf
        let g1 = PhysicalPage::<Size1G>::from_addr(PhysicalAddress::new(0x8000_0000));
        let e_1g = PdptEntry::make_1g(g1, EntryBits::new_common_rw());
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
        let e_tbl = PdEntry::make_next(pt, EntryBits::new_common_rw());
        match e_tbl.kind().unwrap() {
            PdEntryKind::NextPageTable(p, f) => {
                assert_eq!(p.base().as_u64(), 0x3000_0000);
                assert!(!f.large_page());
            }
            _ => panic!("expected next PT"),
        }

        let m2 = PhysicalPage::<Size2M>::from_addr(PhysicalAddress::new(0x4000_0000));
        let e_2m = PdEntry::make_2m(m2, EntryBits::new_common_rw());
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
        let e = PtEntry::make_4k(k4, EntryBits::new_user_ro_nx());
        let (p, fl) = e.page_4k().unwrap();
        assert_eq!(p.base().as_u64(), 0x5555_0000);
        assert!(!fl.large_page());
        assert!(fl.no_execute());
        assert!(fl.user_access());
        assert!(!fl.writable());
    }
}
