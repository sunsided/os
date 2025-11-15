use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;
use kernel_memory_addresses::PhysicalAddress;

/// CR3 — Page-Map Level-4 Base Register (IA-32e, PCID disabled).
///
/// Holds the physical base address of the PML4 table and cache-control flags
/// for PML4 walks. Assumes standard 4 KiB alignment and no PCID (CR4.PCIDE = 0).
#[bitfield(u64)]
pub struct Cr3 {
    /// Bits 0–2 — Reserved (must be 0).
    #[bits(3)]
    pub reserved0: u8,

    /// Bit 3 — PWT: Page-level Write-Through for PML4.
    ///
    /// Controls write-through vs write-back caching when accessing the PML4
    /// via CR3.
    pub pwt: bool,

    /// Bit 4 — PCD: Page-level Cache Disable for PML4.
    ///
    /// When set, disables caching for PML4 accesses.
    pub pcd: bool,

    /// Bits 5–11 — Reserved (must be 0 when written).
    #[bits(7)]
    pub reserved1: u8,

    /// Bits 12–51 — PML4 physical base >> 12.
    ///
    /// These bits store the physical base address of the PML4 table, shifted
    /// right by 12 (4 KiB alignment). To get the full physical address:
    /// `pml4_base_phys = pml4_base_4k << 12`.
    #[bits(40)]
    pml4_base_4k: u64,

    /// Bits 52–63 — Reserved.
    #[bits(12)]
    pub reserved2: u16,
}

impl Cr3 {
    /// Create a `Cr3` value from a PML4 physical base address and flags.
    ///
    /// `pml4_phys` must be 4 KiB-aligned.
    #[must_use]
    pub fn from_pml4_phys(pml4_phys: PhysicalAddress, pwt: bool, pcd: bool) -> Self {
        debug_assert_eq!(
            pml4_phys.as_u64() & 0xFFF,
            0,
            "PML4 base must be 4K-aligned"
        );
        let mut cr3 = Self::new();
        cr3.set_pwt(pwt);
        cr3.set_pcd(pcd);
        cr3.set_pml4_base_4k(pml4_phys.as_u64() >> 12);
        cr3
    }

    /// Return the full physical address of the PML4 base.
    #[must_use]
    pub fn pml4_phys(&self) -> PhysicalAddress {
        // In 4- and 5-level paging, CR3[51:12] is the base. Upper bits should be zero.
        let bits = self.into_bits();
        debug_assert_eq!(bits >> 52, 0, "CR3 has nonzero high bits: {bits:#018x}");

        PhysicalAddress::new(self.pml4_base_4k() << 12)
    }
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Cr3 {
    unsafe fn load_unsafe() -> Self {
        let mut cr3: u64;
        unsafe {
            core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
        }
        Self::from_bits(cr3)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Cr3 {
    #[allow(clippy::cast_precision_loss)]
    unsafe fn store_unsafe(self) {
        let cr3 = self.into_bits();
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) cr3, options(nostack, preserves_flags));
        }
    }
}
