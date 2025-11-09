#![allow(dead_code)]

use crate::apic;
use crate::gdt::KERNEL_CS_SEL;
use crate::interrupts::{GateType, Idt};
use crate::per_cpu::PerCpu;

pub const LAPIC_TIMER_VECTOR: u8 = 0xE0; // 224

pub trait TimerInterrupt {
    fn init_timer_gate(&mut self, handler: extern "C" fn()) -> &mut Self;
}

impl TimerInterrupt for Idt {
    fn init_timer_gate(&mut self, handler: extern "C" fn()) -> &mut Self {
        self[usize::from(LAPIC_TIMER_VECTOR)]
            .set_handler(handler)
            .selector(KERNEL_CS_SEL)
            .present(true)
            .kernel_only()
            .gate_type(GateType::InterruptGate);

        let e = &self[LAPIC_TIMER_VECTOR as usize];
        debug_assert_eq!(e.selector.to_u16(), 0x08);
        debug_assert_eq!((e.ist_type >> 0) & 7, 0);
        debug_assert_eq!(e.ist_type >> 8, 0x8E);

        self
    }
}

#[unsafe(naked)]
pub extern "C" fn lapic_timer_handler() {
    core::arch::naked_asm!(
        "cld",
        // Save all caller-saved + callee-saved GPRs
        "push rax","push rbx","push rcx","push rdx","push rsi","push rdi","push rbp",
        "push r8","push r9","push r10","push r11","push r12","push r13","push r14","push r15",

        // Ensure SysV stack alignment for the CALL.
        // SysV requires RSP % 16 == 8 BEFORE `call` so that inside the callee it's 16-aligned.
        // We don't know the pre-interrupt alignment, so compute and fix it.
        "mov r11, rsp",
        "and r11, 15",
        "cmp r11, 8",
        "je 2f",              // already correct parity → no adjust
        "sub rsp, 8",         // make it so
        "mov r11, 1",         // remember we adjusted
        "jmp 3f",
        "2:",
        "xor r11, r11",       // no adjust
        "3:",

        // Call the Rust handler (does EOI/masking etc.)
        "call {rust_handler}",

        // Undo temporary alignment if we subtracted 8
        "test r11, r11",
        "jz 4f",
        "add rsp, 8",
        "4:",

        // Restore GPRs and return from interrupt
        "pop r15","pop r14","pop r13","pop r12","pop r11","pop r10","pop r9","pop r8",
        "pop rbp","pop rdi","pop rsi","pop rdx","pop rcx","pop rbx","pop rax",
        "iretq",

        rust_handler = sym lapic_timer_handler_rust,
    )
}

extern "C" fn lapic_timer_handler_rust() {
    // EOI first to reduce chance of nesting storms
    unsafe {
        apic::eoi_x2apic();
    }

    let p = PerCpu::current();
    p.ticks.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
}
