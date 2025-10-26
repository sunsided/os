#![allow(unsafe_code)]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem;
use core::prelude::rust_2024::global_allocator;
use core::ptr::{self, null_mut};
use core::sync::atomic::{AtomicBool, Ordering};

// A very small spinlock for uniprocessor/early boot.
struct SpinLock<T> {
    locked: AtomicBool,
    inner: UnsafeCell<T>,
}

// Safety: SpinLock provides mutual exclusion; it can be shared across threads as long as T is Send.
unsafe impl<T: core::marker::Send> core::marker::Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    const fn new(inner: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(inner),
        }
    }

    fn with_lock<R>(&self, f: impl core::ops::FnOnce(&mut T) -> R) -> R {
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

#[repr(C)]
struct ListNode {
    size: usize,         // size of the free region (excluding this header)
    next: *mut ListNode, // next free block
}

impl ListNode {
    const fn new(size: usize) -> Self {
        Self {
            size,
            next: core::ptr::null_mut(),
        }
    }
}

#[inline]
const fn align_up(addr: usize, align: usize) -> usize {
    (addr + (align - 1)) & !(align - 1)
}

struct FreeListAllocator {
    head: ListNode, // sentinel; head.next points to first block
    initialized: bool,
}

impl FreeListAllocator {
    const fn new() -> Self {
        Self {
            head: ListNode {
                size: 0,
                next: ptr::null_mut(),
            },
            initialized: false,
        }
    }

    const unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        // Create a single free block covering the whole heap
        let node_ptr = heap_start as *mut ListNode;
        // Available payload is heap_size - header
        let payload_start = heap_start + mem::size_of::<ListNode>();
        let payload_size = heap_size.saturating_sub(mem::size_of::<ListNode>());
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

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        if size < mem::size_of::<ListNode>() {
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
            (*new) = ListNode::new(size - mem::size_of::<ListNode>());
            (*new).next = current;
            (*prev).next = new;
        }
        // Try to coalesce around prev
        unsafe {
            self.coalesce(prev);
        }
    }

    unsafe fn coalesce(&mut self, prev_ptr: *mut ListNode) {
        let curr = unsafe { (*prev_ptr).next };
        if curr.is_null() {
            return;
        }
        let next = unsafe { (*curr).next };
        if !next.is_null() {
            let curr_end = (curr as usize) + mem::size_of::<ListNode>() + unsafe { (*curr).size };
            if curr_end == next as usize {
                unsafe {
                    (*curr).size += mem::size_of::<ListNode>() + (*next).size;
                    (*curr).next = (*next).next;
                }
            }
        }
        // Coalesce prev and curr if adjacent (skip when prev is the sentinel head)
        if !core::ptr::eq(prev_ptr, &raw const self.head) {
            let prev_end =
                (prev_ptr as usize) + mem::size_of::<ListNode>() + unsafe { (*prev_ptr).size };
            if prev_end == curr as usize {
                unsafe {
                    (*prev_ptr).size += mem::size_of::<ListNode>() + (*curr).size;
                    (*prev_ptr).next = (*curr).next;
                }
            }
        }
    }

    unsafe fn find_region(&mut self, size: usize, align: usize) -> *mut u8 {
        let size = core::cmp::max(size, 1);
        let align = core::cmp::max(align, mem::size_of::<usize>());
        let mut prev = &raw mut self.head;
        let mut current = unsafe { (*prev).next };
        while !current.is_null() {
            let region_start = current as usize + mem::size_of::<ListNode>();
            let alloc_start = align_up(region_start, align);
            let alloc_end = alloc_start.saturating_add(size);
            let region_end =
                (current as usize) + mem::size_of::<ListNode>() + unsafe { (*current).size };
            if alloc_end <= region_end {
                // Split current into up to two free parts around the allocation
                let head_remainder = alloc_start - region_start;
                let tail_remainder = region_end - alloc_end;

                // Remove current from free list
                unsafe {
                    (*prev).next = (*current).next;
                }

                // Add tail remainder back as a new free block
                if tail_remainder >= mem::size_of::<ListNode>() {
                    unsafe {
                        self.add_free_region(
                            alloc_end - mem::size_of::<ListNode>(),
                            tail_remainder,
                        );
                    }
                }
                // Add head remainder back as a new free block
                if head_remainder >= mem::size_of::<ListNode>() {
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

    unsafe fn deallocate(&mut self, ptr: *mut u8, size: usize, _align: usize) {
        if ptr.is_null() || size == 0 {
            return;
        }
        // Place a ListNode header immediately before the allocation start.
        let header_addr = (ptr as usize) - mem::size_of::<ListNode>();
        unsafe {
            self.add_free_region(header_addr, size + mem::size_of::<ListNode>());
        }
    }
}

// Safety: The allocator is always used under SpinLock; raw pointers are only accessed while locked.
unsafe impl core::marker::Send for FreeListAllocator {}

// Reserve a static heap. Adjust as needed; design supports scaling to 1 GiB by changing the size.
const HEAP_SIZE: usize = 64 * 1024 * 1024; // 64 MiB

#[repr(align(16))]
struct HeapMem([u8; HEAP_SIZE]);

#[unsafe(link_section = ".bss.heap")]
static mut HEAP: HeapMem = HeapMem([0; HEAP_SIZE]);

static ALLOC: SpinLock<FreeListAllocator> = SpinLock::new(FreeListAllocator::new());
static DID_INIT: AtomicBool = AtomicBool::new(false);

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

pub struct KernelAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ensure_init();
        ALLOC.with_lock(|alloc| unsafe { alloc.find_region(layout.size(), layout.align()) })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }
        ensure_init();
        ALLOC.with_lock(|alloc| unsafe { alloc.deallocate(ptr, layout.size(), layout.align()) });
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { self.alloc(layout) };
        if !p.is_null() {
            unsafe { ptr::write_bytes(p, 0, layout.size()) };
        }
        p
    }
}
