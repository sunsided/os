use crate::idt::{idt_update_in_place, init_idt_once};
use crate::init::boot_stack::boot_kstack_top;
use crate::interrupts::Idt;
use crate::interrupts::syscall::SyscallInterrupt;
use crate::tracing::trace_boot_info;
use crate::{gdt, interrupts, kernel_main, remap_boot_memory};
use kernel_info::boot::KernelBootInfo;
use kernel_qemu::qemu_trace;

mod boot_stack;
pub use boot_stack::{BOOT_STACK, BOOT_STACK_SIZE};

/// The kernel entry point
///
/// # UEFI Interaction
/// The UEFI loader will jump here after `ExitBootServices`.
///
/// # ABI
/// The ABI is defined as `sysv64` (Rust's `extern "C"`), so the kernel is called
/// with the `boot_info` pointer in `RDI` (System V AMD64 ABI, as on Linux/x86_64).
///
/// # Naked function & Stack
/// This is a naked function in order to set up the stack ourselves. Without
/// the `naked` attribute (and the [`naked_asm`](core::arch::naked_asm) instruction), Rust
/// compiler would apply its own assumptions based on the C ABI and would attempt to
/// unwind the stack on the call into [`kernel_entry_on_boot_stack`]. Since we're clearing out the stack
/// here, this would cause UB.
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start_kernel(_boot_info: *const KernelBootInfo) {
    core::arch::naked_asm!(
        "cli",
        // save RDI (boot_info per SysV64)
        "mov r12, rdi",
        // Build our own kernel stack and establish a valid call frame for kernel_entry
        "lea rax, [rip + {stack_sym}]",
        "add rax, {stack_size}",
        // Align down to 16
        "and rax, -16",
        // Set RSP to the prepared value
        "mov rsp, rax",
        // Emulate a CALL by pushing a dummy return address (so RSP % 16 == 8 at entry)
        "push 0",
        "xor rbp, rbp",
        // Restore boot_info into the expected arg register (SysV/C ABI)
        "mov rdi, r12",
        // Jump to Rust entry and never return
        "jmp {rust_entry}",
        stack_sym = sym BOOT_STACK,
        stack_size = const BOOT_STACK_SIZE,
        rust_entry = sym kernel_entry_on_boot_stack,
    );
}

/// Kernel entry running on the boot stack ([`BOOT_STACK`]).
///
/// # Notes
/// * `no_mangle` is used so that [`_start_kernel`] can jump to it by name.
/// * It uses C ABI to have a defined convention when calling in from ASM.
/// * The [`_start_kernel`] function keeps `boot_info` in `RDI`, matching C ABI expectations.
#[unsafe(no_mangle)]
pub extern "C" fn kernel_entry_on_boot_stack(boot_info: *const KernelBootInfo) -> ! {
    qemu_trace!("Kernel reporting to QEMU!\n");

    early_kernel_init_arch();
    qemu_trace!("Early kernel init done\n");

    // Enable interrupts (undo the earlier 'cli')
    unsafe { core::arch::asm!("sti") };

    let bi = unsafe { &*boot_info };
    trace_boot_info(bi);

    let fb_virt = remap_boot_memory(bi);
    kernel_main(&fb_virt)
}

fn early_kernel_init_arch() {
    // TODO: 1. Start on the boot stack (your _start_kernel does this).
    // TODO: 2. Bring up the minimal MM you need to map memory.
    // TODO: 3. Allocate & map a per-CPU kernel stack (with a guard page), compute its 16-byte–aligned top.
    // TODO: 4. Switch rsp to that new top.
    // TODO: 5. Now call gdt::init_gdt_and_tss(new_top, ist) and then IDT setup.

    // GDT + TSS (loads GDT via lgdt and TSS via ltr)
    qemu_trace!("Allocating boot kernel stack\n");
    let kstack_top = boot_kstack_top(); // TODO: Bad idea, should allocate proper kernel stack here

    qemu_trace!("Initializing GDT and TSS ...\n");
    gdt::init_gdt_and_tss(kstack_top, None); // TODO: feeds boot stack into TSS.rsp0

    // Initialize the IDT once.
    qemu_trace!("Initializing IDT ...\n");

    unsafe {
        init_idt_once(Idt::new());
    }

    // Update the IDT. Enters a critical section and (re-)enables interrupts
    // when the function returns.
    qemu_trace!("Installing interrupt handlers ...\n");
    idt_update_in_place(|idt| {
        idt.init_syscall_gate(interrupts::int80_entry::int80_entry);
    });
}
