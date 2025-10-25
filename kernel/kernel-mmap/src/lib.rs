//! # Kernel Memory Mapping

#![no_std]
#![allow(unsafe_code)]

extern crate alloc;

// mod bootmem;
mod page_table;
// mod vmem;

/// Keep the first 1 GiB of physical memory identity-mapped.
const IDENT_SIZE: u64 = 1 << 30; // 1 GiB

/// The kernel image's virtual memory address.
///
/// # Linker Information
/// The Kernel's linker script must agree with this.
const KERNEL_BASE: u64 = 0xffffffff80000000;

/*
pub unsafe fn early_vm_init(
    mmap: &[MmapEntry],
    kernel_phys_start: u64,
    kernel_len: u64,
    framebuffer_phys: Option<(u64, u64)>,
) -> Vm {
    // Build simple allocator from the UEFI memory map
    let mut bm = BootMem::from_mmap(mmap);

    // Create new PML4
    let mut vm = Vm::new(&mut bm);

    // 1) identity map first 1 GiB (2MiB pages)
    vm.map_identity_2m(&mut bm, IDENT_SIZE, NX | RW | P);

    // 2) map kernel image to higher half
    let ksize = align_up(kernel_len, 2 * 1024 * 1024);
    vm.map_higher_2m(
        &mut bm,
        KERNEL_BASE,
        kernel_phys_start,
        ksize,
        RW | P, /* NX cleared for now */
    );

    // Optionally: split code vs data by section if you want proper NX now; otherwise refine later.

    // 3) map framebuffer (writeable, NX)
    if let Some((fb_phys, fb_len)) = framebuffer_phys {
        let fb_base = 0xffff8000_00000000; // pick a slot in the kernel range
        let len = align_up(fb_len, 2 * 1024 * 1024);
        vm.map_higher_2m(&mut bm, fb_base, fb_phys, len, RW | NX | P | PCD); // often UC/UC-; PCD helps avoid cache issues
        // store fb_base somewhere global if you want a virtual pointer to it
    }

    // 4) switch to our CR3
    vm.activate();

    vm
}

#[allow(clippy::inline_always)]
#[inline(always)]
fn align_down(x: u64, a: u64) -> u64 {
    debug_assert!(a.is_power_of_two());
    x & !(a - 1)
}

#[allow(clippy::inline_always)]
#[inline(always)]
fn align_up(x: u64, a: u64) -> u64 {
    debug_assert!(a.is_power_of_two());
    (x + a - 1) & !(a - 1)
}
*/
