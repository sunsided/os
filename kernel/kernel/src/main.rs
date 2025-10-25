//! # Kernel Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

use core::hint::spin_loop;

#[repr(C)]
pub struct BootInfo {
    pub framebuffer_ptr: u64,
    pub framebuffer_width: usize,
    pub framebuffer_height: usize,
    pub framebuffer_stride: usize,
    pub reserved: u32,
}

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
pub extern "C" fn _start_kernel(_boot_info: *const BootInfo) -> ! {
    unsafe {
        let base: *mut u8 = core::ptr::addr_of_mut!(BOOT_STACK).cast();
        let top = base.add(BOOT_STACK_SIZE);
        core::arch::asm!("mov rsp, {}", in(reg) top, options(nostack, nomem));
    }

    loop {
        kernel_main();
    }
}

fn kernel_main() {
    spin_loop();
}
