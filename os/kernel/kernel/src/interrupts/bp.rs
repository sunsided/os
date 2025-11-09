use crate::gdt::KERNEL_CS_SEL;
use crate::interrupts::{GateType, Idt};
use core::arch::naked_asm;
use kernel_qemu::qemu_trace;

pub const BP_VECTOR: usize = 0x03;

pub trait BreakpointInterrupt {
    fn init_breakpoint_gate(&mut self, handler: extern "C" fn()) -> &mut Self;
}

impl BreakpointInterrupt for Idt {
    fn init_breakpoint_gate(&mut self, handler: extern "C" fn()) -> &mut Self {
        self[BP_VECTOR]
            .set_handler(handler)
            .selector(KERNEL_CS_SEL)
            .present(true)
            .user_callable()
            .gate_type(GateType::InterruptGate);
        self
    }
}

#[unsafe(naked)]
pub extern "C" fn bp_handler() {
    naked_asm!(
        "push rax",
        "mov rax, cr3",
        "push rax",

        // ENTRY swapgs if from CPL3: CS at [rsp + 24]
        "mov rax, [rsp + 24]",
        "test al, 3",
        "jz 1f",
        "swapgs",
        "1:",

        "mov rdi, [rsp]      ", // cr3 as arg0 (just to print something)
        "call {rust}",

         // EXIT: swapgs back if returning to CPL3 (same offset; stack unchanged)
        "mov rax, [rsp + 24]",
        "test al, 3",
        "jz 2f",
        "swapgs",
        "2:",

        "add rsp, 8",            // pop cr3
        "pop rax",
        "iretq",
        rust = sym bp_rust
    );
}

extern "C" fn bp_rust(cr3: u64) {
    qemu_trace!("Breakpoint from user, CR3={:#x}", cr3);
}
