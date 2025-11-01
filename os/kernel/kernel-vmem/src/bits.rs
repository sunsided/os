use crate::addresses::{PhysicalPage, Size1G, Size2M, Size4K};
use crate::page_table::pd::{Pde, Pde2M};
use crate::page_table::pdpt::{Pdpte, Pdpte1G};
use crate::page_table::pml4::Pml4Entry;
use crate::page_table::pt::PtEntry4k;
use utils_accessors_derive::Setters;

/// Unified, ergonomic view over x86-64 paging entries (all levels / forms).
///
/// This type deliberately does **not** use bit-packing. Instead, it models the
/// *semantic superset* of fields across:
///
/// - L4: [`Pml4Entry`] (non-leaf only)
/// - L3: [`Pdpte`] (non-leaf) / [`Pdpte1G`] (1 GiB leaf)
/// - L2: [`Pde`] (non-leaf) / [`Pde2M`]  (2 MiB leaf)
/// - L1: [`PtEntry4k`]  (4 KiB leaf)
///
/// Use the provided `from_*` and `to_*` helpers to map between this view and
/// the actual bitfield entries. Alignment and level-specific constraints are
/// validated in `to_*` conversions (debug assertions).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Setters)]
#[allow(clippy::struct_excessive_bools)]
pub struct VirtualMemoryPageBits {
    /// **Present** (P): valid entry if `true`.
    pub present: bool,

    /// **Writable** (RW): allows writes if `true`.
    pub writable: bool,

    /// **User/Supervisor** (US): user-mode access if `true`.
    pub user: bool,

    /// **Page Write-Through** (PWT): write-through policy (PAT bit0).
    pub write_through: bool,

    /// **Page Cache Disable** (PCD): disable caching (PAT bit1).
    pub cache_disable: bool,

    /// **Accessed** (A): set by CPU on first access. Not a permission bit.
    pub accessed: bool,

    /// **Dirty** (D): set by CPU on first write.
    ///
    /// Meaningful for **leaf** entries only. Ignored in non-leaf mappings.
    pub dirty: bool,

    /// **Global** (G): TLB not flushed on CR3 reload (leaf only).
    pub global: bool,

    /// **Execute Disable** / NX: disallow instruction fetch if `true`.
    ///
    /// Requires `EFER.NXE` support. Applies across the walk (permission
    /// intersection).
    pub no_execute: bool,

    /// **Protection key** (PKU) or OS-use (bits 59..62).
    ///
    /// Only meaningful if PKU is supported; otherwise free for OS use.
    /// Value is masked to 4 bits on `to_*`.
    pub protection_key: u8,

    /// **OS-available (low)**: bits 9..11 (masked to 3 bits in `to_*`).
    pub os_available_low: u8,

    /// **OS-available (high)**: bits 52..58 (masked to 7 bits in `to_*`).
    pub os_available_high: u8,

    /// PAT selector **bit #2**:
    ///
    /// - **4 KiB leaf**: this is PTE bit7 (PAT small).
    /// - **2 MiB leaf**: this is PDE bit12 (PAT large).
    /// - **1 GiB leaf**: this is PDPTE bit12 (PAT large).
    /// - **Non-leaf**: **ignored** and forced to `false` in `to_*`.
    ///
    /// Combined with `write_through`(bit3) and `cache_disable`(bit4) to form the
    /// 3-bit PAT index: `[pat_bit2 : PCD : PWT]`.
    pub pat_bit2: bool,
}

impl VirtualMemoryPageBits {
    /// Enable write-combining (WC) via PAT registers.
    #[inline]
    #[must_use]
    pub const fn with_write_combining(self) -> Self {
        self.with_write_through(false)
            .with_cache_disable(true)
            .with_pat_bit2(true)
    }

    /// Populate from an L4 [`Pml4Entry`] (non-leaf).
    #[must_use]
    pub const fn from_pml4e(e: &Pml4Entry) -> Self {
        Self {
            present: e.present(),
            writable: e.writable(),
            user: e.user(),
            write_through: e.write_through(),
            cache_disable: e.cache_disable(),
            accessed: e.accessed(),
            dirty: false,  // ignored at non-leaf
            global: false, // ignored at non-leaf
            no_execute: e.no_execute(),
            protection_key: e.protection_key(),
            os_available_low: e.os_available_low() & 0b111,
            os_available_high: e.os_available_high() & 0x7F,
            pat_bit2: false, // not applicable for non-leaf
        }
    }

