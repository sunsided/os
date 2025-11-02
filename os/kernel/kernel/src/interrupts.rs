//! # x86-64 Interrupt Descriptor Table (IDT)
//!
//! Minimal, `no_std`-friendly IDT implementation for x86-64 kernels with a
//! fluent builder API. It lets you declare gates like this:
//!
//! ```ignore
//! // Install a classic int 0x80 syscall gate that userland may invoke (DPL=3):
//! idt[0x80]
//!     .set_handler(syscall_int80_handler)
//!     .selector(KERNEL_CS)                 // run handler in the kernel code segment
//!     .dpl(3)                              // allow ring-3 `int 0x80`
//!     .present(true)                       // mark entry valid
//!     .gate_interrupt();                   // mask IF on entry
//!
//! // Finally, load the IDT (must be done at CPL0):
//! unsafe { idt.load(); }
//! ```
//!
//! ## Acronyms (kept close at hand)
//! - **IDT** – *Interrupt Descriptor Table* (256 entries describing traps/IRQs)
//! - **ISR** – *Interrupt Service Routine* (your handler function)
//! - **DPL** – *Descriptor Privilege Level* (0=kernel … 3=user)
//! - **IST** – *Interrupt Stack Table* (per-entry alternate stack from the TSS)
//! - **TSS** – *Task State Segment* (holds `rsp0` and up to 7 IST pointers)
//! - **P** – *Present* bit (must be 1 for a valid gate)
//!
//! ## Design notes
//! - Exact 16-byte entry layout per Intel SDM, chapter “Interrupt Descriptor Table”.
//! - A tiny bitfield helper ([`IdtGateAttr`]) encodes the middle two bytes
//!   (IST + type/attrs) using [`bitfield_struct`], while the rest stays `repr(C)`
//!   to keep offsets crystal-clear.
//! - Builder methods return `Self` for concise chaining and are `const` where
//!   possible.
//!
//! ## Safety considerations
//! - Calling [`Idt::load`] is `unsafe`: you must be in CPL0 with a valid IDT,
//!   valid handler pointers, and a sane TSS (e.g., `tss.rsp0` points to a
//!   kernel stack for privilege transitions).
//! - Mark entries `.present(true)` **only** once the handler and segments are
//!   correctly initialized.
//! - If you use IST, ensure the chosen `tss.istN` points at a properly sized,
//!   non-shared stack.
//!
//! ## When to use Trap vs. Interrupt gates
//! - **Interrupt gates** clear IF on entry (masking maskable interrupts). Good
//!   for most ISRs and for a simple `int 0x80` syscall path.
//! - **Trap gates** leave IF unchanged. Useful for debugging and certain faults.

mod syscall;

use bitfield_struct::bitfield;
use core::arch::asm;
use core::mem::size_of;
use core::ops::{Index, IndexMut};

// Compile-time layout sanity checks for the architecture.
//
// An IDT entry **must** be 16 bytes, and the table benefits from
// 16-byte alignment for the `lidt` limit calculation and common conventions.
const _: () = assert!(size_of::<IdtEntry>() == 16);
const _: () = assert!(align_of::<Idt>() == 16);

/// Two bytes of an IDT entry that pack:
///
/// - **low byte**: `IST` (3 bits) + 5 reserved zero bits
/// - **high byte**: `| P | DPL(2) | S(0) | Type(4) |`
///
/// On little-endian x86-64, this maps cleanly to a `u16`.
#[bitfield(u16)]
pub struct IdtGateAttr {
    /// **IST** – Interrupt Stack Table index (0 disables IST switching).
    ///
    /// Requires a properly initialized **TSS** with `ist[index]` stack pointers.
    #[bits(3)]
    pub ist: u8,

    /// Must be zero (hardware-reserved).
    #[bits(5)]
    __zero0: u8,

    /// **Type** – 0xE = *Interrupt gate*, 0xF = *Trap gate*.
    #[bits(4)]
    pub typ: u8,

    /// **S** – System bit (must be `0` for interrupt/trap gates).
    #[bits(1)]
    pub s: bool,

    /// **DPL** – Descriptor Privilege Level (0..=3).
    ///
    /// To allow invocation from user mode via `int n`, set DPL to `3`.
    #[bits(2)]
    pub dpl: u8,

