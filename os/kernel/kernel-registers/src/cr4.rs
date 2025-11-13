use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;

/// CR4 — Control Register 4 (x86-64).
///
/// Controls paging, extended instruction state management, and
/// various protection features (`UMIP`, `SMEP`/`SMAP`, `PKE`, ...).
///
/// Only the low bits are architecturally defined; the rest are reserved.
#[bitfield(u64, order = Lsb)]
pub struct Cr4 {
    /// Bit 0 — VME: Virtual-8086 Mode Extensions.
    pub vme: bool,

    /// Bit 1 — PVI: Protected-Mode Virtual Interrupts.
    pub pvi: bool,

    /// Bit 2 — TSD: Time Stamp Disable.
    ///
    /// When set, RDTSC/RDTSCP are privileged (CPL 0 only).
    pub tsd: bool,

    /// Bit 3 — DE: Debugging Extensions.
    pub de: bool,

    /// Bit 4 — PSE: Page Size Extensions.
    pub pse: bool,

    /// Bit 5 — PAE: Physical Address Extension.
    pub pae: bool,

    /// Bit 6 — MCE: Machine-Check Enable.
    pub mce: bool,

    /// Bit 7 — PGE: Page Global Enable.
    pub pge: bool,

    /// Bit 8 — PCE: Performance-Monitoring Counter Enable.
    pub pce: bool,

    /// Bit 9 — OSFXSR: OS supports FXSAVE/FXRSTOR.
    pub osfxsr: bool,

    /// Bit 10 — OSXMMEXCPT: OS supports unmasked SIMD FP exceptions.
    pub osxmmexcpt: bool,

    /// Bit 11 — UMIP: User-Mode Instruction Prevention.
    pub umip: bool,

    /// Bit 12 — LA57: 57-bit linear addresses (5-level paging).
    pub la57: bool,

    /// Bit 13 — VMXE: VMX Enable (Intel VT-x).
    pub vmxe: bool,

    /// Bit 14 — SMXE: SMX Enable.
    pub smxe: bool,

    /// Bit 15 — Reserved (must be 0).
    #[bits(access = RO)]
    pub reserved0: bool,

    /// Bit 16 — FSGSBASE: Enable {R,W}{D,}FSBASE/GSBASE in CPL > 0.
    pub fsgsbase: bool,

    /// Bit 17 — PCIDE: Process-Context Identifiers.
    pub pcide: bool,

    /// Bit 18 — OSXSAVE: OS uses XSAVE/XRSTOR and XCR0.
    pub osxsave: bool,

    /// Bit 19 — Reserved (must be 0 for current CPUs).
    #[bits(access = RO)]
    pub reserved1: bool,

    /// Bit 20 — SMEP: Supervisor Mode Execution Prevention.
    pub smep: bool,

    /// Bit 21 — SMAP: Supervisor Mode Access Prevention.
    pub smap: bool,

    /// Bit 22 — PKE: Protection Keys Enable.
    pub pke: bool,

    /// Bits 23–63 — Reserved.
    #[bits(41, access = RO)]
    pub reserved2: u64,
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Cr4 {
    unsafe fn load() -> Self {
        let mut cr4: u64;
        unsafe {
            core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, preserves_flags));
        }
        Self::from_bits(cr4)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Cr4 {
    #[allow(clippy::cast_precision_loss)]
    unsafe fn store(self) {
        let cr4 = self.into_bits();
        unsafe {
            core::arch::asm!("mov cr4, {}", in(reg) cr4, options(nomem, preserves_flags));
        }
    }
}
