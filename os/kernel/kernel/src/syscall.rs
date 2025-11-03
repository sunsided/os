#![allow(dead_code, unused_variables)]

#[repr(u64)]
pub enum Sysno {
    /// Write a single byte to a kernel-chosen “debug” sink.
    DebugWrite = 1,
    /// Just return a made-up number to prove plumbing.
    Bogus = 2,
}

#[derive(Debug)]
#[repr(C)]
pub struct TrapFrame {
    // Pushed by CPU on interrupt gate entry (x86_64):
    // Order: RIP, CS, RFLAGS, RSP, SS
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
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
}

#[unsafe(naked)]
extern "C" fn syscall_int80_handler() -> ! {
    core::arch::naked_asm!(
        // Save GPRs we care about (SysV)
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
        // &TrapFrame into rdi (first arg)
        "mov rdi, rsp",

        "call {rust}",

        // Restore in reverse
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
        rust = sym syscall_int80_rust
    )
}

// low-level entry stub sets up a full frame then calls this:
extern "C" fn syscall_int80_rust(tf: &mut TrapFrame) {
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

// outb helper
#[inline(always)]
#[allow(clippy::inline_always)]
unsafe fn outb(port: u16, val: u8) {
    // TODO: Code duplication with kernel-qemu/src/lib.rs
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nostack, preserves_flags));
    }
}
