use crate::bootmem::BootMem;
use crate::page_table::{P, PS, PageTable, RW};

pub struct Vm {
    pml4_phys: u64,
}

impl Vm {
    pub fn new(bm: &mut BootMem) -> Self {
        let pml4 = zero_page(bm);
        Self { pml4_phys: pml4 }
    }

    /// Identity-map [0, ident_size) with 2MiB pages.
    pub fn map_identity_2m(&mut self, bm: &mut BootMem, ident_size: u64, flags: u64) {
        map_2m_range(self.pml4_phys, 0, 0, ident_size, flags, bm);
    }

    /// Map a physical range to higher-half virtual base, 2MiB pages.
    pub fn map_higher_2m(
        &mut self,
        bm: &mut BootMem,
        virt_base: u64,
        phys_base: u64,
        size: u64,
        flags: u64,
    ) {
        map_2m_range(self.pml4_phys, virt_base, phys_base, size, flags, bm);
    }

    /// Switch to this address space (just load CR3).
    pub unsafe fn activate(&self) {
        core::arch::asm!("mov cr3, {}", in(reg) self.pml4_phys, options(nostack, preserves_flags));
    }
}

fn zero_page(bm: &mut BootMem) -> u64 {
    let p = bm.alloc_frame().expect("out of frames for page tables");
    unsafe {
        core::ptr::write_bytes(p as *mut u8, 0, PAGE_SIZE as usize);
    }
    p
}

fn map_2m_range(
    pml4_phys: u64,
    virt_base: u64,
    phys_base: u64,
    size: u64,
    flags: u64,
    bm: &mut BootMem,
) {
    assert!(virt_base % (2 * 1024 * 1024) == 0);
    assert!(phys_base % (2 * 1024 * 1024) == 0);

    let pages_2m = (align_up(size, 2 * 1024 * 1024) / (2 * 1024 * 1024)) as u64;

    for i in 0..pages_2m {
        let v = virt_base + i * 2 * 1024 * 1024;
        let p = phys_base + i * 2 * 1024 * 1024;
        map_single_2m(pml4_phys, v, p, flags, bm);
    }
}

fn map_single_2m(pml4_phys: u64, vaddr: u64, paddr: u64, flags: u64, bm: &mut BootMem) {
    let (l4i, l3i, l2i) = indices(vaddr);

    unsafe {
        let pml4 = (pml4_phys as *mut PageTable).as_mut().unwrap();
        // L3
        let pdpt_phys = ensure_next(pml4, l4i, bm);
        let pdpt = (pdpt_phys as *mut PageTable).as_mut().unwrap();
        // L2
        let pd_phys = ensure_next(pdpt, l3i, bm);
        let pd = (pd_phys as *mut PageTable).as_mut().unwrap();
        // 2MiB entry at L2
        let entry = paddr | P | RW | PS | (flags & !PS);
        pd.0[l2i] = entry;
    }
}

fn ensure_next(tbl: &mut PageTable, idx: usize, bm: &mut BootMem) -> u64 {
    let e = tbl.0[idx];
    if e & P != 0 {
        (e & 0x000fffff_fffff000) // existing next-level phys
    } else {
        let next = zero_page(bm);
        tbl.0[idx] = next | P | RW; // kernel-only, writable
        next
    }
}

#[inline(always)]
fn indices(v: u64) -> (usize, usize, usize) {
    let l4 = ((v >> 39) & 0x1ff) as usize;
    let l3 = ((v >> 30) & 0x1ff) as usize;
    let l2 = ((v >> 21) & 0x1ff) as usize;
    (l4, l3, l2)
}
