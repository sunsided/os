use crate::alloc;
use crate::gdt::KERNEL_CS_SEL;
use crate::interrupts::{GateType, Idt, Ist};
use crate::tracing::log_ctrl_bits;
use bitfield_struct::bitfield;
use core::arch::naked_asm;
use core::hint::spin_loop;
use kernel_alloc::phys_mapper::HhdmPhysMapper;
use kernel_qemu::qemu_trace;
use kernel_vmem::addresses::VirtualAddress;

pub const PAGE_FAULT_VECTOR: usize = 0x0E; // 14

pub trait PageFaultInterrupt {
    /// Install a ring-0 page-fault handler (no user-call) and assign an IST slot.
    fn init_page_fault_gate_ist(&mut self, handler: extern "C" fn(), ist: Ist) -> &mut Self;
}

impl PageFaultInterrupt for Idt {
    fn init_page_fault_gate_ist(&mut self, handler: extern "C" fn(), ist: Ist) -> &mut Self {
        self[PAGE_FAULT_VECTOR]
            .set_handler(handler)
            .selector(KERNEL_CS_SEL)
            .present(true)
            .ist(ist)
            .gate_type(GateType::InterruptGate);
        self
    }
}

/// Interrupt-gate PF handler: reads CR2 and the pushed error code, then halts.
///
/// # Safety
/// Early-bringup only. Does not attempt to resume execution.
#[unsafe(naked)]
pub extern "C" fn page_fault_handler() {
    naked_asm!(
        "cli",
        // Save a minimal set of caller-saved regs we’ll use (SysV: rax, rcx, rdx, rsi, rdi, r8-r11).
        "push rax",
        "push rdi",
        "push rsi",

        // ENTRY swapgs if from CPL3: CS at [rsp + 40]
        "mov rax, [rsp + 40]",
        "test al, 3",
        "jz 1f",
        "swapgs",
        "1:",

        // rdi := cr2 (first arg)
        "mov rdi, cr2",
        // The CPU pushed an error code before entering the handler.
        // We just pushed 3 regs → error code is now at [rsp + 3*8].
        "mov rsi, [rsp + 24]",   // rsi := error code (second arg)
        "call {log_pf}",         // log_page_fault(cr2, err)

        // Stop here; don't try to return in early bringup.
        "1: hlt",
        "jmp 1b",
        log_pf = sym log_page_fault
    )
}

