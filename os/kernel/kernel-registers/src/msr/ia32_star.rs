use crate::msr::Msr;
use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;

/// `IA32_STAR` — System Call Target & Segment Selectors (MSR `0xC000_0081`).
///
/// In 64-bit mode:
///
/// - `syscall` uses `IA32_STAR[47:32]` to derive kernel CS/SS.
/// - `sysret` uses `IA32_STAR[63:48]` to derive user CS/SS.
/// - `IA32_LSTAR` provides the 64-bit RIP target for `syscall`.
///
/// In compatibility mode, `IA32_STAR[31:0]` holds the 32-bit EIP target for `syscall`.
#[bitfield(u64)]
pub struct Ia32Star {
    /// Bits 0–31 — Compatibility-mode `syscall` EIP.
    ///
    /// Used only when executing `syscall` from IA-32e compatibility mode.
    /// Ignored for 64-bit `syscall`, which uses `IA32_LSTAR`.
    #[bits(32, access = RO)]
    pub compat_syscall_eip: u32,

    /// Bits 32–47 — Kernel code segment selector base for `syscall`.
    ///
    /// On `syscall` in 64-bit mode:
    /// ```text
    ///   CS ← (this & 0xFFFC)
    ///   SS ← (this + 8)
    /// ```
    ///
    /// You typically store your kernel CS selector here (e.g. `KERNEL_CS_SEL`, `0x08`).
    #[bits(16)]
    pub syscall_cs_selector: u16,

    /// Bits 48–63 — User code segment selector base for `sysret`.
    ///
    /// On `sysret` in 64-bit mode:
    /// ```text
    ///   CS ← (this + 16) | 3
    ///   SS ← (this +  8) | 3
    /// ```
    ///
    /// You typically store your user CS selector here (e.g. `USER_CS_SEL`, `0x1b`).
    #[bits(16)]
    pub sysret_cs_selector: u16,
}

impl Ia32Star {
    /// MSR index for `IA32_STAR`.
    pub const IA32_STAR: u32 = 0xC000_0081;

    /// The MSR.
    pub const MSR: Msr = Msr::new(Self::IA32_STAR);

    /// Helper to build a STAR value for a pure 64-bit kernel.
    ///
    /// `kernel_cs` and `user_cs` are the *selectors* (e.g. `0x08` and `0x1b`).
    #[must_use]
    pub fn new_64bit_raw(kernel_cs: u16, user_cs: u16) -> Self {
        // Assumption:
        // kcode -> kdata -> udata -> ucode

        // Small helpers for raw selectors.
        #[inline]
        const fn gdt_index(sel: u16) -> u16 {
            sel >> 3
        }

        #[inline]
        const fn rpl(sel: u16) -> u16 {
            sel & 0b11
        }

        // Extract and validate GDT indices
        let kidx = gdt_index(kernel_cs);
        let uidx = gdt_index(user_cs);

        // SYSRET requires:
        //   user_ss_index = user_cs_index - 1
        // because SS uses (base + 8), CS uses (base + 16).
        debug_assert_ne!(kidx, 0);
        debug_assert_eq!(rpl(kernel_cs), 0, "kernel CS must be Ring0");
        debug_assert_ne!(uidx, 0, "User CS selector at GDT index 0 is invalid");

        let user_ss_index = uidx - 1;

        // Compute STAR base value for SYSRET
        //
        // STAR[63:48] = raw 16-bit base selector WITHOUT RPL bits.
        //
        //   SS = (base + 8 ) | 3  → must equal (user_ss_index << 3) | 3
        //   CS = (base + 16) | 3  → must equal (user_cs_index << 3) | 3
        //
        // Solve:
        //   base = (user_ss_index << 3) - 8
        //
        let base_no_rpl: u16 = (user_ss_index << 3).wrapping_sub(8);

        Self::new()
            // Hardware ignores the RPL bits for the syscall CS selector in STAR,
            // but storing the full selector is fine; only bits 15:3 matter.
            .with_syscall_cs_selector(kernel_cs)
            .with_sysret_cs_selector(base_no_rpl)
    }
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Ia32Star {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn load_unsafe() -> Self {
        let msr = unsafe { Self::MSR.load_raw() };
        Self::from_bits(msr)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Ia32Star {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn store_unsafe(self) {
        unsafe { Self::MSR.store_raw(self.into_bits()) }
    }
}
