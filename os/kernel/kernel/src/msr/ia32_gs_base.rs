//! Provides the [`Ia32GsBaseMsr`] type.

use crate::msr::{Msr, is_canonical};
use crate::per_cpu::PerCpu;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

/// Model-Specific Register: current **GS base address**.
///
/// The CPU uses this value when resolving memory references through the GS
/// segment register (`mov %gs:offset, ...` or `mov ..., %gs:offset`).
///
/// In 64-bit mode, this value is 64 bits wide and read/writable through
/// `RDMSR`/`WRMSR` at index `0xC000_0101`.
pub struct Ia32GsBaseMsr(Msr);

impl Ia32GsBaseMsr {
    pub const IA32_GS_BASE: u32 = 0xC000_0101;

    pub const fn new() -> Self {
        Self(Msr::new(Self::IA32_GS_BASE))
    }

    /// Set the *current* GS base (used by `gs:` memory references).
    ///
    /// # Safety
    /// - CPL0 only; WRMSR at CPL>0 traps.
    /// - `base` must be a valid, canonical virtual address mapped for the intended use.
    /// - Changing GS base while concurrently using `gs:` can race; coordinate on SMP.
    #[inline]
    pub unsafe fn set_gs_base(percpu: &PerCpu) {
        let base = NonNull::from_ref(percpu);
        let addr = base.as_ptr() as u64;
        debug_assert!(is_canonical(addr), "non-canonical GS base: {addr:#x}");
        unsafe {
            Self::new().write_raw(addr);
        }
    }

    /// Get the [`PerCpu`] pointer from the current [`IA32_GS_BASE`](Msr::IA32_GS_BASE).
    #[inline(always)]
    #[allow(clippy::inline_always)]
    #[doc(alias = "gs_base_ptr")]
    pub fn read_ptr() -> *const PerCpu {
        unsafe { Self::new().read_raw() as *const PerCpu }
    }

    /// Get the [`PerCpu`] reference from the current [`IA32_GS_BASE`](Msr::IA32_GS_BASE).
    #[inline(always)]
    #[allow(clippy::inline_always)]
    #[doc(alias = "gs_base_ptr")]
    pub fn current() -> &'static PerCpu {
        let ptr = Self::read_ptr();
        debug_assert!(!ptr.is_null(), "Per-CPU instance pointer is unset");
        unsafe { &*ptr }
    }
}

impl Default for Ia32GsBaseMsr {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Ia32GsBaseMsr {
    type Target = Msr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Ia32GsBaseMsr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
