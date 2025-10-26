//! # Tiny Free-List Kernel Allocator
//!
//! A minimal, `no_std`-friendly global allocator intended for early boot / hobby
//! kernels. The implementation manages a single statically reserved heap using a
//! **singly linked free-list** with headers embedded in free blocks.
//!
//! ## Design outline
//! - **Storage**: a single `.bss`-backed byte array (`HEAP`) is treated as the heap.
//! - **Free-list nodes**: each free block starts with a [`ListNode`] header
//!   followed by `size` bytes of payload. The header is *part of the free block*.
//! - **Allocation strategy**: first-fit with **alignment**. Blocks are split into
//!   up to two remainders (head/tail). The chosen block’s header is removed from
//!   the free list, and the allocation returns the aligned payload pointer.
//! - **Deallocation**: the allocator expects the original `Layout` (size and
//!   alignment). It recreates a free block by placing a header immediately before
//!   the returned pointer and reinserts it into the list in address order.
//! - **Coalescing**: adjacent free blocks are merged on insert to combat
//!   fragmentation.
//! - **Synchronization**: a tiny [`SpinLock`] guards all allocator operations.
//!
//! ## Constraints & caveats
//! - Designed for **uniprocessor** or very early boot. For SMP, either keep
//!   allocations short and rare or replace [`SpinLock`] with a stronger primitive.
//! - Interrupts are **not** masked by the lock; if you allocate in interrupt
//!   context, ensure you won’t deadlock.
//! - `dealloc` must receive the same `Layout` used for `alloc` (or a layout with
//!   the same `size`), as mandated by `GlobalAlloc`.
//! - This allocator does **not** grow; its capacity is fixed by `HEAP_SIZE`.
//!
//! ## When to use
//! - Early boot, kernel bring-up, test kernels, QEMU experiments.
//! - Not intended as a production-grade, scalable SMP allocator.
//!
//! ## Related items
//! - [`KernelAllocator`] implements `GlobalAlloc` and is installed via
//!   [`GLOBAL_ALLOCATOR`].
//! - The heap is lazily initialized on the first allocation through [`ensure_init`].
//!
//! ## Safety audit points
//! - All interior `unsafe` is confined to well-documented sections.
//! - Free-list pointer manipulation is performed only while holding the lock.
//!
//! This file deliberately uses a small amount of `unsafe` to manage raw memory
//! and uphold `GlobalAlloc`’s contract in a `no_std` environment.

#![allow(unsafe_code)]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem;
use core::prelude::rust_2024::global_allocator;
use core::ptr::{self, null_mut};
use core::sync::atomic::{AtomicBool, Ordering};

/// A tiny spinlock for short critical sections.
///
/// This lock is suitable for **uniprocessor** or early boot stages where:
/// - Preemption is either disabled or non-existent.
/// - Critical sections are very short (no I/O, no blocking).
///
/// # Guarantees
/// - Provides mutual exclusion for access to the protected value.
/// - `Sync` is implemented when `T: Send`, allowing shared references across
///   threads (the lock enforces interior mutability).
///
/// # Caveats
/// - Does **not** disable interrupts.
/// - Busy-waits with `spin_loop`, so keep critical sections small.
struct SpinLock<T> {
    /// Lock state (`false` = unlocked, `true` = locked).
    locked: AtomicBool,
    /// The protected value.
    inner: UnsafeCell<T>,
}

// Safety: SpinLock provides mutual exclusion; it can be shared across threads as long as T is Send.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Create a new spinlock wrapping `inner`.
    const fn new(inner: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(inner),
        }
    }

    /// Execute `f` with exclusive access to the inner value.
    ///
    /// Spins until the lock is acquired, then releases it after `f` returns.
    ///
    /// # Panics
    /// Never panics by itself; panics in `f` will unwind through the critical section.
    fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // Spin until we acquire the lock.
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        // SAFETY: We have exclusive access while the lock is held.
        let res = {
            let inner = unsafe { &mut *self.inner.get() };
            f(inner)
        };
        self.locked.store(false, Ordering::Release);
        res
    }
}

/// Header stored at the beginning of every **free** block.
///
/// A free block in memory has the following layout:
///
/// ```text
/// +----------------------+-------------------------+
/// | ListNode (header)    |      payload (size)     |
/// +----------------------+-------------------------+
/// ^ block_addr           ^ block_addr + sizeof::<ListNode>()
/// ```
///
/// - `size` is the number of payload bytes **after** the header.
/// - `next` links to the next free block. Free blocks are kept **sorted by
///   address** to enable coalescing.
#[repr(C)]
struct ListNode {
    /// Size of the payload (bytes) following this header.
    size: usize,
    /// Pointer to the next free block (or null).
    next: *mut ListNode,
}

