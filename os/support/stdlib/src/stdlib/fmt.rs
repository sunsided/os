use crate::syscall::debug_byte;
use core::fmt::{self, Write};

pub struct SyscallSink;

impl Write for SyscallSink {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            debug_byte(b);
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
pub fn syscall_write(args: fmt::Arguments) {
    // Ignore errors; this is best-effort debug output.
    fmt::write(&mut SyscallSink, args).ok();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::stdlib::fmt::syscall_write(core::format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        $crate::stdlib::fmt::syscall_write(core::format_args!($($arg)*));
        $crate::syscall::debug_byte(b'\n');
    }};
}
