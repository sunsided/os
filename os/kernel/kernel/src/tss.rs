//! # Minimal 64-bit Task State Segment (TSS) for x86_64
//!
//! ## What is the TSS for in 64-bit mode?
//! In 64-bit mode, the CPU no longer performs hardware task switching, but it
//! **still uses the TSS** to find safe stacks when transitioning from user
//! space (Ring-3) to the kernel (Ring-0), and optionally for **IST** stacks.
//!
//! - `rsp0` is the **Ring-0 stack** used automatically on privilege transitions
//!   (e.g., `int 0x80`, page faults, syscalls via interrupt gates).
//! - `ist1..ist7` are optional **Interrupt Stack Table** entries that you can
//!   bind to specific IDT entries to guarantee a clean stack for critical
//!   faults (like double fault) even if your normal kernel stack is corrupted.
//! - `iopb_offset` points to the I/O permission bitmap. Setting it to the
//!   **end of the TSS** disables the bitmap (typical for kernels).
//!
//! ## How we use it
//! - `init_tss(rsp0, ist1)` fills a global TSS with a kernel stack and
//!   optional IST1. You will reference this TSS from the GDT's **TSS
//!   descriptor**, then load it into the **Task Register** via `ltr`.
//! - Later, you can change `rsp0` via `set_rsp0()`.
//!
//! For SMP, create one TSS per CPU and load the CPU-local TSS on AP startup.

use crate::per_cpu::PerCpu;
use core::mem::size_of;
use kernel_vmem::addresses::VirtualAddress;

/// 64-bit Task State Segment (TSS) as used by x86-64 long mode.
///
/// In 64-bit mode, the CPU no longer performs hardware task switches, but it
/// **still consults the TSS** for two critical things:
///
/// 1) **Privilege Stack Table (PST)** — the `rsp0..rsp2` pointers.
///    When the processor enters the kernel through an **interrupt/exception/trap
///    gate** that causes a **privilege-level change** (e.g., Ring-3 → Ring-0 on
///    `int 0x80`, page fault from userland, etc.), it loads the new stack
///    pointer from the PST entry corresponding to the target CPL (for Ring-0
///    transitions, `rsp0`). This guarantees entry on a **kernel-controlled**
///    stack even if userland’s RSP is bogus.
///
/// 2) **Interrupt Stack Table (IST)** — the `ist1..ist7` pointers.
///    If the IDT gate used to deliver an interrupt/exception specifies a **non-zero
///    IST index** (1..7), the processor will ignore the normal PST and instead
///    switch to the specified **IST stack** before pushing the interrupt frame.
///    This is essential for handling cases like **double faults** safely (when
///    your current stack may be corrupted).
///
/// The **I/O Permission Bitmap** (controlled via `iopb_offset`) optionally
/// restricts `in`/`out` port I/O for tasks at CPL > IOPL. Most kernels either
/// **disable** the bitmap for the kernel or place a bitmap at the end of the TSS
/// to **deny all** user I/O by default.
///
/// Notes:
/// - All `_reserved*` fields must be zero.
/// - The TSS can reside at any (canonical) address; the GDT holds a 16-byte TSS
///   system descriptor that points to it.
/// - If you **don’t** use an IST for an IDT entry, the CPU will use the PST
///   (`rsp0..2`) when there is a privilege change; otherwise it continues on the
///   current stack.
/// - `syscall/sysret` do **not** consult the TSS; they use MSRs (`STAR/LSTAR/SFMASK`)
///   and usually `swapgs` with a per-CPU stack scheme. Interrupt-gate-based
///   syscalls (e.g., `int 0x80`) **do** use the TSS.
///
#[repr(C, packed)]
pub struct Tss64 {
    /// Must be zero. Reserved by the architecture.
    ///
    /// The first 4 bytes of the 64-bit TSS layout are reserved to align the
    /// following `rsp0` field on an 8-byte boundary.
    _reserved0: u32,

    /* ─────────────── Privilege Stack Table (PST) ─────────────── */
    /// **Ring-0 stack pointer (RSP0)** used on privilege elevation.
    ///
    /// When the CPU delivers an interrupt/exception/trap to a gate that changes
    /// CPL to 0 (e.g., user → kernel), it **loads RSP from `rsp0`** and then
    /// pushes the saved user context on this kernel stack before entering the
    /// handler. This is how you ensure a trustworthy stack on kernel entry.
    ///
    /// Typical kernel setup: point this to the **top of a kernel stack** that
    /// grows downward. In SMP systems you usually set a **different `rsp0` per
    /// CPU** (i.e., one TSS per CPU).
    pub rsp0: VirtualAddress,

