#![allow(dead_code)]

use core::ptr::read_volatile;
use kernel_vmem::PhysMapperExt;
use kernel_vmem::addresses::{PageSize, Size4K, VirtualAddress};
use kernel_vmem::addresses::{PhysicalAddress, PhysicalPage};
use log::info;

// TODO: Review whether the default type can be used
#[repr(transparent)]
#[derive(Copy, Clone)]
struct Pte(u64);

impl Pte {
    #[inline]
    const fn p(self) -> bool {
        self.0 & (1 << 0) != 0
    }

    #[inline]
    const fn rw(self) -> bool {
        self.0 & (1 << 1) != 0
    }

    #[inline]
    const fn us(self) -> bool {
        self.0 & (1 << 2) != 0
    }

    #[inline]
    const fn ps(self) -> bool {
        self.0 & (1 << 7) != 0
    } // page size (L3/L2)

    #[inline]
    const fn nx(self) -> bool {
        self.0 & (1 << 63) != 0
    } // XD/NX (leaf-relevant)

    #[inline]
    const fn addr(self) -> u64 {
        self.0 & 0x000F_FFFF_FFFF_F000
    }
}

// Raw volatile read of the idx-th u64 entry in a 4KiB table mapped via your typed view
#[inline]
unsafe fn read_table_u64(base_ptr: *const u64, idx: usize) -> u64 {
    unsafe { read_volatile(base_ptr.add(idx)) }
}

#[inline]
const fn phys_to_page4k(pa: u64) -> PhysicalPage<Size4K> {
    let base = PhysicalAddress::new(pa & !(Size4K::SIZE - 1));
    base.page()
}

#[allow(clippy::similar_names)]
pub fn dump_walk<M: PhysMapperExt>(mapper: &M, va: VirtualAddress) {
    unsafe {
        // Indices for VA
        let va = va.as_u64();
        let l4i = ((va >> 39) & 0x1FF) as usize;
        let l3i = ((va >> 30) & 0x1FF) as usize;
        let l2i = ((va >> 21) & 0x1FF) as usize;
        let l1i = ((va >> 12) & 0x1FF) as usize;

        // CR3 → PML4 base PA
        let mut cr3: u64;
        core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
        let pml4_pa = cr3 & 0x000F_FFFF_FFFF_F000;

        // L4
        let pml4 = core::ptr::from_mut(mapper.pml4_mut(phys_to_page4k(pml4_pa))) as *const u64;
        let pml4e = Pte(read_table_u64(pml4, l4i));
        info!(
            "L4[{:3}]={:016x} P={} RW={} US={} NX={}",
            l4i,
            pml4e.0,
            pml4e.p(),
            pml4e.rw(),
            pml4e.us(),
            pml4e.nx()
        );
        if !pml4e.p() {
            info!("-- not present at L4");
            return;
        }

        // L3
        let pdpt_pa = pml4e.addr();
        let pdpt = core::ptr::from_mut(mapper.pdpt_mut(phys_to_page4k(pdpt_pa))) as *const u64;
        let pdpte = Pte(read_table_u64(pdpt, l3i));
        info!(
            "L3[{:3}]={:016x} P={} RW={} US={} PS={} NX={}",
            l3i,
            pdpte.0,
            pdpte.p(),
            pdpte.rw(),
            pdpte.us(),
            pdpte.ps(),
            pdpte.nx()
        );
        if !pdpte.p() {
            info!("-- not present at L3");
            return;
        }
        if pdpte.ps() {
            info!("-- 1GiB leaf: US={} NX={}\n", pdpte.us(), pdpte.nx());
            return;
        }

        // L2
        let pd_pa = pdpte.addr();
        let pd = core::ptr::from_mut(mapper.pd_mut(phys_to_page4k(pd_pa))) as *const u64;
        let pde = Pte(read_table_u64(pd, l2i));
        info!(
            "L2[{:3}]={:016x} P={} RW={} US={} PS={} NX={}",
            l2i,
            pde.0,
            pde.p(),
            pde.rw(),
            pde.us(),
            pde.ps(),
            pde.nx()
        );
        if !pde.p() {
            info!("-- not present at L2");
            return;
        }
        if pde.ps() {
            info!("-- 2MiB leaf: US={} NX={}\n", pde.us(), pde.nx());
            return;
        }

        // L1
        let pt_pa = pde.addr();
        let pt = core::ptr::from_mut(mapper.pt_mut(phys_to_page4k(pt_pa))) as *const u64;
        let pte = Pte(read_table_u64(pt, l1i));
        info!(
            "L1[{:3}]={:016x} P={} RW={} US={} NX={}",
            l1i,
            pte.0,
            pte.p(),
            pte.rw(),
            pte.us(),
            pte.nx()
        );
    }
}

