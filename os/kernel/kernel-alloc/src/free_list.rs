use core::ptr::{self, null_mut};

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
pub(super) struct ListNode {
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
pub(crate) struct FreeListAllocator {
    /// Sentinel node (does not represent memory).
    ///
    /// `head.next` points to first block
    head: ListNode,
    /// Lazily set on first call to [`init`](Self::init).
    initialized: bool,
}

// Safety: The allocator is always used under SpinLock; raw pointers are only accessed while locked.
unsafe impl Send for FreeListAllocator {}

impl FreeListAllocator {
    /// Construct an empty allocator (heap not yet initialized).
    pub(crate) const fn new() -> Self {
        Self {
            head: ListNode {
                size: 0,
                next: null_mut(),
            },
            initialized: false,
        }
    }

    /// Indicates whether the allocator has been initialized.
    pub const fn is_initialized(&self) -> bool {
        self.initialized
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
    pub(crate) const unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
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
    pub(crate) unsafe fn find_region(&mut self, size: usize, align: usize) -> *mut u8 {
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
    pub(crate) unsafe fn deallocate(&mut self, ptr: *mut u8, size: usize, _align: usize) {
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
