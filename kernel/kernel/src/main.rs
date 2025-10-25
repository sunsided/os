//! # Kernel Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

use core::hint::spin_loop;
use kernel_info::{BootPixelFormat, KernelBootInfo};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        spin_loop();
    }
}

const BOOT_STACK_SIZE: usize = 64 * 1024;

#[repr(align(16))] // keep stack 16-byte aligned
struct Aligned<const N: usize>([u8; N]);

#[unsafe(link_section = ".bss.boot")]
#[unsafe(no_mangle)]
static mut BOOT_STACK: Aligned<BOOT_STACK_SIZE> = Aligned([0; BOOT_STACK_SIZE]);

/// Our kernel entry point symbol. The UEFI loader will jump here *after* `ExitBootServices`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start_kernel(boot_info: *const KernelBootInfo) -> ! {
    // TODO: Disable interrupts before messing with the stack.

    // Not actually passing memory but a pointer to the stack.
    #[allow(clippy::pointers_in_nomem_asm_block)]
    unsafe {
        let base: *mut u8 = core::ptr::addr_of_mut!(BOOT_STACK).cast();
        let top = base.add(BOOT_STACK_SIZE);
        core::arch::asm!(
            "mov rsp, {top}",
            "xor rbp, rbp", // set stack pointer base to zero
            top = in(reg) top,
            options(nomem, preserves_flags),
        );
    }

    // TODO: Enable interrupts after messing with the stack.

    // TODO: Assert pointer is not null.
    let boot_info = unsafe { &*boot_info };
    kernel_main(boot_info);
}

fn kernel_main(bi: &KernelBootInfo) -> ! {
    loop {
        unsafe { fill_solid(bi, 255, 0, 0) };
        spin_loop();
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn fill_solid(bi: &KernelBootInfo, r: u8, g: u8, b: u8) {
    unsafe {
        if matches!(bi.framebuffer_format, BootPixelFormat::BltOnly) {
            return; // nothing to draw to
        }

        let mut p = bi.framebuffer_ptr as *mut u8;
        let bpp = 4; // common on PC GOP; for Bitmask you could compute bpp from masks
        let row_bytes = bi.framebuffer_stride * bpp;

        for _y in 0..bi.framebuffer_height {
            let mut row = p;
            for _x in 0..bi.framebuffer_width {
                match bi.framebuffer_format {
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
