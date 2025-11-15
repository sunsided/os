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
        //   RAX = syscall number
        //   RDI = arg0
        //   RSI = arg1
        //   RDX = arg2

        // Switch GS base to kernel PerCpu
        "swapgs",

        // Save user RSP (we’ll store it in the frame)
        "mov rbx, rsp",

        // Switch to kernel syscall stack: rsp = PerCpu.kstack_top
        "mov rsp, qword ptr gs:[{kstack}]",

        // Build SyscallFrame on kernel stack.
        //
        // push order (last pushed at lowest address):
        //   rsp   (user)
        //   rflags (user)
        //   rip   (user)
        //   rdx   (arg2)
        //   rsi   (arg1)
        //   rdi   (arg0)
        //   rax   (sysno)
        //
        // resulting layout at [rsp]:
        //   0  rax
        //   8  rdi
        //   16 rsi
        //   24 rdx
        //   32 rip
        //   40 rflags
        //   48 rsp
        "push rbx",    // rsp (user)
        "push r11",    // rflags (user)
        "push rcx",    // rip (user)
        "push rdx",    // arg2
        "push rsi",    // arg1
        "push rdi",    // arg0
        "push rax",    // sysno

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
        "mov rcx, [rsp + 32]",  // user RIP
        "mov r11, [rsp + 40]",  // user RFLAGS
        "mov rbx, [rsp + 48]",  // user RSP

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

    tf.rax = syscall(sysno, a0, a1, a2, SyscallSource::Syscall);
}
