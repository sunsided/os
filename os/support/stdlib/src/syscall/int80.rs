use crate::syscall_abi::Sysno;

#[inline(always)]
#[deprecated(note = "Use debug_byte instead")]
pub fn debug_byte_int80(b: u8) {
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") Sysno::DebugWriteByte as u64,
            in("rdi") b as u64,
            // clobbers: none declared; int80 preserves ABI like a call
            options(nostack)
        );
    }
}

#[inline(always)]
#[deprecated(note = "Use sys_bogus instead")]
pub fn sys_bogus_int80() -> u64 {
    let mut ret: u64;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") Sysno::Bogus as u64 => ret,
            options(nostack)
        );
    }
    ret
}
