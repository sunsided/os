//! # Kernel Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

mod bootstrap_alloc;
mod framebuffer;
mod tracing;
mod vmem;

// Bring in the global allocator.
// extern crate kernel_alloc; // disabled temporarily

use crate::framebuffer::fill_solid;
use crate::tracing::trace_boot_info;
use crate::vmem::map_framebuffer_into_hhdm;
use core::hint::spin_loop;
use kernel_info::boot::{FramebufferInfo, KernelBootInfo};
use kernel_qemu::qemu_trace;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        spin_loop();
    }
}

/// Stack size.
const BOOT_STACK_SIZE: usize = 64 * 1024;

/// 16-byte aligned stack
#[repr(align(16))]
struct Aligned<const N: usize>([u8; N]);

#[unsafe(link_section = ".bss.boot")]
#[unsafe(no_mangle)]
static mut BOOT_STACK: Aligned<BOOT_STACK_SIZE> = Aligned([0; BOOT_STACK_SIZE]);

/// The kernel entry point
///
/// # UEFI Interaction
/// The UEFI loader will jump here after `ExitBootServices`.
///
/// # ABI
/// The ABI is defined as `win64` since the kernel is called from a UEFI
/// (PE/COFF) application. This passes the `boot_info` pointer as `RCX`
/// (as opposed to `RDI` for the SysV ABI).
///
/// # Naked function & Stack
/// This is a naked function in order to set up the stack ourselves. Without
/// the `naked` attribute (and the [`naked_asm`](core::arch::naked_asm) instruction), Rust
/// compiler would apply its own assumptions based on the C ABI and would attempt to
/// unwind the stack on the call into [`kernel_entry`]. Since we're clearing out the stack
/// here, this would cause UB.
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "win64" fn _start_kernel(_boot_info: *const KernelBootInfo) {
    core::arch::naked_asm!(
        "cli",

        // These OUTs need no memory; if you see them, trampoline code page is mapped in new CR3.
        "mov    dx, 0x402",
        "mov    al, 'C'",
        "out    dx, al",
        // continue as usual

        // save RCX (boot_info per Win64)
        "mov r12, rcx",

        // Build our own kernel stack and establish a valid call frame for kernel_entry
        "lea rax, [rip + {stack_sym}]",
        "add rax, {stack_size}",
        // Align down to 16
        "and rax, -16",
        // Reserve 32-byte shadow space
        "sub rax, 32",
        // Set RSP to the prepared value
        "mov rsp, rax",
        // Emulate a CALL by pushing a dummy return address (so RSP % 16 == 8 at entry)
        "push 0",
        "xor rbp, rbp",

        // Restore boot_info into the expected arg register (SysV/C ABI)
        "mov rdi, r12",

        // These OUTs need no memory; if you see them, trampoline code page is mapped in new CR3.
        "mov    dx, 0x402",
        "mov    al, 'D'",
        "out    dx, al",
        // continue as usual

        // Jump to Rust entry and never return
        "jmp {rust_entry}",
        stack_sym = sym BOOT_STACK,
        stack_size = const BOOT_STACK_SIZE,
        rust_entry = sym kernel_entry,
    );
}

/// Kernel entry running on normal stack.
///
/// # Notes
/// * `no_mangle` is used so that [`_start_kernel`] can jump to it by name.
/// * It uses C ABI to have a defined convention when calling in from ASM.
/// * The [`_start_kernel`] function keeps `boot_info` in `RDI`, matching C ABI expectations.
#[unsafe(no_mangle)]
extern "C" fn kernel_entry(boot_info: *const KernelBootInfo) -> ! {
    qemu_trace!("Kernel reporting to QEMU!\n");

    // (You can enable interrupts here when ready.)
    let bi = unsafe { &*boot_info };
    kernel_main(bi)
}

fn kernel_main(bi: &KernelBootInfo) -> ! {
    qemu_trace!("Entering Kernel main loop ...\n");
    trace_boot_info(bi);

    let fb_virt = remap_boot_memory(bi);

    let mut cycle = 127u8;
    loop {
        cycle = cycle.wrapping_add(1);
        unsafe { fill_solid(&fb_virt, 72, 0, cycle) };
        spin_loop();
    }
}

/// Map the framebuffer into HHDM at a VGA-like offset and use it virtually
fn remap_boot_memory(bi: &KernelBootInfo) -> FramebufferInfo {
    let (fb_va_base, _mapped_len) = unsafe { map_framebuffer_into_hhdm(&bi.fb) };
    let mut fb_virt = bi.fb.clone();
    fb_virt.framebuffer_ptr = fb_va_base.as_u64();

    qemu_trace!("Remapped frame buffer to {fb_va_base:?}\n");
    fb_virt
}
