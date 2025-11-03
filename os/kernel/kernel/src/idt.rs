//! # Interrupt Descriptor Table (IDT)
//!
//! The **IDT** tells the CPU which handler to call for each **interrupt**, **exception**,
//! or **software trap**. In x86-64, the IDT is an array of 16-byte gate descriptors,
//! and a pointer to it is stored in the **IDTR** (loaded via `lidt`).
//!
//! ## How the IDT relates to the GDT/TSS
//! Although most *segmentation* is off in long mode, privilege transitions on
//! interrupts still consult state provided by your **GDT/TSS** setup:
//!
//! - On a transition from Ring-3 → Ring-0 (e.g., a syscall interrupt gate with DPL=3,
//!   or a user fault), the CPU **loads the stack pointer from the TSS** (`rsp0`).
//!   This requires that you have installed a valid TSS descriptor in the GDT and
//!   executed `ltr` once. See [`crate::gdt::init_gdt_and_tss`] and [`crate::gdt::TSS_SEL`].
//! - If an IDT entry uses an **IST** (Interrupt Stack Table) slot, the CPU will switch
//!   to the corresponding `istN` stack from the **TSS** before entering the handler.
//!   This is how you can make double-fault or NMI handlers robust even if the
//!   current stack is corrupted. See [`crate::gdt::Tss64`] and your TSS initialization.
//!
//! > In short: **load the GDT and TSS first**, then `lidt`, then enable interrupts.
//!
//! ## What this module provides
//! - A single, **global** IDT storage (`static mut IDT`) and helpers to:
//!   - Install it once into the IDTR: [`init_idt_once`]
//!   - Mutate it safely in place without reloading IDTR: [`idt_update_in_place`]
//!   - Borrow a mutable reference when you know what you’re doing: [`idt_mut`]
//!
//! The pattern is intentionally simple for a bootstrap CPU. If we later move to a
//! per-CPU IDT, we can keep the same call sites and swap the backing storage.
//!
//! ## Quick start
//! ```no_run
//! use crate::interrupts::Idt;
//!
//! //Build an IDT with your stubs/gates.
//! let mut idt = Idt::new();
//! idt.init_exceptions();            // your helper
//! idt.init_irq_gates();             // your helper
//! idt.init_syscall_gate(int80_stub); // example
//!
//! // Make sure GDT+TSS are installed once (stack for ring changes / IST).
//! //    See: crate::gdt::init_gdt_and_tss
//!
//! // Load the IDT into IDTR.
//! unsafe { crate::idt::init_idt_once(idt) };
//!
//! // Enable interrupts when ready.
//! unsafe { core::arch::asm!("sti") };
//! ```
//!
//! ## Safety & concurrency model
//! - **Install once per CPU** before enabling interrupts. This module currently holds
//!   a single global IDT (shared by CPUs). If multiple cores update entries, serialize
//!   with your chosen mechanism (IPI + stop-the-world, spinlocks, etc.).
//! - [`idt_update_in_place`] wraps updates in `cli`/`sti` on the **local** CPU and
//!   issues a full memory fence to make changes visible before interrupts resume.
//! - You **do not** need to execute `lidt` again when editing entries **in place**,
//!   as long as the base address and limit of the table remain unchanged.
//!
//! ## Ordering checklist (typical bootstrap)
//! 1. Map memory and enter long mode.
//! 2. **GDT/TSS:** call [`crate::gdt::init_gdt_and_tss`] (loads GDT via `lgdt`, sets up TSS, executes `ltr`).
//! 3. **IDT:** build an `Idt`, then call [`init_idt_once`] (loads IDTR via `lidt`).
//! 4. Configure PIC/APIC as needed, then `sti`.
//!
//! With this sequence, user→kernel transitions will get a sane Ring-0 stack via
//! TSS.`rsp0`, and critical handlers can use IST stacks if you configured them.

use crate::interrupts::Idt;
use core::mem::MaybeUninit;
use core::sync::atomic;