impl ListNode {
    /// Create a new header for a free block with the given payload `size`.
    const fn new(size: usize) -> Self {
        Self {
            size,
            next: null_mut(),
        }
    }
}

/// Align `addr` upwards to `align` (must be a power of two).
#[inline]
const fn align_up(addr: usize, align: usize) -> usize {
    (addr + (align - 1)) & !(align - 1)
}

/// A simple first-fit, split-and-coalesce free-list allocator.
///
/// The list is kept in **address order** so that neighbors can be detected and
/// coalesced on insertion.
///
/// # Invariants
/// - All free blocks are non-overlapping and lie within the heap range.
/// - Each free block is large enough to hold a `ListNode`.
/// - `head` is a sentinel; the first real block is at `head.next`.
struct FreeListAllocator {
    /// Sentinel node (does not represent memory).
    ///
    /// `head.next` points to first block
    head: ListNode,
    /// Lazily set on first call to [`init`](Self::init).
    initialized: bool,
}

impl FreeListAllocator {
    /// Construct an empty allocator (heap not yet initialized).
    const fn new() -> Self {
        Self {
            head: ListNode {
                size: 0,
                next: null_mut(),
            },
            initialized: false,
        }
    }

    /// Initialize the allocator to manage the region `[heap_start, heap_start + heap_size)`.
    ///
    /// This creates a single free block spanning the entire region (minus the
    /// header) and zeroes the payload for safety.
    ///
    /// # Safety
    /// - The memory range must be **valid**, **writable**, and **exclusive** to the allocator.
    /// - Must be called **at most once** before any allocations on this instance.
    /// - `heap_start` must be suitably aligned for storing a `ListNode`.
    const unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        // Create a single free block covering the whole heap
        let node_ptr = heap_start as *mut ListNode;
        // Available payload is heap_size - header
        let payload_start = heap_start + size_of::<ListNode>();
        let payload_size = heap_size.saturating_sub(size_of::<ListNode>());
        unsafe {
            ptr::write(node_ptr, ListNode::new(payload_size));
        }
        self.head.next = node_ptr;
        self.initialized = true;
        // Zero the payload for safety
        unsafe {
            ptr::write_bytes(payload_start as *mut u8, 0, payload_size);
        }
    }

    /// Insert a new free region starting at `addr` with total size `size` (including header).
    ///
    /// The region is inserted in address order and coalesced with adjacent free blocks.
    ///
    /// # Safety
    /// - `[addr, addr + size)` must be a valid, free memory range not currently managed as allocated.
    /// - `addr` must be aligned for `ListNode`.
    /// - `size >= size_of::<ListNode>()`.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        if size < size_of::<ListNode>() {
            return;
        }
        // Insert sorted by address to enable coalescing.
        let mut prev = &raw mut self.head;
        let mut current = unsafe { (*prev).next };
        while !current.is_null() && (current as usize) < addr {
            prev = current;
            current = unsafe { (*current).next };
        }
        let new = addr as *mut ListNode;
        unsafe {
            *new = ListNode::new(size - size_of::<ListNode>());
            (*new).next = current;
            (*prev).next = new;
        }
        // Try to coalesce around prev
        unsafe {
            self.coalesce(prev);
        }
    }

    /// Attempt to coalesce the block after `prev_ptr` with its neighbors.
    ///
    /// `prev_ptr` must be a node in the free list (possibly the sentinel head).
    ///
    /// # Safety
    /// - The list invariants must hold (sorted, non-overlapping).
    unsafe fn coalesce(&mut self, prev_ptr: *mut ListNode) {
        let curr = unsafe { (*prev_ptr).next };
        if curr.is_null() {
            return;
        }
        let next = unsafe { (*curr).next };
        if !next.is_null() {
            let curr_end = (curr as usize) + size_of::<ListNode>() + unsafe { (*curr).size };
            if curr_end == next as usize {
                unsafe {
                    (*curr).size += size_of::<ListNode>() + (*next).size;
                    (*curr).next = (*next).next;
                }
            }
        }
        // Coalesce prev and curr if adjacent (skip when prev is the sentinel head)
        if !ptr::eq(prev_ptr, &raw const self.head) {
            let prev_end =
                (prev_ptr as usize) + size_of::<ListNode>() + unsafe { (*prev_ptr).size };
            if prev_end == curr as usize {
                unsafe {
                    (*prev_ptr).size += size_of::<ListNode>() + (*curr).size;
                    (*prev_ptr).next = (*curr).next;
                }
            }
        }
    }

    /// Find and remove a free region large enough for `size` bytes with `align` alignment.
    ///
    /// Returns a pointer to the **aligned payload** (not the header) or null on failure.
    ///
    /// The chosen block is split into head/tail remainders as needed, which are
    /// reinserted into the free list (and coalesced on insert).
    ///
    /// # Safety
    /// - Must only be called while holding the allocator’s lock.
    /// - The free-list invariants must hold.
    unsafe fn find_region(&mut self, size: usize, align: usize) -> *mut u8 {
        let size = core::cmp::max(size, 1);
        let align = core::cmp::max(align, size_of::<usize>());
        let mut prev = &raw mut self.head;
        let mut current = unsafe { (*prev).next };
        while !current.is_null() {
            let region_start = current as usize + size_of::<ListNode>();
            let alloc_start = align_up(region_start, align);
            let alloc_end = alloc_start.saturating_add(size);
            let region_end =
                (current as usize) + size_of::<ListNode>() + unsafe { (*current).size };
            if alloc_end <= region_end {
                // Split current into up to two free parts around the allocation
                let head_remainder = alloc_start - region_start;
                let tail_remainder = region_end - alloc_end;

                // Remove current from free list
                unsafe {
                    (*prev).next = (*current).next;
                }

                // Add tail remainder back as a new free block
                if tail_remainder >= size_of::<ListNode>() {
                    unsafe {
                        self.add_free_region(alloc_end - size_of::<ListNode>(), tail_remainder);
                    }
                }
                // Add head remainder back as a new free block
                if head_remainder >= size_of::<ListNode>() {
                    unsafe {
                        self.add_free_region(current as usize, head_remainder);
                    }
                }

                return alloc_start as *mut u8;
            }
            prev = current;
            current = unsafe { (*current).next };
        }
        null_mut()
    }

    /// Free a previously allocated block.
    ///
    /// This recreates the header immediately before `ptr` and reinserts the
    /// region into the list (including coalescing).
    ///
    /// # Safety
    /// - `ptr` must be a pointer previously returned by this allocator’s `alloc/alloc_zeroed`.
    /// - `size` must match the `Layout::size()` used at allocation time.
    /// - Must only be called while holding the allocator’s lock.
    unsafe fn deallocate(&mut self, ptr: *mut u8, size: usize, _align: usize) {
        if ptr.is_null() || size == 0 {
            return;
        }
        // Place a ListNode header immediately before the allocation start.
        let header_addr = (ptr as usize) - size_of::<ListNode>();
        unsafe {
            self.add_free_region(header_addr, size + size_of::<ListNode>());
        }
    }
}

