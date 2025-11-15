//! Provides the [`Ia32KernelGsBaseMsr`] type.

use crate::msr::{Msr, is_canonical_gs_base};
use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;
use core::ptr::NonNull;

/// Model-Specific Register: **kernel GS base**.
///
/// This MSR holds the *alternate* GS base used by the CPU after executing the
/// `swapgs` instruction. It allows the kernel to maintain its own GS base
/// independently of userland.
///
/// On `swapgs`, the CPU atomically swaps the contents of
/// `IA32_GS_BASE` and `IA32_KERNEL_GS_BASE`.
#[bitfield(u64, order = Lsb)]
pub struct Ia32KernelGsBaseMsr {
    #[bits(64)]
    #[doc(alias = "kernel_gs_base_ptr")]
    pub ptr: u64,
}

impl Ia32KernelGsBaseMsr {
    pub const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;
    pub const MSR: Msr = Msr::new(Self::IA32_KERNEL_GS_BASE);

    /// Set the *kernel* GS base that becomes active after `swapgs`.
    ///
    /// # Safety
    /// - CPL0 only; WRMSR at CPL>0 traps.
    /// - `base` must be a valid, canonical virtual address to kernel per-CPU (or similar).
    /// - Ensure your `swapgs` usage matches your entry/exit path expectations.
    #[inline(always)]
    #[allow(clippy::inline_always)]
    #[must_use]
    pub fn with_kernel_gs_base<T>(self, base: NonNull<T>) -> Self {
        let addr = base.as_ptr() as u64;
        debug_assert!(
            is_canonical_gs_base(addr),
            "non-canonical KERNEL_GS base: {addr:#x}"
        );
        self.with_ptr(addr)
    }
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Ia32KernelGsBaseMsr {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn load_unsafe() -> Self {
        let msr = unsafe { Self::MSR.load_raw() };
        Self::from_bits(msr)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Ia32KernelGsBaseMsr {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn store_unsafe(self) {
        unsafe { Self::MSR.store_raw(self.into_bits()) }
    }
}
