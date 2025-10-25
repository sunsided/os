//! # Kernel Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

use core::hint::spin_loop;

#[repr(C)]
pub struct BootInfo {
    pub framebuffer_ptr: u64,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_stride: u32,
    pub reserved: u32,
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        spin_loop();
    }
}

/// Our kernel entry point symbol. The UEFI loader will jump here *after* `ExitBootServices`.
#[unsafe(no_mangle)]
pub extern "C" fn _start_kernel(boot_info: *const BootInfo) -> ! {
    // Do something trivial so we can tell we got here.
    // (Nothing to print yet—no console. We’ll just spin.)
    let _ = boot_info; // keep the parameter “used” for now

    loop {
        spin_loop();
    }
}
