//! Provides the [`Ia32GsBaseMsr`] type.

use crate::msr::{Msr, is_canonical_gs_base};
use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;
use core::ptr::NonNull;

/// Model-Specific Register: current **GS base address**.
///
/// The CPU uses this value when resolving memory references through the GS
/// segment register (`mov %gs:offset, ...` or `mov ..., %gs:offset`).
///
/// In 64-bit mode, this value is 64 bits wide and read/writable through
/// `RDMSR`/`WRMSR` at index `0xC000_0101`.
#[bitfield(u64, order = Lsb)]
pub struct Ia32GsBaseMsr {
    #[bits(64)]
    #[doc(alias = "kernel_gs_base_ptr")]
    pub ptr: u64,
}

impl Ia32GsBaseMsr {
    pub const IA32_GS_BASE: u32 = 0xC000_0101;
    pub const MSR: Msr = Msr::new(Self::IA32_GS_BASE);

    /// Set the *current* GS base (used by `gs:` memory references).
    ///
    /// # Safety
    /// - CPL0 only; WRMSR at CPL>0 traps.
    /// - `base` must be a valid, canonical virtual address mapped for the intended use.
    /// - Changing GS base while concurrently using `gs:` can race; coordinate on SMP.
    #[inline]
    #[must_use]
    pub fn with_gs_base<T>(self, base: NonNull<T>) -> Self {
        let addr = base.as_ptr() as u64;
        debug_assert!(
            is_canonical_gs_base(addr),
            "non-canonical GS base: {addr:#x}"
        );
        self.with_ptr(addr)
    }
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Ia32GsBaseMsr {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn load_unsafe() -> Self {
        let msr = unsafe { Self::MSR.load_raw() };
        Self::from_bits(msr)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Ia32GsBaseMsr {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn store_unsafe(self) {
        unsafe { Self::MSR.store_raw(self.into_bits()) }
    }
}
