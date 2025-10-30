//! # Minimal Bitmap-based Physical Memory Manager (PMM)
//!
//! This module provides a minimal, no-heap physical memory manager for 4K frames,
//! using a bitmap to track free/used frames in a fixed region. It is suitable for
//! early kernel use or as a foundation for a more advanced PMM.
//!
//! ## Features
//! - Tracks allocation and freeing of 4K frames using a bitmap.
//! - No heap required; all state is stored inline.
//! - Can be extended to initialize from a memory map.
//!
//! ## Usage Example
//! ```rust
//! use kernel_alloc::frame_alloc::BitmapFrameAlloc;
//! use kernel_vmem::FrameAlloc;
//! let mut pmm = BitmapFrameAlloc::new();
//! let frame = pmm.alloc_4k();
//! if let Some(pa) = frame {
//!     // Use the physical address...
//!     pmm.free_4k(pa);
//! }
//! ```
//!
//! ## Safety
//! - Only physical addresses within the managed region are tracked.
//! - The user must ensure that reserved/used frames (e.g., kernel, bootloader) are marked as used before allocation.
//! - No synchronization is provided; not thread-safe.

use kernel_vmem::FrameAlloc;
use kernel_vmem::addresses::{PhysicalAddress, PhysicalPage, Size4K};

const PHYS_MEM_START: u64 = 0x0010_0000; // 1 MiB, example
const PHYS_MEM_SIZE: u64 = 64 * 1024 * 1024; // 64 MiB, example
const FRAME_SIZE: u64 = 4096;
const NUM_FRAMES: usize = (PHYS_MEM_SIZE / FRAME_SIZE) as usize;

/// Minimal bitmap-based PMM for 4K frames in a fixed region.
///
/// This type manages a fixed region of physical memory, tracking free/used 4K frames
/// using a bitmap. It supports allocation and freeing, but does not require a heap.
///
/// # Example
/// ```rust
/// use kernel_alloc::frame_alloc::BitmapFrameAlloc;
/// use kernel_vmem::FrameAlloc;
/// let mut pmm = BitmapFrameAlloc::new();
/// let frame = pmm.alloc_4k();
/// if let Some(pa) = frame {
///     // Use the physical address...
///     pmm.free_4k(pa);
/// }
/// ```
///
/// # Safety
/// - Only physical addresses within the managed region are tracked.
/// - The user must ensure that reserved/used frames (e.g., kernel, bootloader) are marked as used before allocation.
/// - No synchronization is provided; not thread-safe.
pub struct BitmapFrameAlloc {
    bitmap: [u64; NUM_FRAMES.div_ceil(64)],
    base: u64,
}

impl Default for BitmapFrameAlloc {
    fn default() -> Self {
        Self::new()
    }
}

impl BitmapFrameAlloc {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bitmap: [0; NUM_FRAMES.div_ceil(64)],
            base: PHYS_MEM_START,
        }
    }

    /// Mark a frame as used (allocated).
    pub const fn mark_used(&mut self, frame_idx: usize) {
        let (word, bit) = (frame_idx / 64, frame_idx % 64);
        self.bitmap[word] |= 1 << bit;
    }

    /// Mark a frame as free.
    pub const fn mark_free(&mut self, frame_idx: usize) {
        let (word, bit) = (frame_idx / 64, frame_idx % 64);
        self.bitmap[word] &= !(1 << bit);
    }

    /// Returns true if the frame is allocated.
    #[must_use]
    pub const fn is_used(&self, frame_idx: usize) -> bool {
        let (word, bit) = (frame_idx / 64, frame_idx % 64);
        (self.bitmap[word] & (1 << bit)) != 0
    }
}

impl FrameAlloc for BitmapFrameAlloc {
    fn alloc_4k(&mut self) -> Option<PhysicalPage<Size4K>> {
        for (i, word) in self.bitmap.iter_mut().enumerate() {
            if *word != u64::MAX {
                for bit in 0..64 {
                    let idx = i * 64 + bit;
                    if idx >= NUM_FRAMES {
                        break;
                    }
                    if (*word & (1 << bit)) == 0 {
                        *word |= 1 << bit;
                        let pa = self.base + (idx as u64) * FRAME_SIZE;
                        return Some(PhysicalPage::from_addr(PhysicalAddress::new(pa)));
                    }
                }
            }
        }
        None
    }

    fn free_4k(&mut self, pa: PhysicalPage<Size4K>) {
        let idx = ((pa.base().as_u64() - self.base) / FRAME_SIZE) as usize;
        self.mark_free(idx);
    }
}
