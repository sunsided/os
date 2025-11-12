use crate::gdt::KERNEL_CS_SEL;
use crate::interrupts::{GateType, Idt};

/// Spurious interrupt vector for APIC.
pub const SPURIOUS_INTERRUPT_VECTOR: u8 = 0xFF;

#[allow(clippy::absurd_extreme_comparisons)]
const _: () = assert!(SPURIOUS_INTERRUPT_VECTOR >= 0x10);

pub trait SpuriousInterrupt {
    /// Install a ring-0 page-fault handler (no user-call) and assign an IST slot.
    fn init_spurious_interrupt_gate(&mut self) -> &mut Self;
}

impl SpuriousInterrupt for Idt {
    fn init_spurious_interrupt_gate(&mut self) -> &mut Self {
        self[usize::from(SPURIOUS_INTERRUPT_VECTOR)]
            .set_handler(spurious_handler)
            .selector(KERNEL_CS_SEL)
            .present(true)
            .gate_type(GateType::InterruptGate);
        self
    }
}

const extern "C" fn spurious_handler() {
    // No EOI for spurious; just return
}
