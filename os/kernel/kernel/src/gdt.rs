//! # Global Descriptor Table (GDT) & Task State Segment (TSS) descriptor wiring for long mode
//!
//! In 64-bit mode (“long mode”), classic segmentation for code/data is largely
//! disabled, but **segment selectors still matter**:
//!
//! - They distinguish **code vs. data/stack** segments.
//! - They carry the **Descriptor Privilege Level (DPL)** used by the CPU to
//!   enforce privilege transitions (Ring-0 ↔ Ring-3).
//! - They locate the **Task State Segment (TSS)** through a **16-byte system
//!   descriptor** in the GDT so the CPU can fetch `rsp0` (kernel stack pointer)
//!   and IST stacks on privilege changes and fault handling.
//!
//! ## Why you need this in a 64-bit kernel
//! - Returning to userland with `iretq` requires **user code/data descriptors**
//!   in the GDT and using their **selectors** (with `RPL=3`) in the `iret` frame.
//! - Reliable user→kernel transitions require a loaded **TSS** (via `ltr`),
//!   because the CPU uses `tss.rsp0` as the **Ring-0 stack** on entry.
//!
//! ## GDT layout used here
//! Index | Selector | Meaning
//! ------|----------|--------
//! 0     | 0x00     | Null
//! 1     | 0x08     | Kernel code (64-bit, DPL=0; [`KERNEL_CS_SEL`])
//! 2     | 0x10     | Kernel data (DPL=0; [`KERNEL_DS_SEL`]
//! 3     | 0x18     | User   code (64-bit, DPL=3) → with RPL=3: **0x1b** ([`USER_CS_SEL`])
//! 4     | 0x20     | User   data (DPL=3)         → with RPL=3: **0x23** ([`USER_DS_SEL`])
//! 5/6   | 0x28     | TSS (16-byte system descriptor; [`TSS_SYS_SEL`])
//!
//! This module builds a typed GDT, loads it with `lgdt`, refreshes the data
//! segments, and loads the Task Register (`ltr`) with the TSS selector.
//!
//! ## SMP / per-CPU note
//! The **TSS is per-CPU**. If you run multiple CPUs, allocate a TSS (and
//! typically a GDT) per CPU and call the init routine on each CPU during bring-up.
//!
//! ## Preconditions
//! - Long mode and paging are enabled.
//! - The GDT (and TSS) memory is mapped and **writable during setup** and
//!   **readable thereafter** at the virtual addresses you pass in.
//! - Interrupts should be masked during the switch to avoid using half-set state.

pub mod descriptors;
pub mod selectors;
pub mod tss_desc;

use crate::gdt::descriptors::Desc64;
use crate::gdt::selectors::{CodeSel, DataSel, SegmentSelector, TssSel};
use crate::gdt::tss_desc::TssDesc64;
use crate::per_cpu::PerCpu;
use crate::privilege::{Dpl, Rpl};
use crate::tss::{Tss64, init_tss};
use core::mem::size_of;
use kernel_memory_addresses::VirtualAddress;

#[allow(dead_code)]
pub struct Selectors {
    pub kernel_cs: SegmentSelector<CodeSel>,
    pub kernel_ds: SegmentSelector<DataSel>,
    pub user_ds: SegmentSelector<DataSel>,
    pub user_cs: SegmentSelector<CodeSel>,
    pub tss: SegmentSelector<TssSel>,
}

impl Selectors {
    pub const fn new() -> Self {
        Self {
            kernel_cs: KERNEL_CS_SEL,
            kernel_ds: KERNEL_DS_SEL,
            user_ds: USER_DS_SEL,
            user_cs: USER_CS_SEL,
            tss: TSS_SYS_SEL,
        }
    }
}

impl Default for Selectors {
    fn default() -> Self {
        Self::new()
    }
}

// Well-known selectors matching the GDT layout in `gdt.rs`.
//
// The `*_SEL` are typed wrappers; the `*_` constants are the encoded `u16`
// values (useful for inline asm or iret frames).
pub const KERNEL_CS_SEL: SegmentSelector<CodeSel> = SegmentSelector::<CodeSel>::new(1, Rpl::Ring0);
pub const KERNEL_DS_SEL: SegmentSelector<DataSel> = SegmentSelector::<DataSel>::new(2, Rpl::Ring0);
pub const USER_DS_SEL: SegmentSelector<DataSel> = SegmentSelector::<DataSel>::new(3, Rpl::Ring3);
pub const USER_CS_SEL: SegmentSelector<CodeSel> = SegmentSelector::<CodeSel>::new(4, Rpl::Ring3);
pub const TSS_SYS_SEL: SegmentSelector<TssSel> = SegmentSelector::<TssSel>::new(5);