/// The global interrupt descriptor table.
static mut IDT: MaybeUninit<Idt> = MaybeUninit::uninit();

/// Initialize the **global Interrupt Descriptor Table (IDT)** once and load it into the CPU.
///
/// # Overview
/// This function permanently installs the given [`Idt`] as the system-wide interrupt table
/// by:
/// 1. Writing it into the global static storage `IDT`, and
/// 2. Executing the `lidt` instruction to load its base and limit into the CPU’s **IDTR**.
///
/// After this call, the CPU will consult the given table whenever an interrupt,
/// exception, or software trap occurs.
///
/// # Safety
/// - Must be called **exactly once per CPU** before any interrupts are enabled.
/// - `idt` must be a fully initialized and valid interrupt table whose memory remains
///   **permanently allocated** (e.g. in static storage).
/// - This function is `unsafe` because it alters the CPU’s global interrupt state.
///   Incorrect usage may lead to undefined behavior if the IDT points to invalid handlers.
///
/// # Notes
/// - The `lidt` instruction only loads a pointer; the CPU fetches entries from memory
///   dynamically. You may therefore modify entries in place later without re-executing
///   `lidt`, provided the table base and size remain unchanged.
pub unsafe fn init_idt_once(idt: Idt) {
    #[allow(static_mut_refs)]
    unsafe {
        IDT.write(idt);
        IDT.assume_init_ref().load();
    }
}

/// Borrow a mutable reference to the **global IDT**.
///
/// # Safety
/// - The global IDT must have been previously initialized with [`init_idt_once`].
/// - The returned reference has `'static` lifetime and must not be aliased mutably
///   from multiple threads or CPUs simultaneously without synchronization.
/// - You typically use this in short critical sections where interrupts are disabled.
///
/// # Example
/// ```no_run
/// unsafe {
///     let idt = idt_mut();
///     idt.set_handler(0x80, syscall_entry);
/// }
/// ```
unsafe fn idt_mut() -> &'static mut Idt {
    #[allow(static_mut_refs)]
    unsafe {
        IDT.assume_init_mut()
    }
}

/// Update entries of the global IDT **in place**, without reloading `lidt`.
///
/// # Behavior
/// - Disables interrupts locally (`cli`) to prevent concurrent entry during mutation.
/// - Applies the user-supplied closure `f(&mut Idt)`.
/// - Executes a full memory fence to ensure visibility of updates before re-enabling interrupts.
/// - Re-enables interrupts (`sti`) after modification.
///
/// This is the preferred mechanism for dynamically patching interrupt vectors (e.g.
/// installing a new syscall handler or replacing an exception stub) when the IDT base
/// address and limit remain unchanged.
///
/// # Safety & Concurrency
/// - Because the IDT is shared between all CPUs, ensure that concurrent updates from
///   other cores are serialized appropriately (e.g. via IPI, spinlock, or stop-the-world).
/// - Updates to the same IDT entry must be atomic with respect to interrupt delivery.
/// - Avoid modifying critical vectors (NMI, double-fault) while they are active.
///
/// # Example
/// ```no_run
/// // Swap syscall handler at runtime.
/// idt_update_in_place(|idt| {
///     idt.init_syscall_gate(new_int80_entry);
/// });
/// ```
pub fn idt_update_in_place<F: FnOnce(&mut Idt)>(f: F) {
    #[allow(static_mut_refs)]
    unsafe {
        debug_assert!(IDT.assume_init_ref().is_loaded(), "IDT is not installed");
    }

    unsafe {
        // Disable interrupts locally while mutating this CPU’s view of the table.
        core::arch::asm!("cli", options(nostack, preserves_flags));
        let idt = idt_mut();
        f(idt);

        // Ensure the write completes before re-enabling interrupts.
        atomic::fence(atomic::Ordering::SeqCst);
        core::arch::asm!("sti", options(nostack, preserves_flags));
    }
}
