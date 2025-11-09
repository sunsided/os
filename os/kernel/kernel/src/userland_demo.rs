// Use int3 to jump into the breakpoint handler
core::arch::global_asm!(
    r#"
    .intel_syntax noprefix
    .section .user.text,"ax",@progbits
    .balign 16
    .globl _user_demo_start
_user_demo_start:
    mov rax, 1          // Sysno::DebugWrite
    mov dil, 0x42       // 'B'
    int 0x80
1:  jmp 1b
    .globl _user_demo_end
_user_demo_end:
"#
);

// Expose start/end as symbols we can take addresses of
unsafe extern "C" {
    static _user_demo_start: u8;
    static _user_demo_end: u8;
}

#[inline]
pub fn user_demo_bytes() -> &'static [u8] {
    unsafe {
        let start = &_user_demo_start as *const u8 as usize;
        let end = &_user_demo_end as *const u8 as usize;
        core::slice::from_raw_parts(start as *const u8, end - start)
    }
}
