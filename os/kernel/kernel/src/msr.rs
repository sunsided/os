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
//! - `IA32_GS_BASE` (0xC000_0101): current GS base address used by `mov %gs:...`
//! - `IA32_KERNEL_GS_BASE` (0xC000_0102): swap value used when executing `swapgs`
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

use crate::per_cpu::PerCpu;
use core::ptr::NonNull;

/// Identifies a **Model-Specific Register (MSR)** by its architectural index.
///
/// MSR indices are 32-bit identifiers used by the `rdmsr` and `wrmsr`
/// instructions to select which internal CPU register to access.
/// The index space is architecture-defined; see the Intel/AMD manuals for details.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Msr(pub u32);

impl Msr {
    /// Model-Specific Register: current **GS base address**.
    ///
    /// The CPU uses this value when resolving memory references through the GS
    /// segment register (`mov %gs:offset, ...` or `mov ..., %gs:offset`).
    ///
    /// In 64-bit mode, this value is 64 bits wide and read/writable through
    /// `RDMSR`/`WRMSR` at index `0xC000_0101`.
    pub const IA32_GS_BASE: Msr = Msr::new(0xC000_0101);

    /// Model-Specific Register: **kernel GS base**.
    ///
    /// This MSR holds the *alternate* GS base used by the CPU after executing the
    /// `swapgs` instruction. It allows the kernel to maintain its own GS base
    /// independently of userland.
    ///
    /// On `swapgs`, the CPU atomically swaps the contents of
    /// `IA32_GS_BASE` and `IA32_KERNEL_GS_BASE`.
    pub const IA32_KERNEL_GS_BASE: Msr = Msr::new(0xC000_0102);

    /// Creates a new `Msr` from a raw index.
    #[inline(always)]
    const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the underlying raw MSR index.
    #[inline(always)]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Write a 64-bit value to the given **Model-Specific Register (MSR)**.
///
/// # Parameters
/// - `msr`: the MSR index (e.g., [`IA32_GS_BASE`]).
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
/// # Example
/// ```no_run
/// use crate::arch::x86_64::msr::{write_msr, IA32_GS_BASE};
///
/// unsafe {
///     // Point GS base to the current per-CPU structure.
///     write_msr(IA32_GS_BASE, per_cpu_ptr as u64);
/// }
/// ```
///
/// # See Also
/// - [`IA32_GS_BASE`]
/// - [`IA32_KERNEL_GS_BASE`]
#[inline]
unsafe fn write_model_specific_register(msr: Msr, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    let msr = msr.raw();
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
pub unsafe fn read_model_specific_register(msr: Msr) -> u64 {
    let lo: u32;
    let hi: u32;
    let ecx = msr.raw();
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") ecx, out("eax") lo, out("edx") hi, options(nomem, nostack));
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Get the [`PerCpu`] pointer from the current [`IA32_GS_BASE`](Msr::IA32_GS_BASE).
#[inline(always)]
pub fn gs_base_ptr() -> *const PerCpu {
    unsafe { read_model_specific_register(Msr::IA32_GS_BASE) as *const PerCpu }
}

/// Get the [`PerCpu`] pointer from the current [`IA32_GS_BASE`](Msr::IA32_GS_BASE).
#[inline(always)]
#[allow(dead_code)]
pub fn kernel_gs_base_ptr() -> *const PerCpu {
    unsafe { read_model_specific_register(Msr::IA32_KERNEL_GS_BASE) as *const PerCpu }
}

/// Convenience: initialize both GS bases to the same per-CPU pointer
/// (common during early boot so a later `swapgs` is a no-op).
///
/// # Safety
/// Same as [`set_gs_base`] and [`set_kernel_gs_base`].
#[inline]
pub unsafe fn init_gs_bases(percpu: &PerCpu) {
    let percpu = NonNull::from_ref(percpu);
    unsafe {
        set_gs_base(percpu);
        set_kernel_gs_base(percpu); // for SWAPGS
    }
}

/// Set the *current* GS base (used by `gs:` memory references).
///
/// # Safety
/// - CPL0 only; WRMSR at CPL>0 traps.
/// - `base` must be a valid, canonical virtual address mapped for the intended use.
/// - Changing GS base while concurrently using `gs:` can race; coordinate on SMP.
#[inline]
pub unsafe fn set_gs_base<T>(base: NonNull<T>) {
    let addr = base.as_ptr() as u64;
    debug_assert!(is_canonical(addr), "non-canonical GS base: {addr:#x}");
    unsafe {
        write_model_specific_register(Msr::IA32_GS_BASE, addr);
    }
}

/// Set the *kernel* GS base that becomes active after `swapgs`.
///
/// # Safety
/// - CPL0 only; WRMSR at CPL>0 traps.
/// - `base` must be a valid, canonical virtual address to kernel per-CPU (or similar).
/// - Ensure your `swapgs` usage matches your entry/exit path expectations.
#[inline]
pub unsafe fn set_kernel_gs_base<T>(base: NonNull<T>) {
    let addr = base.as_ptr() as u64;
    debug_assert!(
        is_canonical(addr),
        "non-canonical KERNEL_GS base: {addr:#x}"
    );
    unsafe {
        write_model_specific_register(Msr::IA32_KERNEL_GS_BASE, addr);
    }
}

#[inline(always)]
const fn is_canonical(addr: u64) -> bool {
    // Canonical if bits 63..48 are all copies of bit 47.
    let sign = (addr >> 47) & 1;
    (addr >> 48) == if sign == 0 { 0 } else { 0xFFFF }
}
