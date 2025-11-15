use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;

/// Architectural model of CR0 in 64-bit mode.
///
/// Exposes all architecturally defined control bits as booleans
/// and keeps all reserved bits forced to 0. Upper 32 bits are
/// also modeled as reserved.
#[bitfield(u64)]
pub struct Cr0 {
    /// Bit 0 — Protection Enable (PE).
    ///
    /// - 0: Real mode (no paging, no protection).
    /// - 1: Protected mode (required for paging / long mode).
    pub pe_protection_enable: bool,

    /// Bit 1 — Monitor Coprocessor (MP).
    ///
    /// Controls interaction of WAIT/FWAIT with TS in CR0 for x87.
    pub mp_monitor_coprocessor: bool,

    /// Bit 2 — Emulation (EM).
    ///
    /// - 1: No x87 present; all x87 instructions fault.
    /// - 0: x87 instructions executed normally.
    pub em_emulation: bool,

    /// Bit 3 — Task Switched (TS).
    ///
    /// Set on task switch; used to manage x87/SSE lazy state.
    pub ts_task_switched: bool,

    /// Bit 4 — Extension Type (ET).
    ///
    /// Historically distinguished 287 vs 387; on modern CPUs this
    /// should be 1 and effectively behaves as a reserved bit.
    pub et_extension_type: bool,

    /// Bit 5 — Numeric Error (NE).
    ///
    /// - 1: x87 errors reported via exceptions (#MF).
    /// - 0: x87 errors signaled via external IRQ 13 (legacy).
    pub ne_numeric_error: bool,

    /// Bits 6–15 — Reserved (must be 0).
    ///
    /// Kept private and defaulted to 0.
    #[bits(10, default = 0)]
    _reserved_6_15: u16,

    /// Bit 16 — Write Protect (WP).
    ///
    /// When set, supervisor code must respect user/supervisor
    /// read-only pages; when clear, supervisor may write them.
    pub wp_write_protect: bool,

    /// Bit 17 — Reserved (must be 0).
    #[bits(default = 0)]
    _reserved_17: bool,

    /// Bit 18 — Alignment Mask (AM).
    ///
    /// With CR0.AM=1 and RFLAGS.AC=1, unaligned accesses in
    /// ring 3 may raise #AC.
    pub am_alignment_mask: bool,

    /// Bits 19–28 — Reserved (must be 0).
    #[bits(10, default = 0)]
    _reserved_19_28: u16,

    /// Bit 29 — Not-Write-Through (NW).
    ///
    /// Controls write-through behavior together with CD.
    pub nw_not_write_through: bool,

    /// Bit 30 — Cache Disable (CD).
    ///
    /// When set, disables caching (with caveats; usually used
    /// only during firmware / early bring-up).
    pub cd_cache_disable: bool,

    /// Bit 31 — Paging (PG).
    ///
    /// - 0: Paging disabled.
    /// - 1: Paging enabled (requires PE=1).
    pub pg_paging: bool,

    /// Bits 32–63 — Reserved (must be 0).
    #[bits(32, default = 0)]
    _reserved_32_63: u32,
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Cr0 {
    unsafe fn load_unsafe() -> Self {
        let mut cr0: u64;
        unsafe {
            core::arch::asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        }
        Self::from_bits(cr0)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Cr0 {
    #[allow(clippy::cast_precision_loss)]
    unsafe fn store_unsafe(self) {
        let cr3 = self.into_bits();
        unsafe {
            core::arch::asm!("mov cr0, {}", in(reg) cr3, options(nostack, preserves_flags));
        }
    }
}
