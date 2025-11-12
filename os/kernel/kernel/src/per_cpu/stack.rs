use crate::alloc::KernelVmm;
use kernel_alloc::vmm::VmmError;
use kernel_vmem::VirtualMemoryPageBits;
use kernel_vmem::addresses::{PageSize, Size4K, VirtualAddress, VirtualPage};

/// Result of creating a kernel stack.
pub struct CpuStack {
    /// First mapped byte (above guard)
    ///
    /// This is the lowest address of the stack space, e.g. `0xffff_ff00_0000_1000`.
    pub base: VirtualAddress,
    /// 16B-aligned RSP start
    ///
    /// This is the highest address of the stack space, e.g. `0xffff_ff00_0000_9000`
    /// X86-64 stacks grow downward from `top` towards `base`, so this is where the stack begins.
    pub top: VirtualAddress,
    /// mapped length, bytes
    pub len: u64,
}

/// Map a new **kernel stack** at a predefined virtual address range,
/// leaving one guard page unmapped below it.
///
/// # Parameters
/// * `vmm` — active virtual memory manager used to allocate and map pages.
/// * `slot` — base page of the intended stack region. The first 4 KiB page
///   starting at `slot.base()` will be reserved as a **guard page**.
/// * `stack_bytes` — usable stack size in bytes (must be a multiple of 4 KiB).
///
/// # Layout
/// ```text
/// [ guard (4 KiB, unmapped) ][ stack_bytes mapped as writable, NX ]
/// ^ slot.base()              ^ base
///                            ^ top (aligned 16 B below upper end)
/// ```
///
/// # Implementation details
/// * `Size4K::SIZE` (= 4096 bytes) — size of one 4 KiB page.
///   Adding this value to `slot.base()` skips the guard page so the
///   mapped stack begins immediately above it.
/// * `& !0xFu64` — rounds the computed stack top down to a **16-byte
///   boundary** (`0xF` = 15). The x86-64 System V ABI requires the stack
///   pointer to be 16-byte aligned at function entry; clearing the lower
///   4 bits enforces that invariant.
///
/// # Returns
/// A [`CpuStack`] describing:
/// * `base` — first mapped byte above the guard page.
/// * `top` — 16-byte-aligned stack pointer where execution should start.
/// * `len` — total mapped stack size (in bytes).
///
/// # Safety & invariants
/// * The mapping must occur in a writable, kernel-only region (user bit clear).
/// * The caller must not access the guard page; it is intentionally unmapped
///   to catch stack overflows by raising a page fault.
/// * `stack_bytes` must be page-aligned; partial pages are not supported.
pub fn map_kernel_stack(
    vmm: &mut KernelVmm,
    slot: VirtualPage<Size4K>,
    stack_bytes: u64,
) -> Result<CpuStack, VmmError> {
    let nonleaf = VirtualMemoryPageBits::new()
        .with_present(true)
        .with_writable(true)
        .with_user(false);
    let leaf = VirtualMemoryPageBits::new()
        .with_present(true)
        .with_writable(true)
        .with_no_execute(true)
        .with_user(false)
        .with_global(true);

    // Leave one page as guard, map `stack_bytes` above it from fresh 4K frames.
    let guard_bytes = Size4K::SIZE;
    vmm.map_anon_4k_pages(slot.base(), guard_bytes, stack_bytes, nonleaf, leaf)?;

    let base = VirtualAddress::new(slot.base().as_u64() + Size4K::SIZE);
    let top = VirtualAddress::new((base.as_u64() + stack_bytes) & !0xFu64);
    Ok(CpuStack {
        base,
        top,
        len: stack_bytes,
    })
}

/// Allocate & map an IST stack with a 4 KiB guard below it.
/// Returns (base, top). `ist_bytes` must be 4 KiB multiple.
pub fn map_ist_stack(
    vmm: &mut KernelVmm,
    slot: VirtualPage<Size4K>,
    ist_bytes: u64,
) -> Result<(VirtualAddress, VirtualAddress), VmmError> {
    let nonleaf = VirtualMemoryPageBits::new()
        .with_present(true)
        .with_writable(true);
    let leaf = VirtualMemoryPageBits::new()
        .with_present(true)
        .with_writable(true)
        .with_no_execute(true)
        .with_user(false)
        .with_global(true);

    let guard_bytes = Size4K::SIZE;
    vmm.map_anon_4k_pages(slot.base(), guard_bytes, ist_bytes, nonleaf, leaf)?;

    let base = VirtualAddress::new(slot.base().as_u64() + Size4K::SIZE);
    let top = VirtualAddress::new((base.as_u64() + ist_bytes) & !0xFu64);
    Ok((base, top))
}
