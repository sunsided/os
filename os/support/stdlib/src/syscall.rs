#[deprecated(since = "0.0.0", note = "Use the syscall variants instead")]
pub mod int80;

use crate::syscall_abi::Sysno;

#[inline(always)]
pub fn debug_byte(b: u8) {
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") Sysno::DebugWriteByte as u64 => _,
            in("rdi") b as u64,
            lateout("rcx") _, // clobbered by SYSCALL
            lateout("r11") _, // clobbered by SYSCALL
            options(nostack)
        );
    }
}

#[inline(always)]
pub fn sys_bogus() -> u64 {
    let mut ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") Sysno::Bogus as u64 => ret,
            lateout("rcx") _, // syscall clobbers
            lateout("r11") _,
            options(nostack)
        );
    }
    ret
}
