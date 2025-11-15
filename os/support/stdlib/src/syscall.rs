#![allow(clippy::inline_always)]

#[deprecated(since = "0.0.0", note = "Use the syscall variants instead")]
pub mod int80;

use crate::syscall_abi::Sysno;

#[inline(always)]
pub fn debug_byte(b: u8) {
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") Sysno::DebugWriteByte as u64 => _,
            in("rdi") u64::from(b),
            out("rcx") _, // syscall clobbers
            out("r11") _, // syscall clobbers
            out("r12") _, // syscall stub clobbers
            options(nostack)
        );
    }
}

#[inline(always)]
#[must_use]
pub fn sys_bogus() -> u64 {
    let mut ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") Sysno::Bogus as u64 => ret,
            out("rcx") _, // syscall clobbers
            out("r11") _, // syscall clobbers
            out("r12") _, // syscall stub clobbers
            options(nostack)
        );
    }
    ret
}