    /// **P** – Present bit. Must be `1` for a valid entry.
    #[bits(1)]
    pub present: bool,
}

impl IdtGateAttr {
    /// Convenience constructor for an **Interrupt Gate** (type 0xE, S=0).
    #[inline]
    #[must_use]
    pub const fn interrupt_gate() -> Self {
        Self::new().with_typ(0xE).with_s(false)
    }

    /// Convenience constructor for a **Trap Gate** (type 0xF, S=0).
    #[inline]
    #[must_use]
    pub const fn trap_gate() -> Self {
        Self::new().with_typ(0xF).with_s(false)
    }
}

/// A 256-entry **Interrupt Descriptor Table**.
///
/// The table itself is 16-byte aligned. Use [`Idt::new`] to create a cleared
/// table (all entries non-present), mutate entries via indexing, and finally
/// load it with [`Idt::load`].
#[repr(C, align(16))]
pub struct Idt {
    entries: [IdtEntry; 256],
}

impl Default for Idt {
    fn default() -> Self {
        Self::new()
    }
}

impl Idt {
    /// Create a new, empty IDT with all gates marked **not present**.
    ///
    /// This is safe to construct at any time. Entries must be filled and marked
    /// present before calling [`load`](Self::load).
    pub const fn new() -> Self {
        Self {
            entries: [IdtEntry::MISSING; 256],
        }
    }

    /// Load this IDT into the CPU’s **IDTR** using `lidt`.
    ///
    /// # Safety
    /// - Must be called at **CPL0**.
    /// - All **present** entries must reference valid handler code in an
    ///   executable segment.
    /// - If any entry is callable from user mode (DPL=3), ensure your **TSS**
    ///   (especially `rsp0`) is configured for safe privilege transitions.
    #[inline]
    pub unsafe fn load(&'static self) {
        let idtr = Idtr {
            limit: (size_of::<Self>() - 1) as u16,
            base: core::ptr::from_ref(self) as u64,
        };
        unsafe {
            asm!("lidt [{}]", in(reg) &raw const idtr, options(nostack, preserves_flags, readonly));
        }
    }
}

impl Index<usize> for Idt {
    type Output = IdtEntry;
    fn index(&self, i: usize) -> &Self::Output {
        &self.entries[i]
    }
}

impl IndexMut<usize> for Idt {
    fn index_mut(&mut self, i: usize) -> &mut Self::Output {
        &mut self.entries[i]
    }
}

/// Operand format used by `lidt` (limit + base).
#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

/// One **16-byte** x86-64 IDT gate descriptor.
///
/// Layout summary (Intel SDM, “Interrupt Descriptor Table”):
///
/// ```text
/// 63                          48 47    40 39  38  37  36 35            32
/// +----------------------------+--------+------+------+--+----------------+
/// |         offset[63:32]     |  zero  |  P   | DPL  |S |   type (0xE/F) |
/// +----------------------------+--------+------+------+--+----------------+
/// 31            16 15        0
/// +----------------+---------+
/// | offset[31:16]  | selector|
/// +----------------+---------+
/// 79             72 71     64
/// +----------------+---------+
/// |  IST (3 bits)  | offset[15:0]
/// +----------------+---------+
/// 127            96
/// +---------------------------+
/// |            zero           |
/// +---------------------------+
/// ```
///
/// **Key fields**
/// - `selector`: code segment selector for the handler (usually your `KERNEL_CS`)
/// - `dpl`: privilege required to invoke via software `int`
/// - `present`: must be `true` for the CPU to accept the gate
/// - `type`: 0xE (*Interrupt*) or 0xF (*Trap*)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct IdtEntry {
    offset_lo: u16,
    selector: u16,
    /// Two bytes packed via `IdtGateAttr` (IST + type/attrs).
    ist_type: u16, // manipulated through IdtGateAttr
    offset_mid: u16,
    offset_hi: u32,
    zero: u32,
}

/// Gate kinds supported by this IDT.
///
/// - [`GateType::InterruptGate`] masks further maskable interrupts upon entry (clears IF).
/// - [`GateType::TrapGate`] leaves IF unchanged (useful for debug/fault handlers).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GateType {
    /// Masks further maskable interrupts upon entry (clears `IF`).
    InterruptGate,
    /// Leaves `IF` unchanged; useful for debugging/tracing faults.
    TrapGate,
}

