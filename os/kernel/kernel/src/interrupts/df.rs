use crate::gdt::KERNEL_CS_SEL;
use crate::interrupts::{GateType, Idt, Ist};
use log::error;

pub const DF_VECTOR: usize = 0x08;

pub trait DfInterrupt {
    fn init_df_gate_ist(&mut self, handler: extern "C" fn(), ist: Ist) -> &mut Self;
}

impl DfInterrupt for Idt {
    fn init_df_gate_ist(&mut self, handler: extern "C" fn(), ist: Ist) -> &mut Self {
        self[DF_VECTOR]
            .set_handler(handler)
            .selector(KERNEL_CS_SEL)
            .present(true)
            .gate_type(GateType::InterruptGate)
            .ist(ist) // MUST use a clean IST!
            .dpl(0);
        self
    }
}

#[unsafe(naked)]
pub extern "C" fn double_fault_handler() {
    core::arch::naked_asm!(
        "cli",
        "push rax",
        "mov rax, cr2",
        "push rax",                 // just log something
        "mov rdi, [rsp]",           // cr2 as arg0
        "call {rust}",
        "1: hlt; jmp 1b",
        rust = sym df_rust
    );
}

extern "C" fn df_rust(cr2: u64) {
    error!("#DF cr2={cr2:#x}");
}
