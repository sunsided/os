//! Provides the [`Ia32KernelGsBaseMsr`] type.

use crate::per_cpu::PerCpu;
use core::ptr::NonNull;
use kernel_registers::msr::{Ia32KernelGsBaseMsr, is_canonical_gs_base};
use kernel_registers::{LoadRegisterUnsafe, StoreRegisterUnsafe};

/// Model-Specific Register: **kernel GS base**.
///
/// This MSR holds the *alternate* GS base used by the CPU after executing the
/// `swapgs` instruction. It allows the kernel to maintain its own GS base
/// independently of userland.
///
/// On `swapgs`, the CPU atomically swaps the contents of
/// `IA32_GS_BASE` and `IA32_KERNEL_GS_BASE`.
pub trait Ia32KernelGsBaseMsrExt {
    /// Set the *kernel* GS base that becomes active after `swapgs`.
    ///
    /// # Safety
    /// - CPL0 only; WRMSR at CPL>0 traps.
    /// - `base` must be a valid, canonical virtual address to kernel per-CPU (or similar).
    /// - Ensure your `swapgs` usage matches your entry/exit path expectations.
    unsafe fn set_kernel_gs_base(percpu: &PerCpu);

    /// Get the [`PerCpu`] pointer from the current [`IA32_KERNEL_GS_BASE`](Ia32KernelGsBaseMsr::IA32_KERNEL_GS_BASE).
    #[allow(dead_code)]
    unsafe fn read_ptr() -> *const PerCpu;

    /// Get the [`PerCpu`] reference from the current [`IA32_GS_BASE`](Ia32KernelGsBaseMsr::IA32_KERNEL_GS_BASE).
    #[allow(dead_code)]
    #[doc(alias = "kernel_gs_base_ptr")]
    unsafe fn current() -> &'static PerCpu;
}

impl Ia32KernelGsBaseMsrExt for Ia32KernelGsBaseMsr {
    /// Set the *kernel* GS base that becomes active after `swapgs`.
    ///
    /// # Safety
    /// - CPL0 only; WRMSR at CPL>0 traps.
    /// - `base` must be a valid, canonical virtual address to kernel per-CPU (or similar).
    /// - Ensure your `swapgs` usage matches your entry/exit path expectations.
    #[inline]
    unsafe fn set_kernel_gs_base(percpu: &PerCpu) {
        let base = NonNull::from_ref(percpu);
        let addr = base.as_ptr() as u64;
        debug_assert!(
            is_canonical_gs_base(addr),
            "non-canonical KERNEL_GS base: {addr:#x}"
        );

        unsafe {
            Self::load_unsafe().with_kernel_gs_base(base).store_unsafe();
        }
    }

    /// Get the [`PerCpu`] pointer from the current [`IA32_KERNEL_GS_BASE`](Ia32KernelGsBaseMsr::IA32_KERNEL_GS_BASE).
    #[inline(always)]
    #[allow(clippy::inline_always)]
    #[allow(dead_code)]
    #[doc(alias = "kernel_gs_base_ptr")]
    unsafe fn read_ptr() -> *const PerCpu {
        let msr = unsafe { Self::load_unsafe() };
        msr.ptr() as *const PerCpu
    }

    /// Get the [`PerCpu`] reference from the current [`IA32_KERNEL_GS_BASE`](Ia32KernelGsBaseMsr::IA32_KERNEL_GS_BASE).
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn current() -> &'static PerCpu {
        let ptr = unsafe { Self::read_ptr() };
        debug_assert!(!ptr.is_null(), "Per-CPU instance pointer is unset");
        unsafe { &*ptr }
    }
}
