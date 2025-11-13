//! # Per-CPU Data Structures and Management
//!
//! This module provides the foundation for per-CPU data storage and management
//! in the kernel, enabling efficient CPU-specific state tracking and future
//! symmetric multiprocessing (SMP) support. It centralizes all CPU-local
//! resources including stacks, descriptors, and runtime state.
//!
//! ## Architecture Overview
//!
//! The per-CPU system is built around the [`PerCpu`] structure, which serves as
//! the central repository for all CPU-specific state. This design enables:
//!
//! * **Cache-Friendly Access**: 64-byte alignment prevents false sharing between CPUs
//! * **Fast Retrieval**: GS-base MSR points directly to current CPU's [`PerCpu`] instance
//! * **Scalable Design**: Virtual memory layout supports multiple CPUs with dedicated regions
//! * **Stack Safety**: Guard pages and separate stack types prevent overflow corruption
//!
//! ## Key Components
//!
//! ### Core Per-CPU Structure ([`PerCpu`])
//! * **CPU Identification**: Logical CPU ID and Local APIC ID tracking
//! * **Task Management**: Current task pointer for future scheduling support
//! * **Hardware Descriptors**: TSS (Task State Segment), GDT, and selector storage
//! * **Stack Management**: Kernel stack and IST (Interrupt Stack Table) pointers
//! * **Accounting**: Tick counter and scratch space for CPU-specific data
//!
//! ### Stack Management Subsystem
//! * [`kernel_stacks`]: Virtual memory layout for per-CPU kernel stacks
//! * [`ist_stacks`]: IST stack allocation for exception handling
//! * [`stack`]: Common stack mapping and guard page implementation
//!
//! ## Virtual Memory Layout
//!
//! The module defines separate virtual memory regions for different stack types:
//!
//! ### Kernel Stacks (`0xffff_ff00_0000_0000` base)
//! ```text
//! CPU 0: [ Guard 4K ][ Stack up to 1M ]
//! CPU 1: [ Guard 4K ][ Stack up to 1M ]  ← +1MB stride
//! CPU 2: [ Guard 4K ][ Stack up to 1M ]  ← +2MB stride
//! ```
//!
//! ### IST Stacks (`0xffff_ff10_0000_0000` base)
//! ```text
//! CPU 0:
//!   IST1: [ Guard 4K ][ Stack 128K ]
//!   IST2: [ Guard 4K ][ Stack 128K ]  ← +128K stride per IST
//!   ...
//! CPU 1: (same layout at +1MB offset)
//! ```
//!
//! ## Stack Safety Features
//!
//! * **Guard Pages**: Unmapped 4KiB pages below each stack catch overflow via page fault
//! * **ABI Alignment**: All stack tops maintain 16-byte alignment for x86-64 System V ABI
//! * **Isolation**: Separate virtual regions prevent stack type confusion
//! * **Size Limits**: Configurable maximum sizes prevent excessive memory usage
//!
//! ## Fast Access Pattern
//!
//! The [`PerCpu::current()`] function provides constant-time access to the current
//! CPU's data structure:
//!
//! ```rust
//! let cpu = PerCpu::current();
//! cpu.ticks.fetch_add(1, Ordering::Relaxed);
//! ```
//!
//! This relies on the GS-base MSR pointing to the current CPU's [`PerCpu`] instance,
//! set up during CPU initialization.
//!
//! ## Future SMP Considerations
//!
//! While currently supporting only the Bootstrap Processor (BSP), the design
//! accommodates future Application Processor (AP) support:
//!
//! * **Scalable Addressing**: Virtual memory layout supports arbitrary CPU counts
//! * **Cache Line Alignment**: 64-byte alignment prevents false sharing
//! * **Independent State**: Each CPU maintains completely separate data structures
//! * **Atomic Operations**: Tick counters and task pointers use atomic primitives
//!
//! ## Safety and Concurrency
//!
//! * **Single Owner**: Each [`PerCpu`] instance belongs to exactly one CPU
//! * **Atomic Fields**: Shared state uses appropriate atomic primitives
//! * **Memory Safety**: Guard pages and bounds checking prevent corruption
//! * **Interrupt Safety**: Access patterns work correctly during interrupt handling

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
    /// **Slot 0:** [`IST1`](super::interrupts::Ist::Ist1)
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
