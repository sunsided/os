use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;

/// `IA32_EFER` / EFER (MSR `0xC000_0080`).
///
/// Extended Feature Enable Register used for `SYSCALL`/`SYSRET`, long mode,
/// `NX`, and various AMD extensions.
#[bitfield(u64, order = Lsb)]
#[derive(Eq, PartialEq)]
pub struct Efer {
    /// Bit 0 — SCE: System Call Extensions.
    ///
    /// Enables SYSCALL/SYSRET when set.
    pub sce: bool,

    /// Bit 1 — DPE (AMD K6 only): Data Prefetch Enable.
    pub dpe: bool,

    /// Bit 2 — SEWBED (AMD K6 only): Speculative EWBE# Disable.
    pub sewbed: bool,

    /// Bit 3 — GEWBED (AMD K6 only): Global EWBE# Disable.
    pub gewbed: bool,

    /// Bit 4 — L2D (AMD K6 only): L2 Cache Disable.
    pub l2d: bool,

    /// Bits 5–7 — Reserved (read as zero, must be written as zero).
    #[bits(3)]
    pub reserved0: u8,

    /// Bit 8 — LME: Long Mode Enable.
    ///
    /// Enables IA-32e (long) mode when paging is enabled.
    pub lme: bool,

    /// Bit 9 — Reserved.
    #[bits(access = RO)]
    pub reserved1: bool,

    /// Bit 10 — LMA: Long Mode Active (read-only).
    ///
    /// Indicates that the processor is currently in long mode.
    pub lma: bool,

    /// Bit 11 — NXE: No-Execute Enable.
    ///
    /// Enables the NX bit in page tables.
    pub nxe: bool,

    /// Bit 12 — SVME: Secure Virtual Machine Enable (AMD SVM).
    pub svme: bool,

    /// Bit 13 — LMSLE: Long Mode Segment Limit Enable.
    pub lmsle: bool,

    /// Bit 14 — FFXSR: Fast FXSAVE/FXRSTOR.
    pub ffxsr: bool,

    /// Bit 15 — TCE: Translation Cache Extension.
    pub tce: bool,

    /// Bit 16 — Reserved.
    pub reserved2: bool,

    /// Bit 17 — MCOMMIT: MCOMMIT instruction enable (AMD).
    pub mcommit: bool,

    /// Bit 18 — INTWB: Interruptible WBINVD/WBNOINVD enable (AMD).
    pub intwb: bool,

    /// Bit 19 — Reserved.
    pub reserved3: bool,

    /// Bit 20 — UAIE: Upper Address Ignore Enable.
    pub uaie: bool,

    /// Bit 21 — AIBRSE: Automatic IBRS Enable.
    pub aibrse: bool,

    /// Bits 22–63 — Reserved.
    #[bits(42, access = RO)]
    pub reserved4: u64,
}

impl Efer {
    /// MSR index for `IA32_EFER` / `EFER`.
    pub const MSR_EFER: u32 = 0xC000_0080;
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Efer {
    unsafe fn load_unsafe() -> Self {
        let (mut lo, mut hi): (u32, u32);
        unsafe {
            core::arch::asm!(
                "rdmsr",
                in("ecx") Self::MSR_EFER,
                out("eax") lo,
                out("edx") hi,
                options(nomem, preserves_flags)
            );
        }
        let efer = u64::from(hi) << 32 | u64::from(lo);
        Self::from_bits(efer)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Efer {
    #[allow(clippy::cast_precision_loss)]
    unsafe fn store_unsafe(self) {
        let efer = self.into_bits();
        let lo = efer as u32;
        let hi = (efer >> 32) as u32;
        unsafe {
            core::arch::asm!(
                "wrmsr",
                in("ecx") Self::MSR_EFER,
                in("eax") lo,
                in("edx") hi,
                options(nomem, preserves_flags)
            );
        }
    }
}
