//! x86_64 INT 0x80 syscall entry and minimal dispatcher.
//!
//! ## Overview
//! We expose a classic Linux-style **software interrupt** (vector `0x80`) as the
//! user→kernel syscall path. A `#[naked]` asm stub:
//! 1) Saves all GP registers into a contiguous `TrapFrame` on the current stack,
//! 2) passes `&mut TrapFrame` to a small Rust dispatcher,
//! 3) restores registers, then issues `iretq` to resume the interrupted context.
//!
//! ## ABI used **by this kernel**
//! - `RAX` = syscall number (`Sysno`).
//! - `RDI`, `RSI`, `RDX` = first three integer args (`a0..a2`).
//! - Return value is written back to `RAX` by the handler.
//!
//! ## Integration requirements
//! - IDT entry for vector **0x80** must be a **64-bit interrupt gate** (type=0xE),
//!   `P=1`, **DPL=3** (user-callable), and point to `syscall_int80_handler`.
//! - A valid **TSS** must be loaded (`ltr`) with `rsp0` set. If you route 0x80 via
//!   an **IST**, set the IST index in the gate accordingly.
//! - If you rely on `GS`-based per-CPU data in kernel mode, you likely need a
//!   `swapgs` on entry/exit (not shown here).
//!
//! ## Error model
//! Unknown/unsupported syscalls return `u64::MAX` (a stand-in for `-ENOSYS`).
//!
//! ## Safety / portability notes
//! - This is x86_64-only and uses a `#[naked]` function with inline asm.
//! - We rely on the precise **push order** below; keep `TrapFrame` in sync.
//! - Stack alignment: on entry the CPU pushes 5×8 bytes; we then push 15×8,
//!   so total push is **160 bytes**, preserving 16-byte alignment before the call
//!   into Rust (SysV ABI-friendly).
//! - The IF flag is **cleared** on interrupt-gate entry. We do not re-enable
//!   interrupts inside the stub.
//!
//! ## Frame shape (top of stack → bottom):
//!   rax, rbx, rcx, rdx, rsi, rdi, rbp, r8, r9, r10, r11, r12, r13, r14, r15,
//!   then CPU-pushed interrupt frame: rip, cs, rflags, rsp, ss.
//!
//! `rsp` at the call site points at `rax` ⇒ `&TrapFrame` starts at `rax`.

#![allow(dead_code, unused_variables)]

use crate::ports::outb;
use kernel_qemu::qemu_trace;

#[repr(u64)]
pub enum Sysno {
    /// Write a single byte to a kernel-chosen “debug” sink.
    DebugWrite = 1,
    /// Just return a made-up number to prove plumbing.
    Bogus = 2,
}

/// Saved register/interrupt context for an INT 0x80 syscall.
///
/// **Layout must match the push order in the naked stub.**
#[derive(Debug)]
#[repr(C)]
pub struct TrapFrame {
    // Pushed by your stub before calling Rust handler:
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,

    // TODO: Validate the CPU frame is indeed in the correct location here.

    // Pushed by CPU on interrupt gate entry (x86_64):
    // Order: RIP, CS, RFLAGS, RSP, SS
    /// Return RIP saved by the CPU.
    rip: u64,
    /// Code segment selector at the time of INT 0x80 (usually user CS).
    cs: u64,
    /// RFLAGS at the time of INT 0x80 (IF is cleared on entry).
    rflags: u64,
    /// Return RSP saved by the CPU (user stack pointer on entry).
    rsp: u64,
    /// Stack segment selector (user SS).
    ss: u64,
}

#[unsafe(naked)]
pub extern "C" fn syscall_int80_handler() {
    // Naked: no compiler prologue/epilogue; we must be perfectly balanced.
    core::arch::naked_asm!(
        // --- Callee-save and caller-save alike: we capture the full GPR set. ---
        // Push order defines our TrapFrame order (top → bottom).
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

        // At this point, [rsp] = rax field ⇒ &TrapFrame == rsp.
        // SysV: first arg in RDI.
        "mov rdi, rsp",

        // Call into Rust. We kept stack 16-byte aligned (160 bytes pushed).
        // The Rust side writes the return value back to tf.rax.
        "call {rust}",

        // Restore in strict reverse order.
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

        // CPU interrupt frame (RIP, CS, RFLAGS, RSP, SS) remains below.
        // `iretq` consumes it and returns to the interrupted context.
        "iretq",

        rust = sym syscall_int80_rust
    )
}

/// Rust dispatcher invoked by the low-level entry stub.
///
/// Contract:
/// - Reads `sysno` from `tf.rax`, args from `tf.{rdi,rsi,rdx}` (kernel ABI).
/// - Writes the return value to `tf.rax`.
/// - Must not assume interrupts are enabled; they are not.
extern "C" fn syscall_int80_rust(tf: &mut TrapFrame) {
    qemu_trace!("In syscall handler\n");

    let sysno = tf.rax;
    let a0 = tf.rdi;
    let a1 = tf.rsi;
    let a2 = tf.rdx;

    tf.rax = match sysno {
        x if x == Sysno::DebugWrite as u64 => {
            unsafe {
                // QEMU debug console: pick the one you wired up; you said 0x402.
                // (0xE9 is another common one.)
                let byte = (a0 & 0xFF) as u8;
                outb(0x402, byte);
            }
            0
        }
        x if x == Sysno::Bogus as u64 => 0xd34d_c0d3, // prove return works
        _ => u64::MAX,                                // -ENOSYS
    };
}
