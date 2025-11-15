#![no_std]
#![no_main]

use stdlib::{println, syscall};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Init process started successfully!");

    #[allow(deprecated)]
    {
        println!("Performing legacy INT80h syscall ...");
        let v = syscall::int80::sys_bogus_int80();
        println!("Returned value: 0x{v:04X}");
    }

    {
        println!("Performing syscall ...");
        let v2 = syscall::sys_bogus();
        println!("Returned value: 0x{v2:04X}");
    }

    loop {
        core::hint::spin_loop();
    }
}