impl IdtEntry {
    /// A zeroed, non-present entry.
    pub const MISSING: Self = Self {
        offset_lo: 0,
        selector: 0,
        ist_type: IdtGateAttr::new().into_bits(),
        offset_mid: 0,
        offset_hi: 0,
        zero: 0,
    };

    /// Initialize this entry with a handler function and return a fluent builder.
    ///
    /// This stores the handler address and defaults the selector to the current
    /// CS (see [`current_cs`]). You can override the selector via
    /// [`IdtEntryBuilder::selector`].
    ///
    /// The entry is **not** marked present by default; call
    /// [`IdtEntryBuilder::present`] when you are ready.
    pub fn set_handler(&mut self, handler: fn()) -> IdtEntryBuilder<'_> {
        let addr = handler as u64;
        self.offset_lo = (addr & 0xFFFF) as u16;
        self.offset_mid = ((addr >> 16) & 0xFFFF) as u16;
        self.offset_hi = (addr >> 32) as u32;
        self.selector = current_cs();

        // default: present=0, DPL=0, interrupt gate, IST=0
        self.ist_type = IdtGateAttr::interrupt_gate()
            .with_present(false)
            .with_dpl(0)
            .with_ist(0)
            .into_bits();

        IdtEntryBuilder { entry: self }
    }
}

/// Fluent builder for an [`IdtEntry`].
///
/// Typical use:
///
/// ```ignore
/// idt[0x80].set_handler(syscall_int80_handler)
///     .selector(KERNEL_CS)
///     .dpl(3)
///     .present(true)
///     .gate_interrupt(); // or `.gate_type(GateType::InterruptGate)`
/// ```
pub struct IdtEntryBuilder<'a> {
    entry: &'a mut IdtEntry,
}

impl IdtEntryBuilder<'_> {
    /// Set the **Present** bit. Must be `true` for a usable gate.
    #[inline]
    pub const fn present(self, p: bool) -> Self {
        let bf = IdtGateAttr::from_bits(self.entry.ist_type).with_present(p);
        self.entry.ist_type = bf.into_bits();
        self
    }

    /// Set **DPL** (Descriptor Privilege Level), 0..=3.
    ///
    /// To allow user-mode code to trigger this gate via `int n`, set `dpl(3)`.
    #[inline]
    pub fn dpl(self, dpl: u8) -> Self {
        debug_assert!(dpl <= 3);
        let bf = IdtGateAttr::from_bits(self.entry.ist_type).with_dpl(dpl);
        self.entry.ist_type = bf.into_bits();
        self
    }

    /// Make this an **Interrupt Gate** (type 0xE, `S=0`).
    #[inline]
    pub const fn gate_interrupt(self) -> Self {
        let bf = IdtGateAttr::from_bits(self.entry.ist_type)
            .with_typ(0xE)
            .with_s(false);
        self.entry.ist_type = bf.into_bits();
        self
    }

    /// Make this a **Trap Gate** (type 0xF, `S=0`).
    #[inline]
    pub const fn gate_trap(self) -> Self {
        let bf = IdtGateAttr::from_bits(self.entry.ist_type)
            .with_typ(0xF)
            .with_s(false);
        self.entry.ist_type = bf.into_bits();
        self
    }

    /// Choose the gate type via an enum.
    #[inline]
    pub const fn gate_type(self, gate_type: GateType) -> Self {
        match gate_type {
            GateType::InterruptGate => self.gate_interrupt(),
            GateType::TrapGate => self.gate_trap(),
        }
    }

    /// Set the **IST** index (0 disables IST switching).
    ///
    /// # Panics (debug only)
    /// Asserts `idx <= 7`. Hardware supports `1..=7`.
    #[inline]
    pub fn ist(self, idx: u8) -> Self {
        debug_assert!(idx <= 7);
        let bf = IdtGateAttr::from_bits(self.entry.ist_type).with_ist(idx);
        self.entry.ist_type = bf.into_bits();
        self
    }

    /// Override the code segment **selector** (defaults to the current CS).
    #[inline]
    pub const fn selector(self, sel: u16) -> Self {
        self.entry.selector = sel;
        self
    }
}

/// Read the current **CS** selector (used as a sensible default for entries).
#[inline]
fn current_cs() -> u16 {
    let cs: u16;
    unsafe {
        asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
    }
    cs
}
