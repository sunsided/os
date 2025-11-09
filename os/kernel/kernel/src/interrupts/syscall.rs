#![allow(clippy::missing_safety_doc)]

use crate::gdt::KERNEL_CS_SEL;
use crate::interrupts::{GateType, Idt};

pub const SYSCALL_VECTOR: usize = 0x80; // 128

pub trait SyscallInterrupt {
    fn init_syscall_gate(&mut self, handler: extern "C" fn()) -> &mut Self;
}

impl SyscallInterrupt for Idt {
    fn init_syscall_gate(&mut self, handler: extern "C" fn()) -> &mut Self {
        self[SYSCALL_VECTOR]
            .set_handler(handler)
            .selector(KERNEL_CS_SEL)
            .present(true)
            .user_callable()
            .gate_type(GateType::InterruptGate);
        self
    }
}
