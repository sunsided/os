//! # CPUID Instruction Interface
//!
//! This module provides a comprehensive interface to the x86-64 CPUID instruction,
//! enabling the kernel to query processor capabilities, features, and identification
//! information. It offers both low-level raw access and high-level typed wrappers
//! for specific CPUID leaves.
//!
//! ## Overview
//!
//! The CPUID instruction returns processor information in the EAX, EBX, ECX, and EDX
//! registers based on the input leaf (EAX) and subleaf (ECX) values. This module
//! structures that information into safe, typed interfaces that make CPU feature
//! detection and processor identification straightforward.
//!
//! ## Architecture
//!
//! The module is organized around specific CPUID leaves:
//!
//! * **Leaf 00H** ([`CpuidRanges`]): Basic CPU information, vendor identification,
//!   and maximum supported leaf numbers for both basic and extended functions
//! * **Leaf 01H** ([`Leaf01h`]): Core feature flags, family/model/stepping info,
//!   and processor capabilities (SSE, AVX, x2APIC, etc.)
//! * **Leaf 15H** ([`Leaf15h`]): TSC (Time Stamp Counter) frequency information
//!   with crystal oscillator frequency and ratio calculations
//! * **Leaf 16H** ([`Leaf16`]): Processor frequency information including base,
//!   maximum, and bus frequencies (Intel advisory data)
//!
//! ## Key Features
//!
//! * **Vendor Detection**: Identifies Intel, AMD, and other CPU manufacturers
//! * **Feature Flags**: Comprehensive access to CPU capability bits (SSE, AVX, etc.)
//! * **Processor Info**: Family, model, stepping identification for CPU variants
//! * **Frequency Data**: TSC and processor frequency information for timing
//! * **x2APIC Support**: Detection of Extended APIC capabilities
//! * **Safe Wrappers**: Type-safe interfaces with automatic leaf availability checks
//!
//! ## Usage Patterns
//!
//! ### Basic CPU Information
//! ```rust
//! let ranges = unsafe { CpuidRanges::read() };
//! println!("CPU Vendor: {}", ranges.vendor.as_str());
//! ```
//!
//! ### Feature Detection
//! ```rust
//! let leaf1 = unsafe { Leaf01h::new() };
//! if leaf1.has_x2apic() {
//!     // x2APIC mode available
//! }
//! ```
//!
//! ### Frequency Information
//! ```rust
//! if let Some(leaf15) = unsafe { Leaf15h::read(&ranges) } {
//!     if let Some(tsc_hz) = leaf15.tsc_hz() {
//!         // TSC frequency determined
//!     }
//! }
//! ```
//!
//! ## Low-Level Interface
//!
//! The [`cpuid`] function provides direct access to the CPUID instruction:
//!
//! ```rust
//! let result = unsafe { cpuid(leaf, subleaf) };
//! // Access result.eax, result.ebx, result.ecx, result.edx
//! ```
//!
//! ## Safety
//!
//! All CPUID operations are marked `unsafe` because they:
//! - Require execution at privilege level 0 (kernel mode)
//! - Assume CPUID instruction availability on the target processor
//! - May return invalid data if called with unsupported leaf values
//!
//! Higher-level wrappers provide safer interfaces with automatic capability checking.

#![allow(dead_code)]

mod leaf01h;
mod leaf15h;
mod leaf16h;
mod ranges;

pub use leaf01h::Leaf01h;
pub use leaf15h::Leaf15h;
pub use leaf16h::Leaf16;
pub use ranges::CpuidRanges;

/// Execute CPUID with the given leaf and subleaf.
///
/// # Safety
/// Must run at CPL0 with CPUID instruction available.
///
/// # See also
/// [`CpuidRanges`] provides typed access to the `cpuid(0, 0)` result.
#[inline(always)]
#[allow(unused_assignments, clippy::inline_always)]
pub unsafe fn cpuid(leaf: u32, subleaf: u32) -> CpuidResult {
    let (mut eax, mut ebx, mut ecx, mut edx) = (leaf, 0u32, subleaf, 0u32);
    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx", // move EBX to a free GPR we bind
            "pop rbx",
            ebx_out = lateout(reg) ebx,
            inlateout("eax") eax,    // eax in/out
            inlateout("ecx") ecx,    // ecx in/out (subleaf)
            lateout("edx") edx,      // edx out
            options(nomem, preserves_flags),
        );
    }
    CpuidResult { eax, ebx, ecx, edx }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct CpuidResult {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}
