/// CPU privilege rings, represented as numerical privilege levels (0–3).
///
/// These are the **Descriptor Privilege Levels (DPLs)** and **Current Privilege Level (CPL)**
/// values used by the x86 and x86-64 architectures to enforce access control.
///
/// ## Overview
/// x86 defines four privilege rings (also called protection levels):
///
/// | Ring | Numeric Level | Typical Use | Privilege |
/// |------|----------------|--------------|------------|
/// | **Ring 0** | 0 | Kernel / supervisor mode | Highest privilege (can execute all instructions) |
/// | **Ring 1** | 1 | Drivers / OS subsystems (rarely used) | Intermediate privilege |
/// | **Ring 2** | 2 | Drivers / OS subsystems (rarely used) | Intermediate privilege |
/// | **Ring 3** | 3 | User-mode applications | Lowest privilege (restricted instructions) |
///
/// In most modern 64-bit kernels, only **Ring 0** (kernel) and **Ring 3** (user) are used.
/// Rings 1 and 2 exist in the architecture but are conventionally unused.
///
/// ## Relationship to DPL, CPL, and RPL
/// - **CPL (Current Privilege Level):** the ring number of the currently executing code segment.
/// - **DPL (Descriptor Privilege Level):** the ring required to *access* a descriptor (segment,
///   gate, etc.). A gate with `DPL=3` can be called from user space (`CPL=3`), whereas `DPL=0`
///   requires kernel mode.
/// - **RPL (Requested Privilege Level):** stored in the low two bits of a segment selector,
///   allowing software to request access as if from a higher ring.
///
/// When the CPU checks access permissions, it effectively compares
/// `max(CPL, RPL) <= DPL`. If the condition fails, a `#GP` (General Protection Fault) is raised.
///
/// ## Usage
/// This enum is mainly used when configuring descriptor privilege levels in
/// structures such as the [Interrupt Descriptor Table (IDT)](crate::interrupts::Idt):
///
/// ```ignore
/// idt[0x80]
///     .set_handler(syscall_int80_handler)
///     .selector(KERNEL_CS)
///     .dpl_ring(Ring::Ring3)  // allow user mode to invoke this gate
///     .present(true)
///     .gate_interrupt();
/// ```
///
/// ## Safety note
/// - Setting an incorrect ring level (e.g., allowing user access to a privileged handler)
///   can compromise system isolation and security.
/// - Use `Ring::Ring3` only for well-defined syscall or user interrupt entry points.
///
/// For most kernel code, only [`Ring::Ring0`] and [`Ring::Ring3`] are relevant.
///
/// See also: Intel SDM Vol. 3A, §5.5 “Privilege Levels”.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Ring {
    /// **Ring 0 (DPL = 0)** — Kernel or supervisor mode.
    ///
    /// This is the most privileged level. Code running in Ring 0 can execute
    /// any instruction, access any memory region, and perform I/O operations.
    /// Typically used for the operating system kernel and core interrupt handlers.
    Ring0 = 0,

    /// **Ring 1 (DPL = 1)** — Historically for OS components or drivers.
    ///
    /// Rarely used on modern systems. Present for architectural completeness.
    #[deprecated]
    Ring1 = 1,

    /// **Ring 2 (DPL = 2)** — Historically for OS subsystems or drivers.
    ///
    /// Like Ring 1, almost never used in 64-bit mode; modern kernels consolidate
    /// all privileged code in Ring 0.
    #[deprecated]
    Ring2 = 2,

    /// **Ring 3 (DPL = 3)** — User-mode applications.
    ///
    /// The least privileged level. Code running in Ring 3 cannot perform I/O or
    /// privileged instructions and can only access user-mapped memory.
    /// Typical for userland processes and syscall entry points.
    Ring3 = 3,
}

impl Ring {
    #[inline]
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}

impl From<Ring> for u8 {
    #[inline]
    fn from(r: Ring) -> Self {
        r.to_u8()
    }
}

impl TryFrom<u8> for Ring {
    type Error = u8;

    #[inline]
    fn try_from(r: u8) -> Result<Self, Self::Error> {
        if r <= 3 {
            Ok(unsafe { core::mem::transmute::<u8, Self>(r) })
        } else {
            Err(r)
        }
    }
}
