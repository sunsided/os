//! # Kernel Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

mod framebuffer;
mod tracing;

use crate::framebuffer::{VGA_LIKE_OFFSET, fill_solid};
use crate::tracing::trace_boot_info;
use core::hint::spin_loop;
use kernel_alloc::frame_alloc::BitmapFrameAlloc;
use kernel_alloc::phys_mapper::HhdmPhysMapper;
use kernel_alloc::vmm::Vmm;
use kernel_info::boot::{FramebufferInfo, KernelBootInfo};
use kernel_info::memory::HHDM_BASE;
use kernel_qemu::qemu_trace;
use kernel_vmem::{MemoryPageFlags, PhysAddr, VirtAddr};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        spin_loop();
    }
}

/// Stack size.
const BOOT_STACK_SIZE: usize = 64 * 1024;

/// Physical Memory mapper for the Higher-Half Direct Map (HHDM).
static MAPPER: HhdmPhysMapper = HhdmPhysMapper;

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
/// The ABI is defined as `sysv64` (Rust's `extern "C"`), so the kernel is called
/// with the `boot_info` pointer in `RDI` (System V AMD64 ABI, as on Linux/x86_64).
///
/// # Naked function & Stack
/// This is a naked function in order to set up the stack ourselves. Without
/// the `naked` attribute (and the [`naked_asm`](core::arch::naked_asm) instruction), Rust
/// compiler would apply its own assumptions based on the C ABI and would attempt to
/// unwind the stack on the call into [`kernel_entry`]. Since we're clearing out the stack
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

    // Enable interrupts (undo the earlier 'cli')
    unsafe { core::arch::asm!("sti") };

    let bi = unsafe { &*boot_info };
    trace_boot_info(bi);

    let fb_virt = remap_boot_memory(bi);
    kernel_main(&fb_virt)
}

fn kernel_main(fb_virt: &FramebufferInfo) -> ! {
    qemu_trace!("Entering Kernel main loop ...\n");

    let mut cycle = 127u8;
    loop {
        cycle = cycle.wrapping_add(10);
        unsafe { fill_solid(fb_virt, 72, 0, cycle) };
        spin_loop();
    }
}

/// Remaps the boot framebuffer memory into the kernel's virtual address space.
///
/// UEFI provides the physical address of the framebuffer in the boot info, but does not
/// include it in the memory mapping table. This means the kernel must manually map the
/// framebuffer into its own virtual address space to access it. This function sets up the
/// necessary mapping so the framebuffer can be used by the kernel.
fn remap_boot_memory(bi: &KernelBootInfo) -> FramebufferInfo {
    // Set up PMM (bootstrap) and VMM (kernel)
    let mut pmm = BitmapFrameAlloc::new();
    let mut vmm = Vmm::new(&MAPPER, &mut pmm);

    // Map framebuffer
    let fb_pa = bi.fb.framebuffer_ptr;
    let fb_len = bi.fb.framebuffer_size;
    let va_base = HHDM_BASE + VGA_LIKE_OFFSET;
    let fb_flags = MemoryPageFlags::WRITABLE | MemoryPageFlags::GLOBAL | MemoryPageFlags::NX;
    vmm.map_region(
        VirtAddr::from_u64(va_base),
        PhysAddr::from_u64(fb_pa),
        fb_len,
        fb_flags,
    )
    .expect("Framebuffer mapping failed");

    // Return updated FramebufferInfo with new virtual address
    let mut fb_virt = bi.fb.clone();
    fb_virt.framebuffer_ptr = va_base + (fb_pa & 0xFFF); // preserve offset within page
    qemu_trace!("Remapped frame buffer to {va_base:#x}\n");
    fb_virt
}
