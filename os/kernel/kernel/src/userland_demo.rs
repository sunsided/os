core::arch::global_asm!(
    r#"
    .section .user.text,"ax",@progbits
    .balign 16
    .globl _user_demo_start
    _user_demo_start:
        mov rax, 2          // Sysno::Bogus
        int 0x80            // rax = 0xd34dc0d3 (from kernel)
        mov rdx, rax        // value to print
        mov rcx, 16         // 16 hex digits

    print_loop:
        rol rdx, 4          // bring next top nibble into low 4 bits
        mov rbx, rdx
        and bl, 0x0F        // bl = nibble 0..15

        // Compute '0' + nibble  (candidate for 0..9)
        movzx r8, bl        // r8 = nibble (64-bit)
        add   r8, '0'       // r8 = '0'..'9' or beyond

        // Compute 'A' + (nibble-10)  (candidate for A..F)
        movzx r9, bl        // r9 = nibble
        add   r9, 'A' - 10  // r9 = 'A'..'F' (for 10..15)

        // Select based on nibble >= 10
        cmp   bl, 10
        cmova r8, r9        // r8 = hex char

        // Emit via DebugWrite (syscall 1)
        mov   rax, 1
        mov   dil, r8b
        int   0x80

        loop print_loop

        // newline
        mov rax, 1
        mov dil, 0x0A
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