// Encoded selector numbers as `u16` (what the CPU actually loads).
pub const KERNEL_CS: u16 = KERNEL_CS_SEL.encode(); // 0x08
pub const KERNEL_DS: u16 = KERNEL_DS_SEL.encode(); // 0x10
pub const USER_CS: u16 = USER_CS_SEL.encode(); // 0x1b
pub const USER_DS: u16 = USER_DS_SEL.encode(); // 0x23
pub const TSS_SEL: u16 = TSS_SYS_SEL.encode(); // 0x28

// Compile-time sanity checks for selectors and descriptor sizes.
#[allow(clippy::items_after_statements)]
const _: () = {
    // Expected raw selector numbers (given the GDT layout in this file).
    assert!(KERNEL_CS == 0x08);
    assert!(KERNEL_DS == 0x10);
    assert!(USER_DS == 0x1b);
    assert!(USER_CS == 0x23);
    assert!(TSS_SEL == 0x28);

    // Encoding formula: (index << 3) | (TI=0) | RPL
    const fn enc(index: u16, rpl: u16) -> u16 {
        (index << 3) | rpl
    }

    assert!(KERNEL_CS == enc(1, 0)); // kernel code: index=1, RPL=0
    assert!(KERNEL_DS == enc(2, 0)); // kernel data: index=2, RPL=0
    assert!(USER_DS == enc(3, 3)); // user   data: index=4, RPL=3
    assert!(USER_CS == enc(4, 3)); // user   code: index=3, RPL=3
    assert!(TSS_SEL == enc(5, 0)); // TSS (low): index=5, RPL=0

    // Typed selectors must produce the same raw values.
    assert!(KERNEL_CS == KERNEL_CS_SEL.encode());
    assert!(KERNEL_DS == KERNEL_DS_SEL.encode());
    assert!(USER_CS == USER_CS_SEL.encode());
    assert!(USER_DS == USER_DS_SEL.encode());
    assert!(TSS_SEL == TSS_SYS_SEL.encode());
};

/// Virtual address used in descriptor-table pointers (with paging on).
///
/// In long mode `lgdt` expects a **linear (virtual) address** when paging is enabled.
pub type LinearAddress = VirtualAddress;

/// Pointer format required by `lgdt`.
///
/// The CPU reads exactly `limit+1` bytes starting at `base` to load the GDT.
#[repr(C, packed)]
struct DescTablePtr {
    /// Size of the table **minus one** in bytes.
    limit: u16,
    /// Base **linear (virtual) address** of the table in memory.
    base: LinearAddress,
}

/// The complete GDT for the bootstrap CPU.
///
/// Layout matches the table described in this module-level doc. The TSS occupies
/// two consecutive entries (a 16-byte system descriptor).
#[repr(C, align(16))]
pub struct Gdt {
    /// Null descriptor (must be present at index 0).
    null: Desc64, // 0
    /// Kernel code segment (64-bit, DPL=0).
    kcode: Desc64, // 1
    /// Kernel data/stack segment (DPL=0).
    /// Must be one index after `kcode` for `SYSCALL`.
    kdata: Desc64, // 2
    /// User data/stack segment (DPL=3).
    /// Must be one index before `ucode` for `SYSRET`.
    udata: Desc64, // 3
    /// User code segment (64-bit, DPL=3).
    ucode: Desc64, // 4
    /// 64-bit Available TSS descriptor (low+high).
    tss: TssDesc64, // 5 & 6 (16-byte system descriptor)
}

impl Default for Gdt {
    fn default() -> Self {
        Self::new()
    }
}

impl Gdt {
    pub const fn new_with_tss(tss: TssDesc64) -> Self {
        Self {
            null: Desc64 { raw: 0 },
            kcode: Desc64::from_code_dpl(Dpl::Ring0), // kernel code: DPL=0
            kdata: Desc64::from_data_dpl(Dpl::Ring0), // kernel data: DPL=0
            udata: Desc64::from_data_dpl(Dpl::Ring3), // user   data: DPL=3
            ucode: Desc64::from_code_dpl(Dpl::Ring3), // user   code: DPL=3
            tss,
        }
    }

