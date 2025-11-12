pub mod ist_stacks;
pub mod kernel_stacks;
pub mod stack;

use crate::gdt::{Gdt, Selectors};
use crate::msr::gs_base_ptr;
use crate::tss::Tss64;
use kernel_vmem::addresses::VirtualAddress;

#[repr(C, align(64))] // avoid false sharing; nice for future SMP
pub struct PerCpu {
    /// Logical CPU index (0..n-1). Often equals BSP/AP numbering.
    pub cpu_id: u32,

    /// APIC/LAPIC id if you add AP startup later.
    pub apic_id: u32,

    /// Pointer to the current running task (kernel struct). Optional for now.
    pub current_task: core::sync::atomic::AtomicPtr<Task>,

    /// 64-bit TSS required in long mode (rsp0, `ISTx`, iopb).
    pub tss: Tss64,

    /// Kernel stack top used on CPL3→CPL0 transitions (loaded from TSS.rsp0).
    pub kstack_top: VirtualAddress,

    /// Interrupt Stack Table entries (alternate hard stacks, e.g., NMI/#DF).
    ///
    /// **Slot 0:** [`IST1`](Ist::Ist1)
    pub ist_stacks: [VirtualAddress; 7],

    /// GDT storage
    pub gdt: Gdt,

    /// GDT selector values (CS/DS/…/TSS).
    pub selectors: Selectors,

    /// Quick per-CPU scratch / small allocator caches if you add them later.
    pub scratch: PerCpuScratch,

    /// Accounting / stats you might grow.
    pub ticks: core::sync::atomic::AtomicU64,
}

pub struct Task;
pub struct PerCpuScratch;

impl PerCpu {
    pub const fn new() -> Self {
        Self {
            cpu_id: 0,
            apic_id: 0,
            current_task: core::sync::atomic::AtomicPtr::new(core::ptr::null_mut()),
            tss: Tss64::new(),
            kstack_top: VirtualAddress::zero(),
            ist_stacks: [VirtualAddress::zero(); 7],
            gdt: Gdt::new(),
            selectors: Selectors::new(),
            scratch: PerCpuScratch,
            ticks: core::sync::atomic::AtomicU64::new(0),
        }
    }

    pub const fn tss_ptr(&self) -> *const Tss64 {
        core::ptr::from_ref::<Tss64>(&self.tss)
    }

    pub const fn tss_base(&self) -> VirtualAddress {
        VirtualAddress::from_ptr(self.tss_ptr())
    }

    #[inline(always)]
    #[allow(clippy::inline_always)]
    pub fn current() -> &'static Self {
        let ptr = gs_base_ptr();
        debug_assert!(!ptr.is_null(), "Per-CPU instance pointer is unset");
        unsafe { &*ptr }
    }
}
