//! Simple physical frame allocator for early kernel use.
//! Replace with a real PMM later.

use kernel_vmem::{FrameAlloc, PhysAddr};

const PHYS_MEM_START: u64 = 0x100000; // 1 MiB, example
const PHYS_MEM_SIZE: u64 = 64 * 1024 * 1024; // 64 MiB, example
const FRAME_SIZE: u64 = 4096;

pub struct DummyFrameAlloc {
    next: u64,
    end: u64,
}

impl DummyFrameAlloc {
    pub const fn new() -> Self {
        Self {
            next: PHYS_MEM_START,
            end: PHYS_MEM_START + PHYS_MEM_SIZE,
        }
    }
}

impl FrameAlloc for DummyFrameAlloc {
    fn alloc_4k(&mut self) -> Option<PhysAddr> {
        if self.next + FRAME_SIZE > self.end {
            return None;
        }
        let pa = self.next;
        self.next += FRAME_SIZE;
        Some(PhysAddr::from_u64(pa))
    }
}
