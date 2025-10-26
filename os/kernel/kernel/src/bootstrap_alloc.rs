//! # Bootstrap-Only Frame Allocator
//!
//! This module provides a bump allocator for 4 KiB page-table frames from a static pool.
//!
//! ## WARNING
//!
//! - **Do not use any code in this file after the main kernel heap allocator is online.**
//! - No heap allocations are performed or allowed here; all memory comes from static pools.
//! - This code is only valid immediately after paging is enabled and before the global allocator is initialized.

/*
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
        // let (start, end) = pt_pool_phys_range();
        // let next = (start + 0xfff) & !0xfff; // align up
        // Self { next, end }
        todo!()
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
            // bootstrap_alloc removed for redesign.
        }

        todo!()
    }
}

 */
