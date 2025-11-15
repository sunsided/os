#![no_std]
#![cfg_attr(not(feature = "syscall"), forbid(unsafe_code))]
#![cfg_attr(feature = "syscall", allow(unsafe_code))]

#[cfg(feature = "stdlib")]
#[macro_use]
pub mod stdlib;

#[cfg(feature = "syscall")]
pub mod syscall;

#[cfg(feature = "syscall-abi")]
pub mod syscall_abi;

#[cfg(feature = "stdlib")]
pub use stdlib::*;

#[cfg(feature = "stdlib")]
mod panic {
    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }
}
