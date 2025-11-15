//! Provides the [`Ia32GsBaseMsrExt`] trait.

use crate::per_cpu::PerCpu;
use core::ptr::NonNull;
use kernel_registers::msr::{Ia32GsBaseMsr, is_canonical_gs_base};
use kernel_registers::{LoadRegisterUnsafe, StoreRegisterUnsafe};

pub trait Ia32GsBaseMsrExt {
    /// Set the *current* GS base (used by `gs:` memory references).
    ///
    /// # Safety
    /// - CPL0 only; WRMSR at CPL>0 traps.
    /// - `base` must be a valid, canonical virtual address mapped for the intended use.
    /// - Changing GS base while concurrently using `gs:` can race; coordinate on SMP.
    unsafe fn set_gs_base(percpu: &PerCpu);

    /// Get the [`PerCpu`] pointer from the current [`IA32_GS_BASE`](Ia32GsBaseMsr::IA32_GS_BASE).
    unsafe fn read_ptr() -> *const PerCpu;

    /// Get the [`PerCpu`] reference from the current [`IA32_GS_BASE`](Ia32GsBaseMsr::IA32_GS_BASE).
    #[doc(alias = "gs_base_ptr")]
    unsafe fn current() -> &'static PerCpu;
}

impl Ia32GsBaseMsrExt for Ia32GsBaseMsr {
    /// Set the *current* GS base (used by `gs:` memory references).
    ///
    /// # Safety
    /// - CPL0 only; WRMSR at CPL>0 traps.
    /// - `base` must be a valid, canonical virtual address mapped for the intended use.
    /// - Changing GS base while concurrently using `gs:` can race; coordinate on SMP.
    #[inline]
    unsafe fn set_gs_base(percpu: &PerCpu) {
        let base = NonNull::from_ref(percpu);
        let addr = base.as_ptr() as u64;
        debug_assert!(
            is_canonical_gs_base(addr),
            "non-canonical GS base: {addr:#x}"
        );

        unsafe {
            Self::load_unsafe().with_gs_base(base).store_unsafe();
        }
    }

    /// Get the [`PerCpu`] pointer from the current [`IA32_GS_BASE`](Ia32GsBaseMsr::IA32_GS_BASE).
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn read_ptr() -> *const PerCpu {
        let msr = unsafe { Self::load_unsafe() };
        msr.ptr() as *const PerCpu
    }

    /// Get the [`PerCpu`] reference from the current [`IA32_GS_BASE`](Ia32GsBaseMsr::IA32_GS_BASE).
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn current() -> &'static PerCpu {
        let ptr = unsafe { Self::read_ptr() };
        debug_assert!(!ptr.is_null(), "Per-CPU instance pointer is unset");
        unsafe { &*ptr }
    }
}
