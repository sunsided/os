//! # Model-Specific Registers (MSR) utilities
//!
//! This module provides low-level access to CPU **Model-Specific Registers (MSRs)**,
//! particularly those related to segment base addresses used in 64-bit mode.
//!
//! ## Background
//! The x86-64 architecture provides the **GS** and **FS** segment registers as
//! general-purpose base registers for thread-local or per-CPU data. Their actual
//! 64-bit base addresses are stored in model-specific registers, accessible via
//! the privileged `RDMSR` and `WRMSR` instructions.
//!
//! Commonly used MSRs include:
//! - `IA32_GS_BASE` (`0xC000_0101)`: current GS base address used by `mov %gs:...`
//! - `IA32_KERNEL_GS_BASE` (`0xC000_0102)`: swap value used when executing `swapgs`
//!
//! The kernel typically uses `IA32_KERNEL_GS_BASE` to store the **kernel-side GS base**
//! (e.g., a pointer to per-CPU data), while userland may use `IA32_GS_BASE` for TLS.
//!
//! On a transition from user to kernel (e.g., via `syscall` or interrupt), the CPU
//! executes `swapgs`, which exchanges the contents of `IA32_GS_BASE` and
//! `IA32_KERNEL_GS_BASE`, giving the kernel immediate access to its per-CPU data.
//!
//! ## References
//! - Intel SDM Vol. 3, §2.5.4 “FS and GS Base Address Registers”
//! - AMD64 Architecture Programmer’s Manual Vol. 2, §4.8.3 “MSRs for FS/GS Base”

mod ia32_gs_base;
mod ia32_kernel_gs_base;
mod ia32_star;

pub use crate::msr::ia32_gs_base::Ia32GsBaseMsrExt;
pub use crate::msr::ia32_kernel_gs_base::Ia32KernelGsBaseMsrExt;
use crate::per_cpu::PerCpu;
use kernel_registers::msr::{Ia32GsBaseMsr, Ia32KernelGsBaseMsr};

/// Convenience: initialize both GS bases to the same per-CPU pointer
/// (common during early boot so a later `swapgs` is a no-op).
///
/// # Safety
/// Same as [`set_gs_base`](Ia32GsBaseMsr::set_gs_base) and [`set_kernel_gs_base`](Ia32KernelGsBaseMsr::set_kernel_gs_base).
#[inline]
pub unsafe fn init_gs_bases(percpu: &PerCpu) {
    unsafe {
        <Ia32GsBaseMsr as Ia32GsBaseMsrExt>::set_gs_base(percpu);
        <Ia32KernelGsBaseMsr as Ia32KernelGsBaseMsrExt>::set_kernel_gs_base(percpu); // for SWAPGS
    }
}

#[inline(always)]
#[allow(clippy::inline_always)]
const fn is_canonical(addr: u64) -> bool {
    // Canonical if bits 63..48 are all copies of bit 47.
    let sign = (addr >> 47) & 1;
    (addr >> 48) == if sign == 0 { 0 } else { 0xFFFF }
}
