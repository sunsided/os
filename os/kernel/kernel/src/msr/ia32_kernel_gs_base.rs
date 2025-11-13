//! Provides the [`Ia32KernelGsBaseMsr`] type.

use crate::msr::{Msr, is_canonical};
use crate::per_cpu::PerCpu;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

/// Model-Specific Register: **kernel GS base**.
///
/// This MSR holds the *alternate* GS base used by the CPU after executing the
/// `swapgs` instruction. It allows the kernel to maintain its own GS base
/// independently of userland.
///
/// On `swapgs`, the CPU atomically swaps the contents of
/// `IA32_GS_BASE` and `IA32_KERNEL_GS_BASE`.
pub struct Ia32KernelGsBaseMsr(Msr);

impl Ia32KernelGsBaseMsr {
    pub const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;

    pub const fn new() -> Self {
        Self(Msr::new(Self::IA32_KERNEL_GS_BASE))
    }

    /// Set the *kernel* GS base that becomes active after `swapgs`.
    ///
    /// # Safety
    /// - CPL0 only; WRMSR at CPL>0 traps.
    /// - `base` must be a valid, canonical virtual address to kernel per-CPU (or similar).
    /// - Ensure your `swapgs` usage matches your entry/exit path expectations.
    #[inline]
    pub unsafe fn set_kernel_gs_base(percpu: &PerCpu) {
        let base = NonNull::from_ref(percpu);
        let addr = base.as_ptr() as u64;
        debug_assert!(
            is_canonical(addr),
            "non-canonical KERNEL_GS base: {addr:#x}"
        );
        unsafe {
            Self::new().write_raw(addr);
        }
    }

    /// Get the [`PerCpu`] pointer from the current [`IA32_KERNEL_GS_BASE`](Self::IA32_KERNEL_GS_BASE).
    #[inline(always)]
    #[allow(clippy::inline_always)]
    #[allow(dead_code)]
    #[doc(alias = "kernel_gs_base_ptr")]
    pub fn read_ptr() -> *const PerCpu {
        unsafe { Self::new().read_raw() as *const PerCpu }
    }
}

impl Default for Ia32KernelGsBaseMsr {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Ia32KernelGsBaseMsr {
    type Target = Msr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Ia32KernelGsBaseMsr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
