use crate::gdt::KERNEL_CS_SEL;
use crate::interrupts::{GateType, Idt};

pub trait SyscallInterrupt {
    fn init_syscall_gate(&mut self, handler: extern "C" fn()) -> &mut Self;
}

impl SyscallInterrupt for Idt {
    fn init_syscall_gate(&mut self, handler: extern "C" fn()) -> &mut Self {
        self[0x80]
            .set_handler(handler)
            .selector(KERNEL_CS_SEL)
            .present(true)
            .user_callable()
            .gate_type(GateType::InterruptGate);
        self
    }
}
