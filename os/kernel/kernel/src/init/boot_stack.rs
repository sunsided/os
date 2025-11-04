use kernel_vmem::addresses::VirtualAddress;

/// Stack size.
pub const BOOT_STACK_SIZE: usize = 64 * 1024;

/// A byte buffer with a **guaranteed 16-byte alignment**.
///
/// This thin newtype is used to back stacks (or other raw buffers) that must
/// satisfy the x86-64 SysV ABI alignment rules. In particular, we want the
/// stack pointer (`RSP`) to be 16-byte aligned at call boundaries and when
/// entering assembly routines that assume vector-friendly alignment.
///
/// - The inner array has length `N`.
/// - The wrapper enforces `align(16)`, regardless of the platform’s default.
///
/// # Why 16 bytes?
/// Many instructions and calling conventions (e.g., SSE/AVX spills, ABI rules)
/// expect 16-byte alignment. Some exception/interrupt paths also benefit from a
/// predictable alignment, so we keep our boot stack aligned from the start.
///
/// # Examples
/// ```
/// # use core::mem::{align_of, size_of};
/// # struct Aligned<const N: usize>([u8; N]);
/// # #[repr(align(16))] struct A<const N: usize>([u8; N]);
/// # let _ = (size_of::<A<4096>>(), align_of::<A<4096>>());
/// // 4096 bytes with 16-byte alignment
/// // let buf = Aligned::<4096>([0; 4096]);
/// ```
#[repr(align(16))]
struct Aligned16<const N: usize>([u8; N]);

/// Early **boot stack** memory placed in a dedicated BSS section,
/// used by the boostrap CPU (BSP).
///
/// This stack is used before the normal kernel allocator and task stacks exist
/// (e.g., right after firmware/bootloader hand-off). It is:
///
/// - **Static** and zero-initialized (lives in `.bss.boot`).
/// - **16-byte aligned** via the `Aligned` wrapper.
/// - **`static mut`** because low-level setup code manipulates it via raw
///   pointers. Access must be controlled to avoid data races.
///
/// Attributes:
/// - `#[unsafe(link_section = ".bss.boot")]` ensures the symbol is emitted into
///   a dedicated section (useful for linker scripts, paging attributes, or for
///   keeping boot-only data together).
/// - `#[unsafe(no_mangle)]` gives the symbol a stable name so hand-written
///   assembly (e.g., early entry stubs) can refer to it.
///
/// # Safety
/// - Only one CPU/core should use this stack at a time. Treat it as **single-
///   owner**, early-boot scratch space.
/// - Do not use once per-CPU or task stacks are set up.
/// - Writing/reading it requires `unsafe` because it’s `static mut`.
#[unsafe(link_section = ".bss.boot")]
#[unsafe(no_mangle)]
pub static mut BOOT_STACK: Aligned16<BOOT_STACK_SIZE> = Aligned16([0; BOOT_STACK_SIZE]);

const _: () = assert!(
    BOOT_STACK_SIZE % 16 == 0,
    "BOOT_STACK_SIZE should be 16-byte aligned"
);

/// Return the **top** (high address) of the boot kernel stack, **aligned to 16 bytes**.
///
/// Stacks on x86-64 grow **downwards**. The “top” is the first address *past*
/// the buffer (i.e., `base + size`), rounded down to a 16-byte boundary so the
/// resulting value is safe to load into `RSP`.
///
/// This mirrors what the early assembly does before using the boot stack, so
/// both Rust and ASM agree on alignment.
///
/// # Returns
/// A `VirtualAddress` pointing to the aligned top-of-stack. This address is
/// suitable for initializing `RSP` (e.g., `mov rsp, <value>`).
///
/// # Notes
/// - We mask with `!0xF` to clear the lower four bits, ensuring 16-byte
///   alignment.
/// - If `BOOT_STACK_SIZE` is itself a multiple of 16 (recommended), the mask is
///   a no-op; otherwise it “rounds down” to the nearest aligned address.
/// - When entering a Rust function via `call`, SysV ABI expects `RSP % 16 == 0`
///   *before* the call instruction pushes a return address. Aligning here keeps
///   that contract easy to satisfy in early code.
///
/// # Safety
/// Reads a `static mut` symbol address (`BOOT_STACK`) via a raw const pointer.
/// This is safe as long as you only **compute** the address here and set `RSP`
/// in controlled early-boot code.
///
/// # Example
/// ```ignore
/// // Early boot (assembly or very early Rust):
/// let rsp = boot_kstack_top().as_u64();
/// unsafe { core::arch::asm!("mov rsp, {}", in(reg) rsp, options(nostack, preserves_flags)); }
/// ```
#[inline]
pub fn boot_kstack_top() -> VirtualAddress {
    let base = unsafe { &raw const BOOT_STACK as u64 };

    // ASM already aligned to 16 before using it; do the same here.
    VirtualAddress::new((base + BOOT_STACK_SIZE as u64) & !0xFu64)
}
