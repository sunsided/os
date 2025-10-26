//! # Kernel Global Allocator
//!
//! A minimal, `no_std`-friendly global allocator intended for early boot / hobby
//! kernels. The implementation manages a single statically reserved heap using a
//! **singly linked free-list** with headers embedded in free blocks.
//!
//! ## Design outline
//! - **Storage**: a single `.bss`-backed byte array (`HEAP`) is treated as the heap.
//! - **Free-list nodes**: each free block starts with a [`ListNode`](crate::allocator::free_list::ListNode) header
//!   followed by `size` bytes of payload. The header is *part of the free block*.

use crate::allocator::static_heap::{ALLOC, ensure_init};
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

/// The kernel’s global allocator.
///
/// Installed via the `#[global_allocator]` attribute below. All `alloc`/`dealloc`
/// calls are serialized by the internal [`SpinLock`](kernel_sync::spin_lock::SpinLock).
pub(super) struct KernelAllocator;

/// The installed global allocator instance.
#[global_allocator]
pub(super) static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    /// Allocate a block of at least `layout.size()` bytes with `layout.align()`.
    ///
    /// # Safety
    /// The `GlobalAlloc` contract applies. Caller must handle a null return (OOM).
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ensure_init();
        ALLOC.with_lock(|alloc| unsafe { alloc.find_region(layout.size(), layout.align()) })
    }

    /// Deallocate a block previously returned by `alloc`/`alloc_zeroed`.
    ///
    /// # Safety
    /// The `GlobalAlloc` contract applies. `ptr` and `layout` must match a prior allocation.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }
        ensure_init();
        ALLOC.with_lock(|alloc| unsafe { alloc.deallocate(ptr, layout.size(), layout.align()) });
    }

    /// Allocate and zero a block.
    ///
    /// Note: zeroing is performed after allocation succeeds.
    ///
    /// # Safety
    /// The `GlobalAlloc` contract applies.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { self.alloc(layout) };
        if !p.is_null() {
            unsafe { ptr::write_bytes(p, 0, layout.size()) };
        }
        p
    }
}
