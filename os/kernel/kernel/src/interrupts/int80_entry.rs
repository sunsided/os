#![allow(clippy::missing_safety_doc)]

use core::arch::naked_asm;

// This is the function symbol we pass to `init_syscall_gate`.
#[unsafe(naked)]
pub extern "C" fn int80_entry() {
    naked_asm!(
        // Save caller-saved + what you need (example set)
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rbp",
        "push rdi",
        "push rsi",
        "push rdx",
        "push rcx",
        "push rbx",
        "push rax",
        "mov rdi, rsp",           // &trapframe scratch if you want
        "call {rust}",
        "pop rax",
        "pop rbx",
        "pop rcx",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop rbp",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",
        "iretq",
        rust = sym int80_dispatch
    );
}

// A tiny dispatcher; read args from regs if you’ve captured them in a TrapFrame.
const extern "C" fn int80_dispatch(_sp: *const u8) {
    // Example: do nothing for now (or poke QEMU port here)
    // unsafe { outb(0x402, b'!'); }
}
