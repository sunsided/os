//! # Kernel QEMU Helpers

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code)]
#![cfg_attr(not(feature = "enabled"), allow(unused_variables))]

#[cfg(feature = "enabled")]
#[doc(hidden)]
pub mod qemu_fmt {
    use core::fmt::{self, Write};

    /// The port number for QEMU's debug port.
    const QEMU_DEBUG_PORT: u16 = 0x402;

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
            options(nomem, preserves_flags)
            );
        }
    }

    // TODO: Model this as an actual sink for arbitrary port write
    pub struct QemuSink;

    impl Write for QemuSink {
        #[inline]
        fn write_str(&mut self, s: &str) -> fmt::Result {
            for b in s.bytes() {
                dbg_putc(b);
            }
            Ok(())
        }

        #[inline]
        fn write_char(&mut self, c: char) -> fmt::Result {
            // UTF-8 encode without allocation.
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            self.write_str(s)
        }
    }

    #[doc(hidden)]
    #[inline(always)]
    #[allow(clippy::inline_always)]
    pub fn _qemu_write(args: fmt::Arguments) {
        // Ignore errors; this is best-effort debug output.
        let _ = fmt::write(&mut QemuSink, args);
    }
}

#[cfg(not(feature = "enabled"))]
#[doc(hidden)]
pub mod qemu_fmt {
    use core::fmt;
    #[doc(hidden)]
    #[inline(always)]
    pub fn _qemu_write(_: fmt::Arguments) {
        // no-op when feature disabled
    }
}

// TODO: Model this as a regular trace macro optionally backed by the QWEMU sink
#[macro_export]
macro_rules! qemu_trace {
    ($($arg:tt)*) => {{
        // No allocation: `format_args!` builds a lightweight `Arguments`.
        $crate::qemu_fmt::_qemu_write(core::format_args!($($arg)*));
    }};
}
