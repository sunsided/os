//! # QEMU Development and Debug Support
//!
//! This crate provides specialized debugging and development utilities for kernels
//! running under the QEMU emulator. It enables efficient logging, tracing, and
//! diagnostic output that significantly simplifies kernel development and debugging
//! in virtualized environments.
//!
//! ## Overview
//!
//! Kernel development presents unique debugging challenges: no standard I/O,
//! limited debugging tools, and the need for early-boot diagnostics. This crate
//! addresses these challenges by leveraging QEMU's built-in debugging features,
//! particularly the debug port mechanism for host-side output.
//!
//! ## Key Features
//!
//! * **Zero-Allocation Logging**: Efficient output without dynamic memory allocation
//! * **Standard Log Integration**: Compatible with Rust's `log` crate ecosystem
//! * **Feature-Gated Operation**: Can be completely disabled for release builds
//! * **UTF-8 Support**: Proper Unicode character encoding for debug output
//! * **Early Boot Support**: Works before full kernel initialization
//!
//! ## Architecture
//!
//! ### QEMU Debug Port Interface
//! The crate communicates with QEMU through a dedicated I/O port:
//! * **Port Address**: `0x402` (QEMU's standard debug port)
//! * **Protocol**: Simple byte-by-byte character transmission
//! * **Direction**: Kernel-to-host output only
//! * **Performance**: Extremely fast, no buffering required
//!
//! ### Output Mechanism
//! ```text
//! Kernel Code
//!     ↓
//! qemu_trace! macro
//!     ↓
//! QemuSink (fmt::Write)
//!     ↓
//! dbg_putc() → I/O port 0x402
//!     ↓
//! QEMU Debug Output
//!     ↓
//! Host Terminal/Console
//! ```
//!
//! ## Core Components
//!
//! ### QEMU Logger ([`QemuLogger`])
//! A `log::Log` implementation that routes log messages to QEMU's debug port:
//! * **Level Filtering**: Configurable log level thresholds
//! * **Target Support**: Module and component-specific logging
//! * **Static Initialization**: No-allocation setup for kernel environments
//! * **Format Control**: Structured log message formatting
//!
//! ### Trace Macro ([`qemu_trace!`])
//! Direct debug output bypassing the logging framework:
//! * **Zero-Overhead**: Compile-time format string processing
//! * **UTF-8 Encoding**: Proper multi-byte character support
//! * **Feature Gating**: Completely removed when `enabled` feature is disabled
//! * **Format Compatibility**: Supports `format!`-style argument patterns
//!
//! ### Output Sink ([`qemu_fmt::QemuSink`])
//! A `core::fmt::Write` implementation for structured output:
//! * **Character-by-Character**: Immediate output with no buffering
//! * **UTF-8 Encoding**: Proper Unicode character decomposition
//! * **Error Resilience**: Best-effort output with graceful degradation
//!
//! ## Feature System
//!
//! The crate uses Cargo features to control compilation and runtime behavior:
//!
//! ### `enabled` Feature (default)
//! When enabled:
//! * Full QEMU debug port functionality
//! * Active trace macro implementation
//! * I/O port operations compiled in
//! * UTF-8 encoding and formatting support
//!
//! When disabled:
//! * All debug operations become no-ops
//! * Zero runtime overhead
//! * No I/O port access
//! * Suitable for production builds
//!
//! ## Usage Patterns
//!
//! ### Basic Logging Setup
//! ```rust,no_run
//! use kernel_qemu::QemuLogger;
//! use log::{LevelFilter, info};
//!
//! // Early in kernel initialization
//! let logger = QemuLogger::new(LevelFilter::Debug);
//! logger.init().expect("logger initialization");
//!
//! // Standard logging throughout kernel
//! info!("Kernel subsystem initialized");
//! ```
//!
//! ### Direct Trace Output
//! ```rust,ignore
//! use kernel_qemu::qemu_trace;
//!
//! // Immediate debug output
//! qemu_trace!("Debug value: {:#x}\n", register_value);
//!
//! // Complex formatting
//! qemu_trace!("CPU {}: Exception {} at RIP={:#x}\n",
//!             cpu_id, exception_number, instruction_pointer);
//! ```
//!
//! ### Conditional Compilation
//! ```rust
//! #[cfg(feature = "qemu-debug")]
//! use kernel_qemu::qemu_trace;
//!
//! fn debug_memory_layout() {
//!     #[cfg(feature = "qemu-debug")]
//!     qemu_trace!("Memory layout: base={:#x} size={:#x}\n", base, size);
//! }
//! ```
//!
//! ## QEMU Integration
//!
//! ### Host-Side Configuration
//! To capture kernel debug output on the host:
//! ```bash
//! # Standard QEMU invocation with debug output
//! qemu-system-x86_64 -kernel kernel.bin -debugcon stdio
//!
//! # Redirect to file
//! qemu-system-x86_64 -kernel kernel.bin -debugcon file:debug.log
//!
//! # Network debugging (advanced)
//! qemu-system-x86_64 -kernel kernel.bin -debugcon tcp:127.0.0.1:1234,server
//! ```
//!
//! ### Debug Port Mechanism
//! QEMU's debug console feature (`-debugcon`) captures writes to port `0x402`:
//! * **Real-time Output**: Immediate visibility of kernel messages
//! * **No Guest Impact**: Zero performance impact on kernel execution
//! * **Flexible Routing**: Output can go to stdio, files, or network
//! * **Cross-Platform**: Works on all QEMU-supported host platforms
//!
//! ## Performance Characteristics
//!
//! * **Macro Overhead**: Zero when `enabled` feature is disabled
//! * **I/O Latency**: Sub-microsecond output latency in QEMU
//! * **Memory Usage**: No dynamic allocation, minimal static footprint
//! * **CPU Impact**: Negligible processing overhead
//!
//! ## Safety Considerations
//!
//! ### I/O Port Access
//! * **Privilege Requirements**: Requires kernel-mode execution (Ring 0)
//! * **Port Safety**: QEMU debug port is read-only from host perspective
//! * **Hardware Compatibility**: Safe no-op on real hardware (port typically unused)
//! * **Error Handling**: Graceful degradation if QEMU features unavailable
//!
//! ### UTF-8 Encoding
//! * **Memory Safety**: No buffer overflows in character encoding
//! * **Encoding Validity**: Proper UTF-8 sequence generation
//! * **Error Recovery**: Invalid characters handled gracefully
//!
//! ## Development Workflow
//!
//! This crate is essential for iterative kernel development:
//!
//! 1. **Early Debugging**: Debug boot process and initialization
//! 2. **Runtime Diagnostics**: Monitor kernel operation and state changes
//! 3. **Performance Analysis**: Trace timing-critical operations
//! 4. **Error Investigation**: Capture crash information and stack traces
//! 5. **Feature Development**: Validate new subsystem implementation
//!
//! The integration with QEMU makes it an indispensable tool for productive
//! kernel development, providing the debugging visibility essential for
//! complex systems programming.

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code)]

mod logger;

pub use logger::QemuLogger;

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
    pub fn qemu_write(args: fmt::Arguments) {
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
        $crate::qemu_fmt::qemu_write(core::format_args!($($arg)*));
    }};
}