    /// **Ring-1 stack pointer (RSP1)** — rarely used in modern OSes.
    ///
    /// If you ever use CPL=1 code segments (uncommon), a privilege transition
    /// that targets CPL=1 would load RSP from here.
    pub rsp1: VirtualAddress,

    /// **Ring-2 stack pointer (RSP2)** — rarely used in modern OSes.
    ///
    /// Analogous to `rsp1`, but for transitions targeting CPL=2.
    pub rsp2: VirtualAddress,

    /// Must be zero. Reserved by the architecture.
    _reserved1: u64,

    /* ─────────────── Interrupt Stack Table (IST) ─────────────── */
    /// **IST1**: optional dedicated stack for an IDT entry.
    ///
    /// If an IDT gate has its IST index field set to 1, the CPU loads RSP from
    /// this field **before** pushing the interrupt frame, ensuring the handler
    /// runs on this stack **regardless of privilege level**. Use for critical
    /// handlers (e.g., **double fault**) to avoid recursion/corruption from a
    /// failing main stack.
    pub ist1: VirtualAddress,

    /// **IST2**: optional dedicated stack for an IDT entry (IST index = 2).
    pub ist2: VirtualAddress,

    /// **IST3**: optional dedicated stack for an IDT entry (IST index = 3).
    pub ist3: VirtualAddress,

    /// **IST4**: optional dedicated stack for an IDT entry (IST index = 4).
    pub ist4: VirtualAddress,

    /// **IST5**: optional dedicated stack for an IDT entry (IST index = 5).
    pub ist5: VirtualAddress,

    /// **IST6**: optional dedicated stack for an IDT entry (IST index = 6).
    pub ist6: VirtualAddress,

    /// **IST7**: optional dedicated stack for an IDT entry (IST index = 7).
    pub ist7: VirtualAddress,

    /// Must be zero. Reserved by the architecture.
    _reserved2: u64,

    /// Must be zero. Reserved by the architecture.
    _reserved3: u16,

    /// Byte offset from the **base of this TSS** to the start of the **I/O Permission Bitmap**.
    ///
    /// - If this **offset is greater than or equal to the TSS limit**, then
    ///   **no I/O bitmap is present**. In that case, access to `in/out`
    ///   instructions is governed solely by **IOPL vs CPL** (typical kernels run
    ///   with IOPL=0 and user CPL=3, so userland port I/O will fault).
    /// - If present, each **bit** corresponds to one I/O port: a set bit
    ///   generally **disallows** access to that port when CPL > IOPL; a clear bit
    ///   **allows** it. The bitmap must be **byte-aligned** and, per manuals,
    ///   should be followed by a **guard/terminator byte** with all bits set.
    ///
    /// Most kernels:
    /// - set this to `core::mem::size_of::<Tss64>() as u16` to **disable** the
    ///   bitmap for the kernel TSS, or
    /// - place a bitmap at the end and fill it with `0xFF` to **deny all** user
    ///   port I/O by default (granting specific ports only if needed).
    pub iopb_offset: u16,
}

impl Tss64 {
    #[allow(clippy::cast_possible_truncation)]
    pub const fn new() -> Self {
        Self {
            _reserved0: 0,
            rsp0: VirtualAddress::zero(),
            rsp1: VirtualAddress::zero(),
            rsp2: VirtualAddress::zero(),
            _reserved1: 0,
            ist1: VirtualAddress::zero(),
            ist2: VirtualAddress::zero(),
            ist3: VirtualAddress::zero(),
            ist4: VirtualAddress::zero(),
            ist5: VirtualAddress::zero(),
            ist6: VirtualAddress::zero(),
            ist7: VirtualAddress::zero(),
            _reserved2: 0,
            _reserved3: 0,
            iopb_offset: size_of::<Self>() as u16,
        }
    }
}

/// Initialize the TSS with a kernel RSP0 and optional IST1.
///
/// * `kernel_stack_top` — top (highest address) of a valid kernel stack.
///   The CPU switches to this when entering Ring-0 from Ring-3 via an
///   interrupt/exception gate (e.g., `int 0x80`).
/// * `ist1_top` — top of an IST1 stack for critical handlers (you
///   later bind an IDT entry to IST1).
pub const fn init_tss(p: &mut PerCpu, kernel_stack_top: VirtualAddress, ist1_top: VirtualAddress) {
    let tss = &mut p.tss;

    tss.rsp0 = kernel_stack_top;
    tss.ist1 = ist1_top;
}

/// Update the Ring-0 stack pointer used on user→kernel transitions.
#[allow(dead_code)]
pub const fn set_rsp0(p: &mut PerCpu, new_top: VirtualAddress) {
    p.tss.rsp0 = new_top;
}
