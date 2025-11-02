use crate::interrupts::{GateType, Idt};

// segment selectors (example values that match a typical GDT layout)
#[allow(dead_code)]
pub const KERNEL_CS: u16 = 0x08;

#[allow(dead_code)]
pub const KERNEL_DS: u16 = 0x10;

#[allow(dead_code)]
pub const USER_CS: u16 = 0x1b; // index=0x18, RPL=3

#[allow(dead_code)]
pub const USER_DS: u16 = 0x23; // index=0x20, RPL=3

pub trait SyscallInterrupt {
    fn init_syscall_gate(&mut self, handler: fn());
}

impl SyscallInterrupt for Idt {
    fn init_syscall_gate(&mut self, handler: fn()) {
        self[0x80]
            .set_handler(handler)
            .selector(KERNEL_CS) // ensure it targets kernel code segment
            .present(true)
            .user_callable()
            .gate_type(GateType::InterruptGate);
    }
}
