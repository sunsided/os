core::arch::global_asm!(
    r#"
    .section .user.text,"ax",@progbits
    .balign 16
    .globl _user_demo_start
    _user_demo_start:

        // --- First run: INT 0x80 path ---

        // Call Bogus via int 0x80: rax = 0xd34dc0d3 (from kernel)
        mov     rax, 2              // Sysno::Bogus
        int     0x80

        // Value to print in hex
        mov     rdx, rax            // rdx = value
        mov     r10d, 16            // r10 = number of hex digits to print

    .Lint_print_loop:
        // Rotate left by 4 bits: bring next top nibble into low 4 bits
        rol     rdx, 4
        mov     rbx, rdx
        and     bl, 0x0F            // bl = nibble (0..15)

        // Compute '0' + nibble  (candidate for 0..9)
        movzx   r8, bl              // r8 = nibble (64-bit)
        add     r8, '0'             // r8 = '0'..'9' or beyond

        // Compute 'A' + (nibble-10)  (candidate for A..F)
        movzx   r9, bl              // r9 = nibble
        add     r9, 'A' - 10        // r9 = 'A'..'F' (for 10..15)

        // Select based on nibble >= 10
        cmp     bl, 10
        cmova   r8, r9              // r8 = hex char

        // Emit via DebugWrite (syscall 1) using INT 0x80
        mov     rax, 1              // Sysno::DebugWriteByte
        mov     dil, r8b
        int     0x80

        // Decrement digit counter and loop
        dec     r10d
        jnz     .Lint_print_loop

        // Newline via INT 0x80
        mov     rax, 1
        mov     dil, 0x0A
        int     0x80


        // --- Second run: SYSCALL path ---

        // Call Bogus via SYSCALL
        mov     rax, 2              // Sysno::Bogus
        syscall

        // Value to print in hex
        mov     rdx, rax            // rdx = value
        mov     r10d, 16            // r10 = number of hex digits to print

    .Lsyscall_print_loop:
        // Same nibble-to-hex logic as above
        rol     rdx, 4
        mov     rbx, rdx
        and     bl, 0x0F

        movzx   r8, bl
        add     r8, '0'

        movzx   r9, bl
        add     r9, 'A' - 10

        cmp     bl, 10
        cmova   r8, r9              // r8 = hex char

        // Emit via DebugWrite (syscall 1) using SYSCALL
        mov     rax, 1              // Sysno::DebugWriteByte
        mov     dil, r8b
        syscall

        dec     r10d
        jnz     .Lsyscall_print_loop

        // Newline via SYSCALL
        mov     rax, 1
        mov     dil, 0x0A
        syscall

    1:
        jmp 1b

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
        let start = &raw const _user_demo_start as usize;
        let end = &raw const _user_demo_end as usize;
        core::slice::from_raw_parts(start as *const u8, end - start)
    }
}
