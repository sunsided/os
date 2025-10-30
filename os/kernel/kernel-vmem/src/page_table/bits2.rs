use crate::addresses::PhysicalAddress;
use bitfield_struct::bitfield;

/// Hardware **Present** bit position shared across levels (bit 0).
const PRESENT_BIT: u64 = 1 << 0;

/// Hardware **Page Size** (PS) bit position shared across levels (bit 7).
///
/// - In non-leaf entries: PS **must be 0**.
/// - In large leaf entries (L3 1 GiB / L2 2 MiB): PS **must be 1**.
/// - In L1 4 KiB PTEs: bit 7 is **PAT** (not PS).
const PS_BIT: u64 = 1 << 7;

/* ======================== L4: PML4E (non-leaf only) ======================= */

/// L4 **PML4E** — pointer to a **PDPT** (non-leaf; PS **must be 0**).
///
/// This entry never maps memory directly. Bits that are meaningful only on
/// leaf entries (e.g., `dirty`, `global`) are ignored here.
///
/// - Physical address (bits **51:12**) is a 4 KiB-aligned PDPT.
/// - `NX` participates in permission intersection across the walk.
/// - `PKU` may be repurposed as OS-available when not supported.
///
/// Reference: AMD APM / Intel SDM paging structures (x86-64).
#[bitfield(u64)]
pub struct Pml4e {
    /// **Present** (bit 0): valid entry if set.
    ///
    /// When clear, the entry is not present and most other fields are ignored.
    pub present: bool,

    /// **Writable** (bit 1): write permission.
    ///
    /// Intersects with lower-level permissions; supervisor write protection,
    /// SMEP/SMAP, CR0.WP, and U/S checks apply.
    pub writable: bool,

    /// **User/Supervisor** (bit 2): allow user-mode access if set.
    ///
    /// If clear, access is restricted to supervisor (ring 0).
    pub user: bool,

    /// **Page Write-Through** (PWT, bit 3): write-through caching policy.
    ///
    /// Effective only if caching isn’t disabled for the mapping.
    pub write_through: bool,

    /// **Page Cache Disable** (PCD, bit 4): disable caching if set.
    ///
    /// Strongly impacts performance; use for MMIO or compliance with device
    /// requirements. Effective policy is the intersection across the walk.
    pub cache_disable: bool,

    /// **Accessed** (A, bit 5): set by CPU on first access via this entry.
    ///
    /// Software may clear to track usage; not a permission bit.
    pub accessed: bool,

    /// (bit 6): **ignored** for non-leaf entries at L4.
    #[bits(1)]
    __d_ignored: u8,

    /// **Page Size** (bit 7): **must be 0** for PML4E (non-leaf).
    #[bits(1)]
    __ps_must_be_0: u8,

    /// **Global** (bit 8): **ignored** for non-leaf entries.
    #[bits(1)]
    __g_ignored: u8,

    /// **OS-available low** (bits 9..11): not interpreted by hardware.
    #[bits(3)]
    pub os_available_low: u8,

    /// **Next-level table physical address** (bits 12..51).
    ///
    /// Stores the PDPT base (4 KiB-aligned). The low 12 bits are omitted.
    #[bits(40)]
    phys_addr_51_12: u64,

    /// **OS-available high** (bits 52..58): not interpreted by hardware.
    #[bits(7)]
    pub os_available_high: u8,

    /// **Protection Key / OS use** (bits 59..62).
    ///
    /// If PKU is supported and enabled, these bits select the protection key;
    /// otherwise they may be used by the OS.
    #[bits(4)]
    pub protection_key: u8,

    /// **No-Execute** (NX, bit 63 / XD on Intel).
    ///
    /// When set and EFER.NXE is enabled, instruction fetch is disallowed
    /// through this entry (permission intersection applies).
    pub no_execute: bool,
}

impl Pml4e {
    /// Set the PDPT base address (must be 4 KiB-aligned).
    #[inline]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        debug_assert!(phys.is_aligned_to(0x1000));
        self.set_phys_addr_51_12(phys.as_u64() >> 12);
    }

    /// Get the PDPT base address (4 KiB-aligned).
    #[inline]
    pub const fn physical_address(self) -> PhysicalAddress {
        PhysicalAddress::new(self.phys_addr_51_12() << 12)
    }
}

/* ==================== L3: PDPTE (non-leaf or 1 GiB leaf) ================== */