    /// Populate from an L3 [`Pdpte`] (non-leaf).
    #[must_use]
    pub const fn from_pdpte(e: &Pdpte) -> Self {
        Self {
            present: e.present(),
            writable: e.writable(),
            user: e.user(),
            write_through: e.write_through(),
            cache_disable: e.cache_disable(),
            accessed: e.accessed(),
            dirty: false,
            global: false,
            no_execute: e.no_execute(),
            protection_key: e.protection_key(),
            os_available_low: e.os_available_low() & 0b111,
            os_available_high: e.os_available_high() & 0x7F,
            pat_bit2: false,
        }
    }

    /// Populate from an L3 [`Pdpte1G`] (1 GiB leaf).
    #[must_use]
    pub const fn from_pdpte_1g(e: &Pdpte1G) -> Self {
        Self {
            present: e.present(),
            writable: e.writable(),
            user: e.user(),
            write_through: e.write_through(),
            cache_disable: e.cache_disable(),
            accessed: e.accessed(),
            dirty: e.dirty(),
            global: e.global(),
            no_execute: e.no_execute(),
            protection_key: e.protection_key(),
            os_available_low: e.os_available_low() & 0b111,
            os_available_high: e.os_available_high() & 0x7F,
            pat_bit2: e.pat_large(),
        }
    }

    /// Populate from an L2 [`Pde`] (non-leaf).
    #[must_use]
    pub const fn from_pde(e: &Pde) -> Self {
        Self {
            present: e.present(),
            writable: e.writable(),
            user: e.user(),
            write_through: e.write_through(),
            cache_disable: e.cache_disable(),
            accessed: e.accessed(),
            dirty: false,
            global: false,
            no_execute: e.no_execute(),
            protection_key: e.protection_key(),
            os_available_low: e.os_available_low() & 0b111,
            os_available_high: e.os_available_high() & 0x7F,
            pat_bit2: false,
        }
    }

    /// Populate from an L2 [`Pde2M`] (2 MiB leaf).
    #[must_use]
    pub const fn from_pde_2m(e: &Pde2M) -> Self {
        Self {
            present: e.present(),
            writable: e.writable(),
            user: e.user(),
            write_through: e.write_through(),
            cache_disable: e.cache_disable(),
            accessed: e.accessed(),
            dirty: e.dirty(),
            global: e.global(),
            no_execute: e.no_execute(),
            protection_key: e.protection_key(),
            os_available_low: e.os_available_low() & 0b111,
            os_available_high: e.os_available_high() & 0x7F,
            pat_bit2: e.pat_large(),
        }
    }

    /// Populate from an L1 [`PtEntry4k`] (4 KiB leaf).
    #[must_use]
    pub const fn from_pte_4k(e: &PtEntry4k) -> Self {
        Self {
            present: e.present(),
            writable: e.writable(),
            user: e.user(),
            write_through: e.write_through(),
            cache_disable: e.cache_disable(),
            accessed: e.accessed(),
            dirty: e.dirty(),
            global: e.global(),
            no_execute: e.no_execute(),
            protection_key: e.protection_key(),
            os_available_low: e.os_available_low() & 0b111,
            os_available_high: e.os_available_high() & 0x7F,
            pat_bit2: e.pat_small(),
        }
    }
}

impl VirtualMemoryPageBits {
    /// Encode into [`Pml4Entry`] (non-leaf).
    ///
    /// - Requires `form == L4Entry`
    /// - Enforces 4 KiB alignment; ignores `dirty`, `global`, `pat_bit2`.
    #[must_use]
    pub const fn to_pml4e(&self, page: PhysicalPage<Size4K>) -> Pml4Entry {
        let mut e = Pml4Entry::new();
        e.set_present(self.present);
        e.set_writable(self.writable);
        e.set_user(self.user);
        e.set_write_through(self.write_through);
        e.set_cache_disable(self.cache_disable);
        e.set_accessed(self.accessed);
        e.set_no_execute(self.no_execute);
        e.set_os_available_low(self.os_available_low & 0b111);
        e.set_os_available_high(self.os_available_high & 0x7F);
        e.set_protection_key(self.protection_key & 0x0F);
        e.set_physical_address(page);
        e
    }

    /// Encode into [`Pdpte`] (non-leaf).
    #[must_use]
    pub const fn to_pdpte(&self, page: PhysicalPage<Size4K>) -> Pdpte {
        let mut e = Pdpte::new();
        e.set_present(self.present);
        e.set_writable(self.writable);
        e.set_user(self.user);
        e.set_write_through(self.write_through);
        e.set_cache_disable(self.cache_disable);
        e.set_accessed(self.accessed);
        e.set_no_execute(self.no_execute);
        e.set_os_available_low(self.os_available_low & 0b111);
        e.set_os_available_high(self.os_available_high & 0x7F);
        e.set_protection_key(self.protection_key & 0x0F);
        e.set_physical_page(page);
        e
    }

