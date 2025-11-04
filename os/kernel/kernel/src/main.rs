//! # The main Kernel

#![cfg_attr(not(test), no_std)]
#![no_main]
#![allow(unsafe_code)]

mod framebuffer;
mod gdt;
mod idt;
mod init;
mod interrupts;
mod panik;
mod privilege;
mod syscall;
mod task;
mod tracing;
mod tss;
mod userland;

use crate::framebuffer::{VGA_LIKE_OFFSET, fill_solid};
use core::hint::spin_loop;
use kernel_alloc::frame_alloc::BitmapFrameAlloc;
use kernel_alloc::phys_mapper::HhdmPhysMapper;
use kernel_alloc::vmm::Vmm;
use kernel_info::boot::{FramebufferInfo, KernelBootInfo};
use kernel_info::memory::HHDM_BASE;
use kernel_qemu::qemu_trace;
use kernel_vmem::VirtualMemoryPageBits;
use kernel_vmem::addresses::{PhysicalAddress, VirtualAddress};

/// Physical Memory mapper for the Higher-Half Direct Map (HHDM).
static MAPPER: HhdmPhysMapper = HhdmPhysMapper;

/// Main kernel loop, running with all memory (including framebuffer) properly mapped.
///
/// # Entry point
/// UEFI enters the kernel in [`_start_kernel`](init::_start_kernel), from where
/// we initialize the boot stack, set up memory management and then jump here.
///
/// # Memory Safety
/// At this point, the kernel operates with virtual addresses set up by the VMM, and
/// the framebuffer is accessible at its mapped virtual address. All further kernel
/// code should use these mapped addresses, not physical ones.
///
/// # Arguments
/// * `fb_virt` - [`FramebufferInfo`] with a valid, mapped virtual address.
///
/// # Safety
/// Assumes that [`remap_boot_memory`] has been called and all required mappings are in place.
fn kernel_main(fb_virt: &FramebufferInfo) -> ! {
    qemu_trace!("Kernel doing kernel things now ...\n");

    let mut cycle = 127u8;
    loop {
        cycle = cycle.wrapping_add(10);
        unsafe { fill_solid(fb_virt, 72, 0, cycle) };
        spin_loop();
    }
}

/// Remaps the boot framebuffer memory into the kernel's virtual address space.
///
/// UEFI provides the physical address of the framebuffer in the boot info, but does not
/// include it in the memory mapping table. This means the kernel must manually map the
/// framebuffer into its own virtual address space to access it. This function sets up the
/// necessary mapping so the framebuffer can be used by the kernel.
fn remap_boot_memory(bi: &KernelBootInfo) -> FramebufferInfo {
    // Set up PMM (bootstrap) and VMM (kernel)
    let mut pmm = BitmapFrameAlloc::new();
    let mut vmm = unsafe { Vmm::from_current(&MAPPER, &mut pmm) };

    // Map framebuffer
    let fb_pa = bi.fb.framebuffer_ptr;
    let fb_len = bi.fb.framebuffer_size;
    let va_base = HHDM_BASE + VGA_LIKE_OFFSET;
    let fb_flags = VirtualMemoryPageBits::default()
        .with_writable(true)
        .with_write_combining()
        .with_global(true)
        .with_no_execute(true);

    vmm.map_region(
        VirtualAddress::new(va_base),
        PhysicalAddress::new(fb_pa),
        fb_len,
        fb_flags,
        fb_flags,
    )
    .expect("Framebuffer mapping failed");

    // Return updated FramebufferInfo with new virtual address
    let mut fb_virt = bi.fb.clone();
    fb_virt.framebuffer_ptr = va_base + (fb_pa & 0xFFF); // preserve offset within page
    qemu_trace!("Remapped frame buffer to {va_base:#x}\n");
    fb_virt
}
