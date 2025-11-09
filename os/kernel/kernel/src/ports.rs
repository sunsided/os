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
/// - **Environment:** Only use on x86/x86_64 with an I/O port bus. Never use
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
/// - **Environment:** Only for x86/x86_64 I/O port space. Do not use for MMIO.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let mut v: u8;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") v, options(nomem, nostack, preserves_flags));
    }
    v
}