    /// Encode into [`Pdpte1G`] (1 GiB leaf).
    #[must_use]
    pub const fn to_pdpte_1g(&self, page: PhysicalPage<Size1G>) -> Pdpte1G {
        let mut e = Pdpte1G::new();
        e.set_present(self.present);
        e.set_writable(self.writable);
        e.set_user(self.user);
        e.set_write_through(self.write_through);
        e.set_cache_disable(self.cache_disable);
        e.set_accessed(self.accessed);
        e.set_dirty(self.dirty);
        e.set_global(self.global);
        e.set_no_execute(self.no_execute);
        e.set_os_available_low(self.os_available_low & 0b111);
        e.set_os_available_high(self.os_available_high & 0x7F);
        e.set_protection_key(self.protection_key & 0x0F);
        e.set_pat_large(self.pat_bit2);
        e.set_physical_page(page);
        // sets PS=1 internally in setter
        e
    }

    /// Encode into [`Pde`] (non-leaf).
    #[must_use]
    pub const fn to_pde(&self, page: PhysicalPage<Size4K>) -> Pde {
        let mut e = Pde::new();
        e.set_present(self.present);
        e.set_writable(self.writable);
        e.set_user(self.user);
        e.set_write_through(self.write_through);
        e.set_cache_disable(self.cache_disable);
        e.set_accessed(self.accessed);
        e.set_no_execute(self.no_execute);
        e.set_os_available_low(self.os_available_low & 0b111);
        e.set_os_available_high(self.os_available_high & 0x7F);
        e.set_protection_key(self.protection_key & 0x0F);
        e.set_physical_page(page);
        e
    }

    /// Encode into [`Pde2M`] (2 MiB leaf).
    #[must_use]
    pub const fn to_pde_2m(&self, page: PhysicalPage<Size2M>) -> Pde2M {
        let mut e = Pde2M::new();
        e.set_present(self.present);
        e.set_writable(self.writable);
        e.set_user(self.user);
        e.set_write_through(self.write_through);
        e.set_cache_disable(self.cache_disable);
        e.set_accessed(self.accessed);
        e.set_dirty(self.dirty);
        e.set_global(self.global);
        e.set_no_execute(self.no_execute);
        e.set_os_available_low(self.os_available_low & 0b111);
        e.set_os_available_high(self.os_available_high & 0x7F);
        e.set_protection_key(self.protection_key & 0x0F);
        e.set_pat_large(self.pat_bit2);
        e.set_physical_page(page);
        // sets PS=1 internally in setter
        e
    }

    /// Encode into [`PtEntry4k`] (4 KiB leaf).
    #[must_use]
    pub const fn to_pte_4k(&self, page: PhysicalPage<Size4K>) -> PtEntry4k {
        let mut e = PtEntry4k::new();
        e.set_present(self.present);
        e.set_writable(self.writable);
        e.set_user(self.user);
        e.set_write_through(self.write_through);
        e.set_cache_disable(self.cache_disable);
        e.set_accessed(self.accessed);
        e.set_dirty(self.dirty);
        e.set_global(self.global);
        e.set_no_execute(self.no_execute);
        e.set_os_available_low(self.os_available_low & 0b111);
        e.set_os_available_high(self.os_available_high & 0x7F);
        e.set_protection_key(self.protection_key & 0x0F);
        e.set_pat_small(self.pat_bit2);
        e.set_physical_page(page);
        e
    }
}

impl From<Pml4Entry> for VirtualMemoryPageBits {
    #[inline]
    fn from(e: Pml4Entry) -> Self {
        Self::from_pml4e(&e)
    }
}

impl From<Pdpte> for VirtualMemoryPageBits {
    #[inline]
    fn from(e: Pdpte) -> Self {
        Self::from_pdpte(&e)
    }
}

impl From<Pdpte1G> for VirtualMemoryPageBits {
    #[inline]
    fn from(e: Pdpte1G) -> Self {
        Self::from_pdpte_1g(&e)
    }
}

impl From<Pde> for VirtualMemoryPageBits {
    #[inline]
    fn from(e: Pde) -> Self {
        Self::from_pde(&e)
    }
}

impl From<Pde2M> for VirtualMemoryPageBits {
    #[inline]
    fn from(e: Pde2M) -> Self {
        Self::from_pde_2m(&e)
    }
}

impl From<PtEntry4k> for VirtualMemoryPageBits {
    #[inline]
    fn from(e: PtEntry4k) -> Self {
        Self::from_pte_4k(&e)
    }
}
