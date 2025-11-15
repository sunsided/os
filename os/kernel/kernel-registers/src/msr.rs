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

mod ia32_fmask;
mod ia32_gs_base;
mod ia32_kernel_gs_base;
mod ia32_lstar;
mod ia32_star;

pub use ia32_fmask::Ia32Fmask;
pub use ia32_gs_base::Ia32GsBaseMsr;
pub use ia32_kernel_gs_base::Ia32KernelGsBaseMsr;
pub use ia32_lstar::Ia32LStar;
pub use ia32_star::Ia32Star;

/// Identifies a **Model-Specific Register (MSR)** by its architectural index.
///
/// MSR indices are 32-bit identifiers used by the `rdmsr` and `wrmsr`
/// instructions to select which internal CPU register to access.
/// The index space is architecture-defined; see the Intel/AMD manuals for details.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Msr(pub u32);

impl Msr {
    /// Creates a new `Msr` from a raw index.
    #[inline(always)]
    #[allow(clippy::inline_always)]
    const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the underlying raw MSR index.
    #[inline(always)]
    #[allow(clippy::inline_always)]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Write a 64-bit value to the given **Model-Specific Register (MSR)**.
    ///
    /// # Parameters
    /// - `msr`: the MSR index (e.g., [`Ia32GsBaseMsr::IA32_GS_BASE`]).
    /// - `val`: the 64-bit value to write.
    ///
    /// # Safety
    /// - This function executes the privileged `WRMSR` instruction, which is only
    ///   valid at **CPL=0** (kernel mode). Executing this in user mode will raise a
    ///   **#GP(0)** exception.
    /// - The target MSR must be **valid and writable** on the current CPU.
    ///   Writing an invalid or reserved MSR causes a general protection fault.
    /// - Callers must ensure that interrupts and concurrent CPU accesses do not
    ///   interfere with the semantics of the written MSR (e.g., writing GS base
    ///   while `swapgs` is in flight).
    ///
    /// # See Also
    /// - [`Ia32GsBaseMsr::IA32_GS_BASE`]
    /// - [`Ia32KernelGsBaseMsr::IA32_KERNEL_GS_BASE`]
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    #[doc(alias = "write_model_specific_register")]
    pub unsafe fn store_raw(self, val: u64) {
        let lo = (val & 0xFFFF_FFFF) as u32;
        let hi = (val >> 32) as u32;
        let msr = self.raw();
        unsafe {
            core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") lo,
            in("edx") hi,
            options(nostack, preserves_flags)
            );
        }
    }

    /// Reads the 64-bit value from the given **Model-Specific Register (MSR)**.
    #[inline(always)]
    #[allow(clippy::inline_always)]
    #[doc(alias = "read_model_specific_register")]
    pub unsafe fn load_raw(self) -> u64 {
        let lo: u32;
        let hi: u32;
        let ecx = self.raw();
        unsafe {
            core::arch::asm!(
            "rdmsr",
            in("ecx") ecx,
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack, preserves_flags)
            );
        }
        (u64::from(hi) << 32) | u64::from(lo)
    }
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub const fn is_canonical_gs_base(addr: u64) -> bool {
    // Canonical if bits 63..48 are all copies of bit 47.
    let sign = (addr >> 47) & 1;
    (addr >> 48) == if sign == 0 { 0 } else { 0xFFFF }
}
