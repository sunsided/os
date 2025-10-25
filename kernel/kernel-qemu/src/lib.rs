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
