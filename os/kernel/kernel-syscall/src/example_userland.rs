/// tiny user code (SysV x86_64); assembled by hand to keep it self-contained.
///
/// Layout:
///
/// ```asm
///   start:
///     mov edi, 7          ; arg0
///     mov esi, 35         ; arg1
///     call add_fn
///     ; rax holds 42
///     mov rax, 2          ; SYSCALL: Bogus
///     int 0x80
///     mov dil, 0x41       ; 'A'
///     mov rax, 1          ; SYSCALL: DebugWrite
///     int 0x80
///   spin: jmp spin
///   add_fn:
///     mov eax, edi
///     add eax, esi
///     ret
/// ```
#[rustfmt::skip]
pub static USER_CODE: &[u8] = &[
    0xbf, 0x07, 0x00, 0x00, 0x00,       // mov edi, 7
    0xbe, 0x23, 0x00, 0x00, 0x00,       // mov esi, 35
    0xe8, 0x0e, 0x00, 0x00, 0x00,       // call +0x0e -> add_fn
    0x48, 0xc7, 0xc0, 0x02, 0x00, 0x00, 0x00, // mov rax, 2
    0xcd, 0x80,                         // int 0x80
    0x40, 0xb7, 0x41,                   // mov dil, 'A'
    0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1
    0xcd, 0x80,                         // int 0x80
    0xeb, 0xfe,                         // spin: jmp $
    // add_fn:
    0x89, 0xf8,                         // mov eax, edi
    0x01, 0xf0,                         // add eax, esi
    0xc3,                               // ret
];
