//! # Static Heap

use crate::free_list::FreeListAllocator;
use core::sync::atomic::{AtomicBool, Ordering};
use kernel_sync::spin_lock::SpinLock;

/// Total size of the statically reserved heap.
///
/// Adjust as needed. The design scales linearly; for larger heaps, consider
/// more advanced allocators to reduce fragmentation or lock contention.
const HEAP_SIZE: usize = 64 * 1024 * 1024; // 64 MiB

/// Backing storage for the allocator’s heap, placed in a dedicated `.bss` section.
#[unsafe(link_section = ".bss.heap")]
static mut HEAP: HeapMem = HeapMem([0; HEAP_SIZE]);

/// Global allocator state protected by a spinlock.
pub static ALLOC: SpinLock<FreeListAllocator> = SpinLock::new(FreeListAllocator::new());

/// One-time heap initialization flag.
static DID_INIT: AtomicBool = AtomicBool::new(false);

/// Heap storage with a minimum alignment suitable for the headers and common types.
#[repr(align(16))]
struct HeapMem([u8; HEAP_SIZE]);

/// Ensure the allocator is initialized (idempotent).
///
/// This computes the heap start address from the static storage and calls
/// [`FreeListAllocator::init`] exactly once.
pub(super) fn ensure_init() {
    if !DID_INIT.load(Ordering::Acquire) {
        ALLOC.with_lock(|alloc| {
            if !alloc.is_initialized() {
                let start = unsafe { (&raw const HEAP.0).cast::<u8>() as usize };
                unsafe {
                    alloc.init(start, HEAP_SIZE);
                }
                DID_INIT.store(true, Ordering::Release);
            }
        });
    }
}
