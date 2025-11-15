use crate::per_cpu::PerCpu;
use crate::syscall::{SyscallSource, syscall};
use core::mem::offset_of;
use kernel_registers::rflags::Rflags;

/// Minimal state for SYSCALL/SYSRET-based syscalls.
///
/// Layout must match the push order in the naked stub:
///
///   push rsp   → rsp
///   push r11   → rflags
///   push rcx   → rip
///   push rdi   → arg0
///   push rax   → sysno/ret
///
/// With `#[repr(C)]`, memory from top-of-stack (RSP) looks like:
///
///   +0  rax
///   +8  rdi
///   +16 rip
///   +24 rflags
///   +32 rsp
#[derive(Debug)]
#[repr(C)]
pub struct SyscallFrame {
    pub rax: u64, // syscall number on entry, return value on exit
    pub rdi: u64, // arg0
    pub rsi: u64, // arg1
    pub rdx: u64, // arg2
    pub r10: u64, // arg3 (normally rcx, but that's reserved by syscall)
    pub r8: u64,  // arg4, sic!
    pub r9: u64,  // arg5
    pub rip: u64, // user return RIP (from RCX)
    pub rflags: Rflags,
    pub rsp: u64, // user stack pointer on entry
}

#[unsafe(naked)]
pub extern "C" fn syscall_entry_stub() {
    // Get the number of bytes between &PerCpu and &PerCpu.kstack_top
    const PERCPU_KSTACK_TOP_OFFSET: usize = offset_of!(PerCpu, kstack_top);

    core::arch::naked_asm!(
        // SYSCALL entry invariants:
        //   RCX = user RIP
        //   R11 = user RFLAGS
        //   RSP = user stack
        //
        //   RAX = syscall number
        //   RDI = arg0
        //   RSI = arg1
        //   R10 = arg2
        //   R8 = arg2
        //   R9 = arg3

        // Switch GS base to kernel PerCpu
        "swapgs",

        // Save user RSP (we’ll store it in the frame)
        "mov rbx, rsp",

        // Switch to kernel syscall stack: rsp = PerCpu.kstack_top
        "mov rsp, qword ptr gs:[{kstack}]",

        // ensure pre-call %rsp % 16 == 8 (SysV). kstack_top is 16-aligned.
        "sub rsp, 8",

        // Build SyscallFrame on kernel stack.
        //
        // push order (last pushed at lowest address):
        //   rsp   (user)
        //   rflags (user)
        //   rip   (user)
        //   r9    (arg5)
        //   r8    (arg4, sic!)
        //   r10   (arg3)
        //   rdx   (arg2)
        //   rsi   (arg1)
        //   rdi   (arg0)
        //   rax   (sysno)
        //
        // resulting layout at [rsp]:
        //   +0  rax   (ret / sysno)
        //   +8  rdi   (a0)
        //   +16 rsi   (a1)
        //   +24 rdx   (a2)
        //   +32 r10   (a3)
        //   +40 r8    (a4)
        //   +48 r9    (a5)
        //   +56 rip
        //   +64 rflags
        //   +72 rsp
        "push rbx",   // +72: user RSP
        "push r11",   // +64: user RFLAGS
        "push rcx",   // +56: user RIP
        "push r9",    // +48: a5
        "push r8",    // +40: a4
        "push r10",   // +32: a3
        "push rdx",   // +24: a2
        "push rsi",   // +16: a1
        "push rdi",   // +8 : a0
        "push rax",   // +0 : sysno / retval

        // &SyscallFrame in RDI for Rust (SysV ABI)
        "mov rdi, rsp",

        // Call Rust dispatcher
        "call {rust}",

        // On return, Rust may have updated tf.rax. Everything else we
        // restore exactly as the user had it.

        // Load fields back into registers:
        "mov rax, [rsp + 0]",   // return value
        "mov rdi, [rsp + 8]",   // arg0 (restore)
        "mov rsi, [rsp + 16]",  // arg1 (restore)
        "mov rdx, [rsp + 24]",  // arg2 (restore)
        "mov r10, [rsp + 32]",  // arg3 (restore)
        "mov r8,  [rsp + 40]",  // arg4 (restore)
        "mov r9,  [rsp + 48]",  // arg5 (restore)
        "mov rcx, [rsp + 56]",  // user RIP
        "mov r11, [rsp + 64]",  // user RFLAGS
        "mov rbx, [rsp + 72]",  // user RSP

        // Switch to user stack
        "mov rsp, rbx",

        // Back to user GS
        "swapgs",

        // Return to user
        "sysretq",

        kstack = const PERCPU_KSTACK_TOP_OFFSET,
        rust = sym syscall_fast_rust,
    );
}

#[allow(clippy::no_effect_underscore_binding)]
extern "C" fn syscall_fast_rust(tf: &mut SyscallFrame) {
    let sysno = tf.rax;
    let a0 = tf.rdi;
    let a1 = tf.rsi;
    let a2 = tf.rdx;
    let a3 = tf.r10;
    let a4 = tf.r8; // sic!
    let a5 = tf.r9;

    tf.rax = syscall(sysno, a0, a1, a2, a3, a4, a5, SyscallSource::Syscall);
}
