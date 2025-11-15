#![no_std]
#![no_main]

use stdlib::{print_hex, syscall};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // INT 0x80
    #[allow(deprecated)]
    {
        let v = syscall::int80::sys_bogus_int80();
        stdlib::print_hex_int80(v);
        syscall::int80::debug_byte_int80(b'\n');
    }

    // SYSCALL
    {
        let v2 = syscall::sys_bogus();
        print_hex(v2);
        syscall::debug_byte(b'\n');
    }

    loop {
        core::hint::spin_loop();
    }
}
