use crate::align_up;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RegionType {
    Conventional,
    Reserved, /* … etc … */
}

#[derive(Clone, Copy)]
pub struct MmapEntry {
    pub phys_start: u64,
    pub page_count: u64,
    pub ty: RegionType,
}

pub struct BootMem {
    frames: core::ops::Range<u64>, // [phys, phys_end) in 4KiB steps
    rest: alloc::vec::IntoIter<(u64, u64)>, // more regions as (start, end)
}

impl BootMem {
    pub fn from_mmap(mmap: &[MmapEntry]) -> Self {
        let mut regions = alloc::vec::Vec::new();
        for e in mmap {
            if e.ty == RegionType::Conventional {
                let start = align_up(e.phys_start, PAGE_SIZE);
                let end = (e.phys_start + e.page_count * PAGE_SIZE).min(u64::MAX);
                if end > start {
                    regions.push((start, end));
                }
            }
        }
        regions.sort_unstable();
        let mut it = regions.into_iter();
        let first = it.next().unwrap_or((0, 0));
        Self {
            frames: (first.0..first.1).step_by(PAGE_SIZE as usize),
            rest: it,
        }
    }

    pub fn alloc_frame(&mut self) -> Option<u64> {
        if let Some(p) = self.frames.next() {
            return Some(p);
        }
        if let Some((s, e)) = self.rest.next() {
            self.frames = (s..e).step_by(PAGE_SIZE as usize);
            return self.frames.next();
        }
        None
    }
}