/// L3 **PDPTE** — pointer to a **Page Directory** (non-leaf; PS **= 0**).
///
/// - Physical address (bits **51:12**) is a 4 KiB-aligned PD.
/// - Leaf-only fields (Dirty/Global) are ignored.
/// - Setting PS here would mean a 1 GiB leaf; use [`Pdpte1G`] for that.
#[bitfield(u64)]
pub struct Pdpte {
    /// Present (bit 0): valid entry if set.
    pub present: bool,
    /// Writable (bit 1): write permission.
    pub writable: bool,
    /// User (bit 2): user-mode access if set.
    pub user: bool,
    /// Write-Through (bit 3).
    pub write_through: bool,
    /// Cache Disable (bit 4).
    pub cache_disable: bool,
    /// Accessed (bit 5).
    pub accessed: bool,
    /// Dirty (bit 6): **ignored** in non-leaf form.
    #[bits(1)]
    __d_ignored: u8,
    /// PS (bit 7): **must be 0** in non-leaf.
    #[bits(1)]
    __ps_must_be_0: u8,
    /// Global (bit 8): **ignored** in non-leaf.
    #[bits(1)]
    __g_ignored: u8,

    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,

    /// Next-level table physical address (bits 12..51, 4 KiB-aligned).
    #[bits(40)]
    phys_addr_51_12: u64,

    /// OS-available high (bits 52..58).
    #[bits(7)]
    pub os_available_high: u8,
    /// Protection Key / OS use (59..62).
    #[bits(4)]
    pub protection_key: u8,
    /// No-Execute (bit 63).
    pub no_execute: bool,
}

impl Pdpte {
    /// Set the Page Directory base (4 KiB-aligned).
    #[inline]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        debug_assert!(phys.is_aligned_to(0x1000));
        self.set_phys_addr_51_12(phys.as_u64() >> 12);
    }

    /// Get the Page Directory base (4 KiB-aligned).
    #[inline]
    pub const fn physical_address(self) -> PhysicalAddress {
        PhysicalAddress::new(self.phys_addr_51_12() << 12)
    }
}

/// L3 **PDPTE (1 GiB leaf)** — maps a single 1 GiB page (`PS = 1`).
///
/// - **PAT** (Page Attribute Table) selector lives at bit **12** in this form.
/// - Physical address uses bits **51:30** and must be **1 GiB aligned**.
/// - `Dirty` is set by CPU on first write; `Global` keeps TLB entries across
///   CR3 reload unless explicitly invalidated.
///
/// This is a terminal mapping (leaf).
#[bitfield(u64)]
pub struct Pdpte1G {
    /// Present (bit 0).
    pub present: bool,
    /// Writable (bit 1).
    pub writable: bool,
    /// User (bit 2).
    pub user: bool,
    /// Write-Through (bit 3).
    pub write_through: bool,
    /// Cache Disable (bit 4).
    pub cache_disable: bool,
    /// Accessed (bit 5).
    pub accessed: bool,

    /// **Dirty** (bit 6): set by CPU on first write to this 1 GiB page.
    pub dirty: bool,

    /// **Page Size** (bit 7): **must be 1** for 1 GiB leaf.
    #[bits(default = true)]
    page_size: bool,

    /// **Global** (bit 8): TLB entry not flushed on CR3 reload.
    pub global: bool,

    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,

    /// **PAT** (Page Attribute Table) selector for 1 GiB mappings (bit 12).
    pub pat_large: bool,

    /// Reserved (bits 13..29): must be 0.
    #[bits(17)]
    __res_13_29: u32,

    /// Physical address bits **51:30** (1 GiB-aligned base).
    #[bits(22)]
    phys_addr_51_30: u32,

    /// OS-available high (bits 52..58).
    #[bits(7)]
    pub os_available_high: u8,

    /// Protection Key / OS use (59..62).
    #[bits(4)]
    pub protection_key: u8,

    /// No-Execute (bit 63).
    pub no_execute: bool,
}

impl Pdpte1G {
    /// Set the 1 GiB page base (must be 1 GiB-aligned).
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        debug_assert!(phys.is_aligned_to(1 << 30));
        self.set_phys_addr_51_30((phys.as_u64() >> 30) as u32);
        self.set_page_size(true);
    }

    /// Get the 1 GiB page base.
    #[inline]
    pub const fn physical_address(self) -> PhysicalAddress {
        PhysicalAddress::new((self.phys_addr_51_30() as u64) << 30)
    }
}

