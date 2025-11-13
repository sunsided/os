//! # x86 I/O Port Access
//!
//! This module provides low-level access to the x86/x86-64 I/O port address space,
//! offering safe wrappers around the `in` and `out` assembly instructions for
//! legacy device communication. It enables kernel drivers to interact with
//! hardware that uses port-mapped I/O rather than memory-mapped I/O (MMIO).
//!
//! ## Overview
//!
//! The x86 architecture provides two distinct address spaces for device communication:
//! - **I/O Port Space**: Legacy 16-bit address space (0x0000-0xFFFF) accessed via
//!   special `in`/`out` instructions
//! - **Memory Space**: Normal memory addresses accessed via standard load/store
//!   instructions (MMIO)
//!
//! This module handles the I/O port space, which is primarily used by legacy
//! devices and some system controllers that maintain backward compatibility.
//!
//! ## Architecture Details
//!
//! ### I/O Port Space Characteristics
//! * **16-bit Addressing**: Port numbers range from 0x0000 to 0xFFFF
//! * **Separate Address Space**: Completely distinct from physical memory
//! * **Special Instructions**: Requires `in`/`out` instructions, not memory loads/stores
//! * **Privilege Controlled**: Access controlled by CPL, IOPL, and I/O permission bitmap
//!
//! ### Common Port Ranges
//! ```text
//! 0x0000-0x001F   DMA Controllers
//! 0x0020-0x0021   Programmable Interrupt Controller (PIC) #1
//! 0x0040-0x0043   Programmable Interval Timer (PIT)
//! 0x0060-0x0064   Keyboard Controller
//! 0x0070-0x0071   CMOS/RTC
//! 0x00A0-0x00A1   PIC #2
//! 0x00F0-0x00FF   Math Coprocessor
//! 0x0170-0x0177   Secondary IDE Controller
//! 0x01F0-0x01F7   Primary IDE Controller
//! 0x0278-0x027A   Parallel Port #2
//! 0x02E8-0x02EF   Serial Port #4
//! 0x02F8-0x02FF   Serial Port #2
//! 0x0378-0x037A   Parallel Port #1
//! 0x03E8-0x03EF   Serial Port #3
//! 0x03F0-0x03F7   Floppy Disk Controller
//! 0x03F8-0x03FF   Serial Port #1
//! ```
//!
//! ## Available Operations
//!
//! ### Byte Operations
//! * [`outb`] - Write a single byte to an I/O port
//! * [`inb`] - Read a single byte from an I/O port
//!
//! ### Usage Example
//! ```rust
//! use crate::ports::{inb, outb};
//!
//! // Read keyboard status (port 0x64)
//! let status = unsafe { inb(0x64) };
//!
//! // Write to serial port (port 0x3F8)
//! unsafe { outb(0x3F8, b'H') };
//! ```
//!
//! ## Safety Requirements
//!
//! All I/O port operations are inherently unsafe due to their direct hardware
//! interaction and potential system impact. Callers must ensure:
//!
//! ### Privilege Requirements
//! * **Ring 0 Execution**: Most secure when running in kernel mode (CPL 0)
//! * **I/O Permission**: If not in ring 0, IOPL bits or I/O permission bitmap
//!   must allow access to the specific port
//!
//! ### Hardware Safety
//! * **Correct Port**: Target the intended device register, not arbitrary addresses
//! * **Device Presence**: Ensure the device exists and is properly initialized
//! * **Protocol Compliance**: Follow device-specific communication protocols
//! * **Timing Requirements**: Respect device timing constraints and handshakes
//!
//! ### Concurrency Safety
//! * **Mutual Exclusion**: Coordinate with interrupt handlers and other threads
//! * **Atomic Sequences**: Protect multi-step device interactions from interruption
//! * **Driver Coordination**: Ensure only one driver controls each device
//!
//! ### Memory Ordering
//! * **I/O Ordering**: `in`/`out` instructions are ordered relative to each other
//! * **Memory Barriers**: Insert appropriate fences when coordinating with MMIO
//! * **Compiler Barriers**: Prevent unwanted optimization of I/O sequences
//!
//! ## Design Philosophy
//!
//! This module follows a minimal, explicit approach:
//! * **Thin Wrappers**: Direct exposure of hardware capabilities without abstraction
//! * **Safety Documentation**: Comprehensive safety requirements rather than runtime checks
//! * **Performance Focus**: Zero-overhead inline assembly with optimal instruction selection
//! * **Explicit Unsafe**: All hardware interaction requires explicit acknowledgment of risks
//!
//! ## Future Extensions
//!
//! Additional I/O operations may be added as needed:
//! * 16-bit operations (`inw`/`outw`) for word-sized transfers
//! * 32-bit operations (`inl`/`outl`) for double-word transfers
//! * String operations (`insb`/`outsb`) for bulk transfers
//! * I/O delay helpers for timing-sensitive devices

/// Write one byte to an I/O port (x86).
///
/// Low-level helper for devices that live in the legacy **I/O port space**
/// (not MMIO). Uses `out dx, al`.
///
/// # Safety
/// You must uphold **all** of the following:
/// - **Privilege:** Execute at CPL0 **or** have I/O permission (IOPL/IO bitmap)
///   that allows access to `port`. Otherwise the CPU raises `#GP`.
/// - **Correct port:** `port` must belong to the intended device and be in a
///   valid state for this write. Writing the wrong port or wrong value can wedge
///   the device or the system (e.g., disabling the PIC, reprogramming timers).
/// - **Device presence:** The target device must exist and be decoded on the
///   bus. Some platforms hang on accesses to nonexistent ports.
/// - **Concurrency:** Coordinate with interrupt handlers and other CPUs/threads
///   that touch the same device/port. Use your driver’s locking/serialization
///   so register-level protocols aren’t violated.
/// - **Ordering:** `out` orders with respect to other I/O instructions to the
///   same device but is **not** a general memory fence. If you need ordering
///   with normal memory (e.g., MMIO buffers or shared memory), add an
///   appropriate compiler/CPU fence around calls.
/// - **Environment:** Only use on `x86/x86_64` with an I/O port bus. Never use
///   for **memory-mapped** devices (MMIO).
#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
    }
}

/// Read one byte from an I/O port (x86).
///
/// Low-level helper for devices that live in the legacy **I/O port space**.
/// Uses `in al, dx`.
///
/// # Safety
/// You must uphold **all** of the following:
/// - **Privilege:** Execute at CPL0 **or** have I/O permission (IOPL/IO bitmap)
///   that allows access to `port`; otherwise the CPU raises `#GP`.
/// - **Correct port:** `port` must be a readable register of the intended
///   device; reading from the wrong port can yield undefined garbage or stall
///   the device’s protocol.
/// - **Device presence:** The target device must exist and be decoding the
///   address. Accesses to nonexistent ports may fault or hang on some systems.
/// - **Concurrency:** Coordinate with interrupt handlers/other CPUs that
///   manipulate the same device/port to avoid tearing multi-step handshakes.
/// - **Ordering:** `in` orders with other I/O instructions but is **not** a
///   general memory fence. If you must order this read with normal memory
///   operations (e.g., reading a status port then consuming an MMIO buffer),
///   insert the appropriate compiler/CPU fence.
/// - **Environment:** Only for `x86/x86_64` I/O port space. Do not use for MMIO.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let mut v: u8;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") v, options(nomem, nostack, preserves_flags));
    }
    v
}
