//! Virtual layout for **per-CPU kernel stacks**.
//!
//! Each CPU receives a fixed 1 MiB slot inside a dedicated high-half region.
//! Within each slot we reserve one unmapped 4 KiB **guard page** at the
//! bottom and map the actual kernel stack immediately above it.
//!
//! ```text
//!  ┌──────────────────────────────┐
//!  │   next CPU’s slot …          │
//!  ├──────────────────────────────┤ ← BASE + (cpu_id+1)*STRIDE
//!  │   mapped kernel stack        │
//!  │   (RW | NX | kernel-only)    │
//!  ├──────────────────────────────┤ ← guard + 4 KiB
//!  │   4 KiB guard (unmapped)     │
//!  └──────────────────────────────┘ ← slot base = BASE + cpu_id*STRIDE
//! ```
//!
//! * The guard page catches stack overflows with a page-fault trap.
//! * Each slot’s total span (`STRIDE`) leaves room for the guard + stack
//!   and keeps adjacent CPU stacks safely separated.

use kernel_info::memory::KERNEL_STACK_SIZE;
use kernel_memory_addresses::{PageSize, Size4K, VirtualAddress, VirtualPage};

/// Size of the unmapped guard page at the bottom of each stack (4 KiB).
pub const KSTACK_GUARD: u64 = Size4K::SIZE;

/// Virtual base address of the **kernel-stack region**.
///
/// Chosen in the canonical higher-half (kernel) address space:
/// `0xffff_ff00_0000_0000` lies below typical higher-half kernel text/data
/// mappings but far above user space.
/// This keeps all per-CPU stacks in a contiguous, predictable range that
/// doesn’t collide with identity-mapped or user regions.
pub const KSTACK_BASE: u64 = 0xffff_ff00_0000_0000;

/// Virtual span reserved per CPU (bytes).
///
/// Each CPU gets one contiguous 1 MiB slot for its guard + stack.
/// 1 MiB leaves ample space even if the kernel stack grows to hundreds of KiB.
pub const KSTACK_CPU_STRIDE: u64 = 0x10_0000; // 1 MiB per CPU

const _: () = {
    // Sanity: ensure stack size fits within the per-CPU slot.
    assert!((KERNEL_STACK_SIZE as u64).is_multiple_of(Size4K::SIZE));
    assert!((KERNEL_STACK_SIZE as u64) <= max_kstack_bytes());
    assert!(KSTACK_CPU_STRIDE.is_multiple_of(Size4K::SIZE));
};

/// Maximum usable bytes for a single kernel stack (excludes guard page).
#[inline]
pub const fn max_kstack_bytes() -> u64 {
    KSTACK_CPU_STRIDE - KSTACK_GUARD
}

/// Return the **guard-page base** of the kernel-stack slot for `cpu_id`.
///
/// The first mapped byte (stack base) lies immediately above this guard page.
/// The top of the stack can be computed as
/// `base + mapped_bytes`, aligned down to 16 bytes for ABI compliance.
#[inline]
pub const fn kstack_slot_for_cpu(cpu_id: u64) -> VirtualPage<Size4K> {
    let addr = VirtualAddress::new(KSTACK_BASE + cpu_id * KSTACK_CPU_STRIDE);
    let page = addr.page();
    assert!(page.base().as_u64() == addr.as_u64());
    page
}