/* =================== L2: PDE (non-leaf or 2 MiB leaf) ===================== */

/// L2 **PDE** — pointer to a **Page Table** (non-leaf; PS **= 0**).
///
/// - Physical address (bits **51:12**) is a 4 KiB-aligned PT.
/// - In non-leaf PDEs, **PAT lives at bit 12 only in the leaf form**;
///   here, all bits 12..51 are the next-level table address.
#[bitfield(u64)]
pub struct Pde {
    /// Present (bit 0).
    pub present: bool,
    /// Writable (bit 1).
    pub writable: bool,
    /// User (bit 2).
    pub user: bool,
    /// Write-Through (bit 3).
    pub write_through: bool,
    /// Cache Disable (bit 4).
    pub cache_disable: bool,
    /// Accessed (bit 5).
    pub accessed: bool,
    /// Dirty (bit 6): **ignored** in non-leaf.
    #[bits(1)]
    __d_ignored: u8,
    /// PS (bit 7): **must be 0** in non-leaf.
    #[bits(1)]
    __ps_must_be_0: u8,
    /// Global (bit 8): **ignored** in non-leaf.
    #[bits(1)]
    __g_ignored: u8,

    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,

    /// **Next-level table physical address** (bits 12..51, 4 KiB-aligned).
    ///
    /// Note: Do **not** insert reserved placeholders here; in non-leaf form
    /// these bits are entirely the PT base address.
    #[bits(40)]
    phys_addr_51_12: u64,

    /// OS-available high (bits 52..58).
    #[bits(7)]
    pub os_available_high: u8,
    /// Protection Key / OS use (59..62).
    #[bits(4)]
    pub protection_key: u8,
    /// No-Execute (bit 63).
    pub no_execute: bool,
}

impl Pde {
    /// Set the Page Table base (4 KiB-aligned).
    #[inline]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        debug_assert!(phys.is_aligned_to(0x1000));
        self.set_phys_addr_51_12(phys.as_u64() >> 12);
    }

    /// Get the Page Table base.
    #[inline]
    pub const fn physical_address(self) -> PhysicalAddress {
        PhysicalAddress::new(self.phys_addr_51_12() << 12)
    }
}

/// L2 **PDE (2 MiB leaf)** — maps a single 2 MiB page (`PS = 1`).
///
/// - **PAT** (Page Attribute Table) selector lives at bit **12** in this form.
/// - Physical address uses bits **51:21** and must be **2 MiB aligned**.
/// - `Dirty` is set by CPU on first write; `Global` keeps TLB entries across
///   CR3 reload unless explicitly invalidated.
///
/// This is a terminal mapping (leaf).
#[bitfield(u64)]
pub struct Pde2M {
    /// Present (bit 0).
    pub present: bool,
    /// Writable (bit 1).
    pub writable: bool,
    /// User (bit 2).
    pub user: bool,
    /// Write-Through (bit 3).
    pub write_through: bool,
    /// Cache Disable (bit 4).
    pub cache_disable: bool,
    /// Accessed (bit 5).
    pub accessed: bool,

    /// **Dirty** (bit 6): set by CPU on first write to this 2 MiB page.
    pub dirty: bool,

    /// **Page Size** (bit 7): **must be 1** for 2 MiB leaf.
    #[bits(default = true)]
    pub(crate) page_size: bool,

    /// **Global** (bit 8): TLB entry not flushed on CR3 reload.
    pub global: bool,

    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,

    /// **PAT** (Page Attribute Table) selector for 2 MiB mappings (bit 12).
    pub pat_large: bool,

    /// Reserved (bits 13..20): must be 0.
    #[bits(8)]
    __res13_20: u8,

    /// Physical address bits **51:21** (2 MiB-aligned base).
    #[bits(31)]
    phys_addr_51_21: u32,

    /// OS-available high (bits 52..58).
    #[bits(7)]
    pub os_available_high: u8,

    /// Protection Key / OS use (59..62).
    #[bits(4)]
    pub protection_key: u8,

    /// No-Execute (bit 63).
    pub no_execute: bool,
}

impl Pde2M {
    /// Set the 2 MiB page base (must be 2 MiB-aligned).
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        debug_assert!(phys.is_aligned_to(1 << 21));
        self.set_phys_addr_51_21((phys.as_u64() >> 21) as u32);
        self.set_page_size(true);
    }

    /// Get the 2 MiB page base.
    #[inline]
    pub const fn physical_address(self) -> PhysicalAddress {
        PhysicalAddress::new((self.phys_addr_51_21() as u64) << 21)
    }
}

