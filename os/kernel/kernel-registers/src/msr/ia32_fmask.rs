use crate::msr::Msr;
use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;

/// `IA32_FMASK` — `RFLAGS` Mask for `syscall` (MSR 0xC000_0084).
///
/// On `syscall`, the CPU computes:
/// ```text
///   RFLAGS := RFLAGS & ~IA32_FMASK
/// ```
///
/// Bits set here are *cleared* in RFLAGS on entry to the kernel.
#[bitfield(u64, order = Lsb)]
pub struct Ia32Fmask {
    /// Carry Flag mask (bit 0).
    ///
    /// Rarely masked — but if set, the kernel always enters with CF = 0.
    ///
    /// Corresponds to [`Rflags::cf_carry`](crate::rflags::Rflags::cf_carry).
    pub cf_carry: bool,

    /// Bit 1 — always 1 in RFLAGS, **must be 0 in FMASK**.
    ///
    /// FMASK may not mask architectural-constant bits.
    #[bits(default = false)]
    _bit1: bool,

    /// Parity Flag mask (bit 2).
    ///
    /// Masking this is uncommon but allowed.
    ///
    /// Corresponds to [`Rflags::pf_parity`](crate::rflags::Rflags::pf_parity).
    pub pf_parity: bool,

    /// Bit 3 — reserved, always 0 in RFLAGS; **must be 0 in FMASK**.
    #[bits(default = false)]
    _bit3: bool,

    /// Adjust Flag mask (bit 4).
    ///
    /// Rarely masked; clearing AF has little semantic effect in the kernel.
    ///
    /// Corresponds to [`Rflags::cf_carry`](crate::rflags::Rflags::af_adjust).
    pub af_adjust: bool,

    /// Bit 5 — reserved, always 0 in RFLAGS; **must be 0 in FMASK**.
    #[bits(default = false)]
    _bit5: bool,

    /// Zero Flag mask (bit 6).
    ///
    /// Normally left unmasked. Masking forces ZF = 0 on entry.
    ///
    /// Corresponds to [`Rflags::zf_zero`](crate::rflags::Rflags::zf_zero).
    pub zf_zero: bool,

    /// Sign Flag mask (bit 7).
    ///
    /// Normally left unmasked. Masking forces SF = 0 on entry.
    ///
    /// Corresponds to [`Rflags::sf_sign`](crate::rflags::Rflags::sf_sign).
    pub sf_sign: bool,

    /// Trap Flag mask (bit 8).
    ///
    /// **Set to 1** to prevent user from single-stepping into kernel entry.
    ///
    /// Very important for syscall entry safety.
    ///
    /// Corresponds to [`Rflags::tf_trap`](crate::rflags::Rflags::tf_trap).
    pub tf_trap: bool,

    /// Interrupt Enable Flag mask (bit 9).
    ///
    /// **Set to 1** so kernel always enters with interrupts disabled.
    ///
    /// This is the standard setup for syscall entry.
    ///
    /// Corresponds to [`Rflags::if_interrupt_enable`](crate::rflags::Rflags::if_interrupt_enable).
    pub if_interrupt_enable: bool,

    /// Direction Flag mask (bit 10).
    ///
    /// **Set to 1** so string ops always run forward (DF = 0) in kernel.
    ///
    /// Corresponds to [`Rflags::df_direction`](crate::rflags::Rflags::df_direction).
    pub df_direction: bool,

    /// Overflow Flag mask (bit 11).
    ///
    /// Rarely masked; typically left alone.
    ///
    /// Corresponds to [`Rflags::of_overflow`](crate::rflags::Rflags::of_overflow).
    pub of_overflow: bool,

    /// I/O Privilege Level mask (bits 12–13).
    ///
    /// Most kernels mask both bits so user IOPL does not leak into kernel mode.
    ///
    /// Corresponds to [`Rflags::iopl`](crate::rflags::Rflags::iopl).
    #[bits(2)]
    pub iopl: u8,

    /// Nested Task mask (bit 14).
    ///
    /// Set to 1 to ensure NT is cleared on entry.
    ///
    /// NT is obsolete but can cause weird interactions if preserved.
    ///
    /// Corresponds to [`Rflags::nt_nested`](crate::rflags::Rflags::nt_nested).
    pub nt_nested: bool,

    /// Bit 15 — reserved, always 0 in RFLAGS; **must be 0 in FMASK**.
    #[bits(default = false)]
    _bit15: bool,

    /// Resume Flag mask (bit 16).
    ///
    /// Masking is useful to avoid debugger interactions leaking into kernel.
    ///
    /// Corresponds to [`Rflags::rf_resume`](crate::rflags::Rflags::rf_resume).
    pub rf_resume: bool,

    /// Virtual-8086 Mode mask (bit 17).
    ///
    /// Must be **0** in long mode.
    /// FMASK must not set this bit.
    #[bits(default = false)]
    _vm: bool,

    /// Alignment Check mask (bit 18).
    ///
    /// Masking can be useful: AC behaves differently across rings.
    ///
    /// Corresponds to [`Rflags::ac_alignment_check`](crate::rflags::Rflags::ac_alignment_check).
    pub ac_alignment_check: bool,

    /// Virtual Interrupt Flag mask (bit 19).
    ///
    /// Set to 1 to ensure VIF is cleared; avoids virtualization contamination.
    ///
    /// Corresponds to [`Rflags::vif_virtual_interrupt`](crate::rflags::Rflags::vif_virtual_interrupt).
    pub vif_virtual_interrupt: bool,

    /// Virtual Interrupt Pending mask (bit 20).
    ///
    /// Set to 1 to clear VIP on entry.
    ///
    /// Corresponds to [`Rflags::vip_virtual_interrupt_pending`](crate::rflags::Rflags::vip_virtual_interrupt_pending).
    pub vip_virtual_interrupt_pending: bool,

    /// ID Flag mask (bit 21).
    ///
    /// If user toggled CPUID availability, kernel generally wants a clean state.
    ///
    /// Mask this if you want to enforce deterministic kernel behavior.
    ///
    /// Corresponds to [`Rflags::id_cpuid`](crate::rflags::Rflags::id_cpuid).
    pub id_cpuid: bool,

    /// Bits 22–63 — all reserved; **must be zero in FMASK**.
    #[bits(42, default = false)]
    _reserved_rest: u64,
}

impl Ia32Fmask {
    pub const IA32_FMASK: u32 = 0xC000_0084;
    pub const MSR: Msr = Msr::new(Self::IA32_FMASK);

    /// Reasonable default: clear TF/IF/DF/IOPL/NT/RF/VM/AC/VIF/VIP.
    pub const fn linux_like_default() -> Self {
        Self::new()
            .with_tf_trap(true)
            .with_if_interrupt_enable(true)
            .with_df_direction(true)
            .with_nt_nested(true)
            .with_rf_resume(true)
            .with_ac_alignment_check(true)
            .with_vif_virtual_interrupt(true)
            .with_vip_virtual_interrupt_pending(true)
    }
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Ia32Fmask {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn load_unsafe() -> Self {
        let msr = unsafe { Self::MSR.load_raw() };
        Self::from_bits(msr)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Ia32Fmask {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn store_unsafe(self) {
        unsafe { Self::MSR.store_raw(self.into_bits()) }
    }
}
