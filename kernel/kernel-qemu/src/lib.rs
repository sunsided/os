//! # Kernel QEMU Helpers

#![no_std]
#![allow(unsafe_code)]

/// The port number for QEMU's debug port.
const QEMU_DEBUG_PORT: u16 = 0x402;

/// Write a string to QEMU's debug port.
pub fn dbg_print<B>(input: B)
where
    B: AsRef<[u8]>,
{
    for &b in input.as_ref() {
        dbg_putc(b);
    }
}

/// Write a usize as decimal to QEMU's debug port.
///
/// - No allocations (uses a small stack buffer)
/// - Handles `0`
/// - Works for 64-bit and 128-bit `usize` (buffer sized for worst case)
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn dbg_print_usize<N>(n: N)
where
    N: Into<usize>,
{
    let mut n = n.into();

    // Fast path for zero.
    if n == 0 {
        dbg_putc(b'0');
        return;
    }

    // 39 digits covers up to 2^128-1; on x86_64 we only need 20, but this is safe everywhere.
    let mut buf = [0u8; 39];
    let mut i = 0usize;

    // Write least-significant digits first.
    while n > 0 {
        let digit = u8::try_from(n % 10).unwrap_or_default();
        buf[i] = b'0' + digit;
        i += 1;
        n /= 10;
    }

    // Emit in reverse (most-significant first).
    while i > 0 {
        i -= 1;
        dbg_putc(buf[i]);
    }
}

/// Write a single character to QEMU's debug port.
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn dbg_putc(c: u8) {
    unsafe { outb(QEMU_DEBUG_PORT, c) }
}

/// Write to QEMU's port.
#[allow(clippy::inline_always)]
#[inline(always)]
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") val,
        options(nomem, preserves_flags));
    }
}