/* ========================= L1: PTE (4 KiB leaf) =========================== */

/// L1 **PTE (4 KiB leaf)** — maps a single 4 KiB page (bit 7 is **PAT**).
///
/// - Physical address uses bits **51:12** and must be **4 KiB aligned**.
/// - The three PAT selector bits are **PWT (bit 3)**, **PCD (bit 4)**,
///   and **PAT (bit 7)**.
#[bitfield(u64)]
pub struct Pte4K {
    /// Present (bit 0).
    pub present: bool,
    /// Writable (bit 1).
    pub writable: bool,
    /// User (bit 2).
    pub user: bool,
    /// Write-Through (bit 3) — **PAT selector bit 0**.
    pub write_through: bool,
    /// Cache Disable (bit 4) — **PAT selector bit 1**.
    pub cache_disable: bool,
    /// Accessed (bit 5).
    pub accessed: bool,
    /// Dirty (bit 6): set by CPU on first write.
    pub dirty: bool,

    /// **PAT** (bit 7) — **PAT selector bit 2** for 4 KiB mappings.
    pub pat_small: bool,

    /// Global (bit 8): TLB entry not flushed on CR3 reload.
    pub global: bool,

    /// OS-available low (bits 9..11).
    #[bits(3)]
    pub os_available_low: u8,

    /// Physical address bits **51:12** (4 KiB-aligned base).
    #[bits(40)]
    phys_addr_51_12: u64,

    /// OS-available high (bits 52..58).
    #[bits(7)]
    pub os_available_high: u8,

    /// Protection Key / OS use (59..62).
    #[bits(4)]
    pub protection_key: u8,

    /// No-Execute (bit 63).
    pub no_execute: bool,
}

impl Pte4K {
    /// Set the 4 KiB page base (4 KiB-aligned).
    #[inline]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        debug_assert!(phys.is_aligned_to(0x1000));
        self.set_phys_addr_51_12(phys.as_u64() >> 12);
    }

    /// Get the 4 KiB page base.
    #[inline]
    pub const fn physical_address(self) -> PhysicalAddress {
        PhysicalAddress::new(self.phys_addr_51_12() << 12)
    }
}

/* ──────────────────────── Unions + type-safe viewers ─────────────────────── */

/// **L3 PDPTE union** — overlays non-leaf [`Pdpte`] and leaf [`Pdpte1G`]
/// on the same 64-bit storage.
///
/// Use [`PdpteUnion::view`] / [`PdpteUnion::view_mut`] to obtain a **typed**
/// reference. These methods inspect the **PS** bit to decide which variant is
/// active and return a safe borrowed view.
///
/// Storing/retrieving raw bits is possible via `from_bits`/`into_bits`.
#[derive(Copy, Clone)]
#[repr(C)]
pub union PdpteUnion {
    /// Raw 64-bit storage of the entry.
    bits: u64,
    /// Non-leaf form: next-level Page Directory (PS=0).
    entry: Pdpte,
    /// Leaf form: 1 GiB mapping (PS=1).
    leaf_1g: Pdpte1G,
}

impl PdpteUnion {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    #[inline]
    #[must_use]
    pub const fn new_entry(entry: Pdpte) -> Self {
        Self { entry }
    }

    #[inline]
    #[must_use]
    pub const fn new_leaf(leaf: Pdpte1G) -> Self {
        Self { leaf_1g: leaf }
    }

    #[inline]
    #[must_use]
    pub const fn present(self) -> bool {
        unsafe { self.bits & PRESENT_BIT != 0 }
    }
}

/// **L2 PDE union** — overlays non-leaf [`Pde`] and leaf [`Pde2M`]
/// on the same 64-bit storage.
///
/// Prefer [`PdeUnion::view`] / [`PdeUnion::view_mut`] for safe typed access.
/// These check the **PS** bit and hand you the correct variant.
#[derive(Copy, Clone)]
#[repr(C)]
pub union PdeUnion {
    /// Raw 64-bit storage of the entry.
    bits: u64,
    /// Non-leaf form: next-level Page Table (PS=0).
    entry: Pde,
    /// Leaf form: 2 MiB mapping (PS=1).
    leaf_2m: Pde2M,
}

