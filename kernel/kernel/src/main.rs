//! # Kernel Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

use core::hint::spin_loop;
use kernel_info::{BootPixelFormat, FramebufferInfo, KernelBootInfo};

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

        // save RCX (boot_info per Win64)
        "mov r12, rcx",

        // Build our stack
        "lea rax, [rip + {stack_sym}]",
        "add rax, {stack_size}",
        "mov rsp, rax",
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
    #[cfg(feature = "qemu")]
    kernel_qemu::dbg_print("Kernel reporting to QEMU!\n");

    // (You can enable interrupts here when ready.)
    let bi = unsafe { &*boot_info };
    kernel_main(bi)
}

fn kernel_main(bi: &KernelBootInfo) -> ! {
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print("Entering Kernel main loop ...\n");
        trace_boot_info(bi);
    }

    #[cfg(feature = "qemu")]
    match bi.fb.framebuffer_format {
        BootPixelFormat::Rgb => kernel_qemu::dbg_print("RGB framebuffer\n"),
        BootPixelFormat::Bgr => kernel_qemu::dbg_print("BGR framebuffer\n"),
        BootPixelFormat::Bitmask => kernel_qemu::dbg_print("Bitmask framebuffer\n"),
        BootPixelFormat::BltOnly => kernel_qemu::dbg_print("BltOnly framebuffer\n"),
    }

    loop {
        unsafe { fill_solid(&bi.fb, 255, 0, 0) };
        spin_loop();
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn fill_solid(fb: &FramebufferInfo, r: u8, g: u8, b: u8) {
    unsafe {
        if matches!(fb.framebuffer_format, BootPixelFormat::BltOnly) {
            return; // nothing to draw to
        }

        let mut p = fb.framebuffer_ptr as *mut u8;
        let bpp = 4; // common on PC GOP; for Bitmask you could compute bpp from masks
        let row_bytes = fb.framebuffer_stride * bpp;
        let row_bytes = usize::try_from(row_bytes).unwrap_or_default(); // TODO: Use a panic here

        for _y in 0..fb.framebuffer_height {
            let mut row = p;
            for _x in 0..fb.framebuffer_width {
                match fb.framebuffer_format {
                    BootPixelFormat::Rgb => {
                        core::ptr::write_unaligned(row.add(0), r);
                        core::ptr::write_unaligned(row.add(1), g);
                        core::ptr::write_unaligned(row.add(2), b);
                        core::ptr::write_unaligned(row.add(3), 0xff);
                    }
                    BootPixelFormat::Bgr => {
                        core::ptr::write_unaligned(row.add(0), b);
                        core::ptr::write_unaligned(row.add(1), g);
                        core::ptr::write_unaligned(row.add(2), r);
                        core::ptr::write_unaligned(row.add(3), 0xff);
                    }
                    BootPixelFormat::BltOnly | BootPixelFormat::Bitmask => {
                        // Convert (r,g,b) to a u32 using masks; left as an exercise.
                    }
                }
                row = row.add(4);
            }
            p = p.add(row_bytes);
        }
    }
}

#[cfg(feature = "qemu")]
fn trace_boot_info(boot_info: &KernelBootInfo) {
    use kernel_qemu::dbg_print as trace;
    use kernel_qemu::dbg_print_u64 as trace_num;
    use kernel_qemu::dbg_print_usize as trace_usize;

    trace("Boot Info in Kernel:\n");
    trace("   BI ptr = ");
    trace_usize(core::ptr::from_ref(boot_info) as usize);
    trace("\n");
    trace(" MMAP ptr = ");
    trace_num(boot_info.mmap.mmap_ptr);
    trace(", MMAP len = ");
    trace_num(boot_info.mmap.mmap_len);
    trace(", MMAP desc size = ");
    trace_num(boot_info.mmap.mmap_desc_size);
    trace(", MMAP desc version = ");
    trace_num(boot_info.mmap.mmap_desc_version);
    trace(", rsdp addr = ");
    trace_num(boot_info.mmap.mmap_desc_version);
    trace("\n");
    trace("   FB ptr = ");
    trace_num(boot_info.fb.framebuffer_ptr);
    trace(", FB size = ");
    trace_num(boot_info.fb.framebuffer_size);
    trace(", FB width = ");
    trace_num(boot_info.fb.framebuffer_width);
    trace(", FB height = ");
    trace_num(boot_info.fb.framebuffer_height);
    trace(", FB stride = ");
    trace_num(boot_info.fb.framebuffer_stride);
    trace(", FB format = ");
    match boot_info.fb.framebuffer_format {
        BootPixelFormat::Rgb => trace("RGB"),
        BootPixelFormat::Bgr => trace("BGR"),
        BootPixelFormat::Bitmask => trace("Bitmask"),
        BootPixelFormat::BltOnly => trace("BltOnly"),
    }
    trace("\n");
}
