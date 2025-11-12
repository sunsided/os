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