impl PdeUnion {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    #[inline]
    #[must_use]
    pub const fn new_entry(entry: Pde) -> Self {
        Self { entry }
    }

    #[inline]
    #[must_use]
    pub const fn new_leaf(leaf: Pde2M) -> Self {
        Self { leaf_2m: leaf }
    }

    pub const fn present(self) -> bool {
        unsafe { self.bits & PRESENT_BIT != 0 }
    }
}

/// **Borrowed view** into an L3 PDPTE.
///
/// Returned by [`PdpteUnion::view`].
pub enum L3View<'a> {
    /// Non-leaf PDPTE view (PS=0).
    Entry(&'a Pdpte),
    /// 1 GiB leaf PDPTE view (PS=1).
    Leaf1G(&'a Pdpte1G),
}

/// **Borrowed view** into an L2 PDE.
///
/// Returned by [`PdeUnion::view`].
pub enum L2View<'a> {
    /// Non-leaf PDE view (PS=0).
    Entry(&'a Pde),
    /// 2 MiB leaf PDE view (PS=1).
    Leaf2M(&'a Pde2M),
}

impl PdpteUnion {
    /// Construct union from raw `bits` (no validation).
    #[inline]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Extract raw `bits` back from the union.
    #[inline]
    pub const fn into_bits(self) -> u64 {
        unsafe { self.bits }
    }

    /// **Typed read-only view** chosen by the **PS** bit.
    ///
    /// - If PS=1 → [`L3View::Leaf1G`]
    /// - If PS=0 → [`L3View::Entry`]
    ///
    /// This function is safe: it returns a view consistent with the PS bit.
    #[inline]
    pub const fn view(&self) -> L3View<'_> {
        unsafe {
            if (self.bits & PS_BIT) != 0 {
                L3View::Leaf1G(&self.leaf_1g)
            } else {
                L3View::Entry(&self.entry)
            }
        }
    }
}

impl PdeUnion {
    /// Construct union from raw `bits` (no validation).
    #[inline]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Extract raw `bits` back from the union.
    #[inline]
    pub const fn into_bits(self) -> u64 {
        unsafe { self.bits }
    }

    /// **Typed read-only view** chosen by the **PS** bit.
    ///
    /// - If PS=1 → [`L2View::Leaf2M`]
    /// - If PS=0 → [`L2View::Entry`]
    #[inline]
    pub const fn view(&self) -> L2View<'_> {
        unsafe {
            if (self.bits & PS_BIT) != 0 {
                L2View::Leaf2M(&self.leaf_2m)
            } else {
                L2View::Entry(&self.entry)
            }
        }
    }
}

/* ───────────────────────── Convenience constructors ─────────────────────── */

impl Pml4e {
    /// Convenience constructor for a typical **kernel RW, supervisor** entry.
    ///
    /// Sets: `present`, `writable`, clears `user`, `write_through`, `cache_disable`, `no_execute`.
    #[inline]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user(false)
            .with_write_through(false)
            .with_cache_disable(false)
            .with_no_execute(false)
    }
}

impl Pdpte {
    /// Non-leaf PDPTE with common kernel RW flags.
    #[inline]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user(false)
            .with_write_through(false)
            .with_cache_disable(false)
            .with_no_execute(false)
    }
}

impl Pdpte1G {
    /// Leaf PDPTE with common kernel RW flags.
    #[inline]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user(false)
            .with_write_through(false)
            .with_cache_disable(false)
            .with_no_execute(false)
            .with_page_size(true)
    }
}

impl Pde {
    /// Non-leaf PDE with common kernel RW flags.
    #[inline]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user(false)
            .with_write_through(false)
            .with_cache_disable(false)
            .with_no_execute(false)
    }
}

impl Pde2M {
    /// Leaf PDE with common kernel RW flags.
    #[inline]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user(false)
            .with_write_through(false)
            .with_cache_disable(false)
            .with_no_execute(false)
            .with_page_size(true)
    }
}

impl Pte4K {
    /// 4 KiB **user RX** mapping (read+exec, no write).
    #[inline]
    pub const fn new_user_rx() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(false)
            .with_user(true)
            .with_write_through(false)
            .with_cache_disable(false)
            .with_no_execute(false)
    }

    /// 4 KiB **user RO+NX** mapping (read-only, no execute).
    #[inline]
    pub const fn new_user_ro_nx() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(false)
            .with_user(true)
            .with_no_execute(true)
    }
}