#[unsafe(no_mangle)]
extern "C" fn log_page_fault(cr2: VirtualAddress, err: PageFaultError) {
    qemu_trace!(
        "page fault page fault page fault
       ⠀⠀⠀⠀⠀⠀⠀⠙⣿⣷⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⢺⣿⣿⡆⠀⠀⠀⠀⠀⠀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⡇⠀⠀⠀⠀⠀⠀⣾⢡⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢢⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠈⣿⣿⣷⡦⠀⠀⠀⠀⢰⣿⣿⣷⠀⠀⠀⠀⠀⠀⠀⠀⠃⣠⣾⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⢻⣿⣿⣿⣆⠀⠀⠀⣾⣿⣿⣿⣷⠄⠀⠰⠤⣀⠀⠀⣴⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠃⢺⣿⣿⣿⣿⡄⠀⠀⣿⣿⢿⣿⣿⣦⣦⣦⣶⣼⣭⣼⣿⣿⣿⠇⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⢿⣿⣿⣿⣷⡆⠂⣿⣿⣞⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⢙⣿⣿⣿⣿⣷⠸⣿⣿⣿⣿⣿⣿⠟⠻⣿⣿⣿⣿⡿⣿⣿⣷⠀⠀⠀P⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠄⢿⣿⣿⣿⣿⡄⣿⣿⣿⣿⣿⣿⡀⢀⣿⣿⣿⣿⠀⢸⣿⣿⠅⠀⠀A⠀F
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠸⣿⣿⣿⣿⣇⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠁⠀ G⠀A⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠠⢐⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠀⠀⠀E⠀U⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣤⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠟⠁⠀⠀⠀⠀⠀L⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⢀⣴⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠀⠀⠀⠀⠀⠀⠀T⠀⠀⠀
        ⠀⠀⠀⠀⠀⡀⣠⣾⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡔⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⢁⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠠⢸⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⣀⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡄⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⣻⣿⣿⣿⣿⣿⡟⠋⠙⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠙⢿⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⣿⣿⣿⣿⣿⡿⠋⠀⠀⠀⢿⣿⣿⣿⣿⣿⣿⠿⢿⡿⠛⠋⠁⠀⠀⠈⠻⣿⣿⣿⣿⣿⣿⣅⠀⠀⠀⠀⠀
        ⠀⠀⠀⣿⣿⣿⣿⡟⠃⠀⠀⠀⠀⢸⣿⣿⣿⣿⣿⣿⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠙⢻⣿⣿⣿⣿⣿⣤⡀⠀⠀⠀
        ⠀⠜⢠⣿⣿⣿⣿⠀⠀⠀⠀⠀⠀⠀⢿⣿⣿⣿⣿⣿⣗⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢻⣿⣿⣿⣿⣿⣦⠄⣠⠀
        ⠠⢸⣿⣿⣿⣿⣿⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⣿⣿⣿⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠘⣿⣿⣿⣿⣿⣿⣿⣿
        ⠀⠛⣿⣿⣿⡿⠏⠀⠀⠀⠀⠀⠀⢳⣾⣿⣿⣿⣿⣿⣿⡶⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⣿⣿⣿⣿⣿⣿
        ⠀⢨⠀⠉⠉⠀⠀⠀⠀⠀⠀⠀⠀⠙⣿⣿⡿⡿⠿⠛⠙⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠹⠏⠉⠻⠿⠟⠁\n"
    );

    qemu_trace!(
        "PAGE FAULT: cr2={cr2} err={raw:#x}\n",
        raw = err.into_bits()
    );
    qemu_trace!("{}\n\n", err.explain());
    qemu_trace!("{err:#?}\n\n");

    qemu_trace!("Control bits:\n");
    log_ctrl_bits();

    qemu_trace!("\nTable walk at CR2:\n");
    alloc::debug::dump_walk(&HhdmPhysMapper, cr2);

    loop {
        spin_loop();
    }
    // TODO: Whenever returning, fix the swapgs in the asm handler above.
}

/// Page-fault error code layout (x86-64).
///
/// Each bit describes the condition that caused the page fault.
/// Reference: Intel SDM Vol. 3A, §6.15.1 “Page-Fault Exception (#PF)”.
#[bitfield(u64)]
pub struct PageFaultError {
    /// 0 = non-present page.
    /// 1 = protection violation (page present but access disallowed).
    pub present: bool, // bit 0

    /// 0 = read or execute.
    /// 1 = write access.
    pub write: bool, // bit 1

    /// 0 = supervisor (CPL 0–2).
    /// 1 = user mode (CPL 3).
    pub user: bool, // bit 2

    /// 1 = caused by reserved bit set in a paging structure.
    pub reserved_bit: bool, // bit 3

    /// 1 = instruction fetch (execute access).
    pub instruction_fetch: bool, // bit 4

    /// 1 = protection-key violation (if CR4.PKE=1).
    pub protection_key: bool, // bit 5

    /// 1 = shadow stack access (if CET-SS enabled).
    pub shadow_stack: bool, // bit 6

    #[bits(57)]
    __: u64, // reserved / ignored bits
}

impl PageFaultError {
    pub fn explain(&self) -> &'static str {
        if !self.present() {
            "Non-present page (page not mapped or swapped out)"
        } else if self.instruction_fetch() {
            if self.user() {
                "User-mode instruction fetch on protected page (likely NX or SMEP)"
            } else {
                "Kernel instruction fetch on protected page"
            }
        } else if self.write() {
            "Write access to protected page"
        } else {
            "Read access to protected page"
        }
    }
}