#[inline]
const fn va_l4_index(va: VirtualAddress) -> usize {
    ((va.as_u64() >> 39) & 0x1ff) as usize
}

pub fn promote_pml4_user_bit<M: PhysMapperExt>(mapper: &M, target_va: VirtualAddress) {
    unsafe {
        let mut cr3: u64;
        core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
        let pml4_pa = cr3 & 0x000F_FFFF_FFFF_F000;
        let slot = va_l4_index(target_va);

        // Get a mutable view of the PML4 table
        let pml4 = mapper.pml4_mut(PhysicalAddress::new(pml4_pa).page());

        // Treat it as raw u64 entries; bit 2 is US
        let ents: *mut u64 = core::ptr::from_mut(pml4).cast::<u64>();
        let cur = read_volatile(ents.add(slot));
        if (cur & 1) == 0 {
            info!("promote_pml4_user_bit: slot {slot} not present");
            return;
        }
        let new = cur | (1 << 2); // US=1
        if new == cur {
            info!("PML4E[{slot}] already US=1");
        } else {
            core::ptr::write_volatile(ents.add(slot), new);
            info!("PML4E[{slot}]: {cur:016x} -> {new:016x} (US=1)");
        }
    }
}

#[inline]
const fn l4i(va: u64) -> usize {
    ((va >> 39) & 0x1ff) as usize
}
#[inline]
const fn l3i(va: u64) -> usize {
    ((va >> 30) & 0x1ff) as usize
}
#[inline]
const fn l2i(va: u64) -> usize {
    ((va >> 21) & 0x1ff) as usize
}

pub fn clear_parent_xd_for_exec<M: PhysMapperExt>(m: &M, va: VirtualAddress) {
    let va = va.as_u64();
    unsafe {
        let mut cr3: u64;
        core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
        let pml4_pa = cr3 & 0x000F_FFFF_FFFF_F000;

        let pml4 =
            core::ptr::from_mut(m.pml4_mut(PhysicalAddress::new(pml4_pa).page())).cast::<u64>();
        let pe4 = pml4.add(l4i(va));
        let e4 = core::ptr::read_volatile(pe4);
        let pdpt_pa = e4 & 0x000F_FFFF_FFFF_F000;

        let pdpt =
            core::ptr::from_mut(m.pdpt_mut(PhysicalAddress::new(pdpt_pa).page())).cast::<u64>();
        let pe3 = pdpt.add(l3i(va));
        let mut e3 = core::ptr::read_volatile(pe3);
        if e3 & (1 << 7) == 0 {
            // not a 1GiB leaf
            // clear NX at L3
            if e3 & (1u64 << 63) != 0 {
                e3 &= !(1u64 << 63);
                core::ptr::write_volatile(pe3, e3);
            }
            let pd_pa = e3 & 0x000F_FFFF_FFFF_F000;

            let pd =
                core::ptr::from_mut(m.pd_mut(PhysicalAddress::new(pd_pa).page())).cast::<u64>();
            let pe2 = pd.add(l2i(va));
            let mut e2 = core::ptr::read_volatile(pe2);
            // if 2MiB leaf, also clear NX here; else just as parent
            if e2 & (1u64 << 63) != 0 {
                e2 &= !(1u64 << 63);
                core::ptr::write_volatile(pe2, e2);
            }
        }
    }
}
