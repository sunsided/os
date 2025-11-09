//! Virtual layout for IST stacks: per-CPU × per-IST slots with a guard page.
//!
//! Layout (virtual):
//!   IST_BASE
//!     + cpu_id * CPU_STRIDE
//!       + (ist_idx-1) * IST_SLOT_STRIDE
//!         -> [ guard (4 KiB, unmapped) ][ IST stack bytes, RW|NX ]
//!
//! Notes
//! - We keep IST stacks in a separate region from kernel stacks to simplify
//!   debugging and avoid tight packing constraints.
//! - One unmapped 4 KiB guard below each IST stack catches overflows.
//! - `IST_SLOT_STRIDE` must be >= guard + max IST size you’ll map.

use crate::interrupts::Ist;
use kernel_vmem::addresses::{PageSize, Size4K, VirtualAddress, VirtualPage};

/// Number of hardware IST entries supported by x86_64 TSS.
pub const IST_SLOTS_PER_CPU: u64 = 7;

/// 4 KiB guard below each IST stack.
#[allow(dead_code)]
pub const IST_GUARD: u64 = Size4K::SIZE;

/// Virtual base for all IST stacks (choose a disjoint, canonical kernel range).
/// Picked in the higher half, far from your kernel-stack region (`0xffff_ff00_…`).
pub const IST_BASE: u64 = 0xffff_ff10_0000_0000;

/// Per-CPU stride in the IST region (bytes). Leave ample room for 7 ISTs.
pub const IST_CPU_STRIDE: u64 = 0x10_0000; // 1 MiB per CPU in the IST region

/// Per-IST stride inside one CPU’s IST area (bytes).
/// With 7 ISTs × 128 KiB < 1 MiB, this fits comfortably.
pub const IST_SLOT_STRIDE: u64 = 0x02_0000; // 128 KiB per IST “slot”

/// 16 KiB is enough for #PF/#DF handlers.
///
/// [`IST1`](Ist::Ist1) is used for critical handlers such as double fault.
pub const IST1_SIZE: u64 = 16 * 1024;

const _: () = {
    // Sanity: 7 IST slots must fit inside one CPU stride
    assert!(IST_SLOTS_PER_CPU * IST_SLOT_STRIDE <= IST_CPU_STRIDE);
    // Page-aligned strides
    assert!(IST_CPU_STRIDE % Size4K::SIZE == 0);
    assert!(IST_SLOT_STRIDE % Size4K::SIZE == 0);
};

/// Maximum usable IST bytes per slot (excludes the 4 KiB guard).
#[inline]
#[allow(dead_code)]
pub const fn max_ist_bytes() -> u64 {
    IST_SLOT_STRIDE - IST_GUARD
}

/// Return the **guard page** base for `(cpu_id, ist_idx)` (1..=7).
/// The first mapped byte (stack base) is `guard + 4 KiB`.
#[inline]
pub const fn ist_slot_for_cpu(cpu_id: u64, ist_idx: Ist) -> VirtualPage<Size4K> {
    // Hardware IST indices are 1..=7.
    assert!(ist_idx.gate_index() >= 1 && ist_idx.gate_index() <= IST_SLOTS_PER_CPU as u8);

    let cpu_off = cpu_id * IST_CPU_STRIDE;
    let ist_off = (ist_idx.gate_index() as u64 - 1) * IST_SLOT_STRIDE;
    let addr = VirtualAddress::new(IST_BASE + cpu_off + ist_off);
    let page = addr.page();
    assert!(page.base().as_u64() == addr.as_u64()); // page-aligned
    page
}

/// Convenience: compute the 16-byte–aligned top given a mapped length.
/// You’ll typically map `[guard (4 KiB)][ist_bytes]` and then use this top.
#[inline]
#[allow(dead_code)]
pub const fn ist_top_for(len_bytes_mapped: u64, guard_base: VirtualAddress) -> VirtualAddress {
    let base = VirtualAddress::new(guard_base.as_u64() + IST_GUARD); // skip guard
    // align down to 16 for ABI-correct entry on the stack
    VirtualAddress::new((base.as_u64() + len_bytes_mapped) & !0xFu64)
}