// Safety: The allocator is always used under SpinLock; raw pointers are only accessed while locked.
unsafe impl Send for FreeListAllocator {}

/// Total size of the statically reserved heap.
///
/// Adjust as needed. The design scales linearly; for larger heaps, consider
/// more advanced allocators to reduce fragmentation or lock contention.
const HEAP_SIZE: usize = 64 * 1024 * 1024; // 64 MiB

/// Heap storage with a minimum alignment suitable for the headers and common types.
#[repr(align(16))]
struct HeapMem([u8; HEAP_SIZE]);

/// Backing storage for the allocator’s heap, placed in a dedicated `.bss` section.
#[unsafe(link_section = ".bss.heap")]
static mut HEAP: HeapMem = HeapMem([0; HEAP_SIZE]);

/// Global allocator state protected by a spinlock.
static ALLOC: SpinLock<FreeListAllocator> = SpinLock::new(FreeListAllocator::new());

/// One-time heap initialization flag.
static DID_INIT: AtomicBool = AtomicBool::new(false);

/// Ensure the allocator is initialized (idempotent).
///
/// This computes the heap start address from the static storage and calls
/// [`FreeListAllocator::init`] exactly once.
fn ensure_init() {
    if !DID_INIT.load(Ordering::Acquire) {
        ALLOC.with_lock(|alloc| unsafe {
            if !alloc.initialized {
                let start = unsafe { (&raw const HEAP.0).cast::<u8>() as usize };
                unsafe {
                    alloc.init(start, HEAP_SIZE);
                }
                DID_INIT.store(true, Ordering::Release);
            }
        });
    }
}

/// The kernel’s global allocator.
///
/// Installed via the `#[global_allocator]` attribute below. All `alloc`/`dealloc`
/// calls are serialized by the internal [`SpinLock`].
pub struct KernelAllocator;

/// The installed global allocator instance.
#[global_allocator]
static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator;

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