    pub const fn new() -> Self {
        Self::new_with_tss(TssDesc64::new(VirtualAddress::zero(), 0))
    }
}

/// Load a GDT with `lgdt`.
///
/// # Safety
/// - `gdt` must point to a valid, fully initialized table whose memory will
///   remain **mapped and readable** for the lifetime of the CPU.
/// - Callers must ensure no interrupts or faults observe a half-installed state.
#[inline]
#[allow(clippy::cast_possible_truncation)]
unsafe fn load_gdt(gdt: &Gdt) {
    let ptr = DescTablePtr {
        limit: (size_of::<Gdt>() - 1) as u16,
        base: LinearAddress::from_ptr(&raw const *gdt),
    };

    unsafe {
        core::arch::asm!(
            "lgdt [{}]",
            in(reg) &raw const ptr,
            options(readonly, nostack, preserves_flags)
        );
    }
}

/// Load the Task Register with a TSS selector.
///
/// The selector must refer to a **present 64-bit Available TSS** system descriptor
/// in the current GDT.
///
/// # Safety
/// - The GDT must be active and contain a valid TSS descriptor at `sel`.
/// - The TSS memory must remain **resident**; the CPU reads from it on traps and
///   privilege changes.
#[inline]
unsafe fn load_task_register(sel: SegmentSelector<TssSel>) {
    let sel = sel.encode();
    unsafe {
        core::arch::asm!(
            "ltr {0:x}",
            in(reg) sel,
            options(nostack, preserves_flags)
        );
    }
}

/// Initialize and load **GDT + TSS** for the bootstrap CPU.
///
/// - Programs the TSS with `rsp0` (kernel entry stack) and optional `IST1`.
/// - Builds a GDT with kernel/user code+data descriptors and a 64-bit TSS descriptor.
/// - Executes `lgdt`, refreshes data segments (DS/ES/SS), and executes `ltr`.
///
/// Call exactly **once per CPU** (with per-CPU TSS/GDT if SMP).
///
/// ### Parameters
/// - `kernel_stack_top`: top of the Ring-0 stack (used on CPL change to 0).
/// - `ist1_top`: optional top of an **IST1** stack for critical handlers.
///
/// ### Safety / Ordering
/// - Run with interrupts disabled.
/// - Long mode + paging are already enabled.
/// - TSS storage and GDT memory must remain mapped and accessible thereafter.
///
/// ### Example
/// ```ignore
/// // During boot on BSP:
/// init_gdt_and_tss(kernel_stack_top, Some(double_fault_stack_top));
/// ```
#[allow(clippy::cast_possible_truncation)]
pub fn init_gdt_and_tss(
    p: &mut PerCpu,
    kernel_stack_top: VirtualAddress,
    ist1_top: VirtualAddress,
) {
    // Initialize TSS contents first.
    init_tss(p, kernel_stack_top, ist1_top);
    let tss_base = p.tss_base();
    let tss_limit = (size_of::<Tss64>() - 1) as u32;

    // Build GDT with typed descriptors (no raw bit twiddling here).
    p.gdt = Gdt::new_with_tss(TssDesc64::new(tss_base, tss_limit));

    // Load GDT + TR and refresh data segments.
    #[allow(static_mut_refs)]
    unsafe {
        // Load the GDTR with this CPU's GDT
        load_gdt(&p.gdt);

        // Refresh data segments to kernel data
        let kdata_sel = p.selectors.kernel_ds.encode();
        core::arch::asm!(
            "mov ds, {0:x}",
            "mov es, {0:x}",
            "mov ss, {0:x}",
            in(reg) kdata_sel,
            options(nostack, preserves_flags)
        );

        // Far reload of CS = 0x08 (kernel code). Use retfq trick in long mode.
        let kcs: u16 = p.selectors.kernel_cs.encode(); // 0x08
        core::arch::asm!(
            // push target CS and RIP, then far return
            "push {cs}",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            cs = in(reg) u64::from(kcs),
            out("rax") _,
            options(nostack)
        );

        // Load TR with the TSS selector (0x28).
        load_task_register(p.selectors.tss);
    }
}
