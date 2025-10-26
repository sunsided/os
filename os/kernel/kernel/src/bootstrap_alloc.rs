//! # Bootstrap-Only Frame Allocator
//!
//! This module provides a bump allocator for 4 KiB page-table frames from a static pool.
//!
//! ## WARNING
//!
//! - **Do not use any code in this file after the main kernel heap allocator is online.**
//! - No heap allocations are performed or allowed here; all memory comes from static pools.
//! - This code is only valid immediately after paging is enabled and before the global allocator is initialized.

use kernel_info::memory::{HHDM_BASE, KERNEL_BASE, PHYS_LOAD};
use kernel_vmem::{PhysAddr, PhysMapper};

/// Total bytes reserved in `.bss.boot` for early page-table frames.
const PT_POOL_BYTES: usize = 64 * 4096;

/// Set to `true` after the bootstrap allocator is used to disable further use.
static mut BOOTSTRAP_ALLOC_DONE: bool = false;

/// Small pool for allocating page-table frames in the kernel (after `ExitBootServices`).
#[unsafe(link_section = ".bss.boot")]
static mut PT_POOL: Align4K<PT_POOL_BYTES> = Align4K([0; PT_POOL_BYTES]);

/// A wrapper type to force **4 KiB alignment** of a byte array.
#[repr(align(4096))]
pub(crate) struct Align4K<const N: usize>(pub [u8; N]);

/// Bootstrap-only bump allocator for 4 KiB **page-table frames**.
///
/// # WARNING
/// This allocator is for use **only during kernel bootstrap**. Do not use after the main heap allocator is online.
///
/// - Never frees. Zero-fills each frame via the HHDM before handing it out.
/// - All memory comes from a statically reserved pool.
/// - Will panic if used after `BOOTSTRAP_ALLOC_DONE` is set (see below).
pub(crate) struct BootstrapFrameAlloc {
    next: u64,
    end: u64,
}

impl BootstrapFrameAlloc {
    /// Construct the allocator from the statically reserved `.bss` pool.
    pub(crate) fn new() -> Self {
        let (start, end) = pt_pool_phys_range();
        let next = (start + 0xfff) & !0xfff; // align up
        Self { next, end }
    }

    /// Mark the bootstrap allocator as finished (should be called after heap is online).
    pub(crate) unsafe fn mark_done() {
        unsafe {
            BOOTSTRAP_ALLOC_DONE = true;
        }
    }
}

impl kernel_vmem::FrameAlloc for BootstrapFrameAlloc {
    fn alloc_4k(&mut self) -> Option<PhysAddr> {
        unsafe {
            if BOOTSTRAP_ALLOC_DONE {
                panic!("BUG: BootstrapFrameAlloc used after heap allocator is online!");
            }
        }
        if self.next + 4096 > self.end {
            return None;
        }
        let pa = self.next;
        self.next += 4096;

        // Zero the frame via HHDM
        let mapper = KernelPhysMapper;
        unsafe {
            core::ptr::write_bytes(mapper.phys_to_mut::<u8>(PhysAddr::from_u64(pa)), 0, 4096);
        }
        Some(PhysAddr::from_u64(pa))
    }
}

/// A physical→virtual translator that views **physical memory through the HHDM**.
pub(crate) struct KernelPhysMapper;

impl PhysMapper for KernelPhysMapper {
    unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T {
        let va = (HHDM_BASE + pa.as_u64()) as *mut T;
        unsafe { &mut *va }
    }
}

/// Compute the **physical** address range covered by the static page-table pool.
fn pt_pool_phys_range() -> (u64, u64) {
    #[allow(unused_unsafe)]
    let va = unsafe { core::ptr::addr_of!(PT_POOL) as u64 };
    let pa_start = PHYS_LOAD + (va - KERNEL_BASE);
    let pa_end = pa_start + (PT_POOL_BYTES as u64);
    (pa_start, pa_end)
}
