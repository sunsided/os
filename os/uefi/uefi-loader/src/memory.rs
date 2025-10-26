#![allow(unsafe_code)]

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::ptr::NonNull;
use core::ptr::null_mut;
use uefi::boot;

/// A UEFI Boot Services pool allocation to back Rust's global allocator.
///
/// # Notes
/// - Valid only while Boot Services are active (before `ExitBootServices`).
/// - We always over-allocate to satisfy alignment and store the original pointer
///   just before the returned aligned block for correct deallocation.
pub struct UefiBootAllocator;

#[global_allocator]
static GLOBAL_ALLOC: UefiBootAllocator = UefiBootAllocator;

unsafe impl GlobalAlloc for UefiBootAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Ensure minimum size of 1 and include header for original pointer and padding for alignment
        let align = layout.align().max(size_of::<usize>());
        let size = layout.size().max(1);
        let Some(total) = size
            .checked_add(align)
            .and_then(|v| v.checked_add(size_of::<usize>()))
        else {
            return null_mut();
        };

        // Boot services must be active; if not, return null to signal OOM.
        // Allocate from LOADER_DATA pool; align is handled manually.
        let Ok(raw) = boot::allocate_pool(boot::MemoryType::LOADER_DATA, total) else {
            return null_mut();
        };

        let raw_ptr = raw.as_ptr();
        let addr = raw_ptr as usize + size_of::<usize>();
        let aligned = (addr + (align - 1)) & !(align - 1);
        let header_ptr = (aligned - size_of::<usize>()) as *mut usize;
        // Store the original allocation pointer just before the aligned region
        unsafe {
            ptr::write(header_ptr, raw_ptr as usize);
        }
        aligned as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        // Recover the original pool pointer from the header we stored in alloc()
        let header_ptr = (ptr as usize - size_of::<usize>()) as *mut usize;
        let orig_ptr = unsafe { ptr::read(header_ptr) as *mut u8 };
        // SAFETY: `orig_ptr` was returned by `allocate_pool` and stored by us.
        let _ = unsafe { boot::free_pool(NonNull::new_unchecked(orig_ptr)) };
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { self.alloc(layout) };
        if !p.is_null() {
            unsafe { ptr::write_bytes(p, 0, layout.size()) };
        }
        p
    }
}
