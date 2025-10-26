//! # Bootstrap-Only: HHDM Framebuffer Mapping and Page-Table Allocator
//!
//! **This module is strictly for early kernel bootstrapping.**
//!
//! - Provides a bump allocator for 4 KiB page-table frames from a static pool.
//! - Provides a helper to map the framebuffer into the HHDM for early drawing.
//!
//! ## WARNING
//!
//! - **Do not use any code in this file after the main kernel heap allocator is online.**
//! - No heap allocations are performed or allowed here; all memory comes from static pools.
//! - This code is only valid immediately after paging is enabled and before the global allocator is initialized.
//!
//! ## Design notes
//!
//! - HHDM: Assumes a higher-half direct map where `HHDM_BASE + PA` == `PA`.
//! - Avoids splitting huge pages by offsetting the framebuffer mapping.
//! - The bump allocator here is for page-table frames only and never frees.
//!
//! ## When to use
//!
//! Use only during kernel bootstrap, before the real memory manager is online.
//!
//! ---

// Bootstrap allocator removed for redesign.
use crate::framebuffer::VGA_LIKE_OFFSET;
use kernel_info::boot::{BootPixelFormat, FramebufferInfo};
use kernel_info::memory::HHDM_BASE;
use kernel_vmem::VirtAddr;

/// Map the framebuffer’s **physical memory** into the HHDM and return its VA slice.
///
/// This creates a 4 KiB-granular linear mapping of the framebuffer range
/// `[framebuffer_ptr, framebuffer_ptr + framebuffer_size)` to a virtual range
/// inside the HHDM starting at `HHDM_BASE + VGA_LIKE_OFFSET + offset_in_page`.
///
/// The mapping uses page flags:
/// - `WRITABLE` (to draw)
/// - `GLOBAL`   (kept in TLB across context switches)
/// - `NX`       (no execute)
///
/// ### Parameters
/// - `fb`: Framebuffer information obtained from the loader.
///   If the pixel format is [`BootPixelFormat::BltOnly`], no mapping is created.
///
/// ### Returns
/// `(va_start, len)` where:
///
/// - `va_start` is the **virtual address** of the first framebuffer byte
///   (respecting the original physical offset within the first page),
/// - `len` is the **byte length** of the framebuffer region mapped.
///
/// If `BltOnly`, returns `(0, 0)`.
///
/// ### Safety
/// - Requires that the current CR3 (read via [`read_cr3_phys`]) points at
///   the kernel’s address space where the HHDM and the kernel’s text/data
///   are valid.
/// - The HHDM must be present at `HHDM_BASE`.
/// - The tiny page-table pool (`PT_POOL`) must be large enough; otherwise
///   this may fail (and we `expect` on failure).
///
/// ### Panics
/// - Panics on mapping failure (`expect("map framebuffer page")`).
///
/// ### Notes
/// - This function tries to **avoid splitting** a 1 GiB huge mapping in the
///   early HHDM by placing the framebuffer at `VGA_LIKE_OFFSET`.
///
/// Map the framebuffer’s **physical memory** into the HHDM and return its VA slice.
///
/// # WARNING
/// Uses only the bootstrap frame allocator. Do not call after heap is online.
pub const unsafe fn map_framebuffer_into_hhdm(fb: &FramebufferInfo) -> (VirtAddr, u64) {
    if matches!(fb.framebuffer_format, BootPixelFormat::BltOnly) {
        return (VirtAddr::from_u64(0), 0);
    }

    let fb_pa = fb.framebuffer_ptr;
    let fb_len = fb.framebuffer_size;

    let page = 4096u64;
    let pa_start = fb_pa & !(page - 1);
    let pa_end = (fb_pa + fb_len + page - 1) & !(page - 1);

    // Choose a VA inside HHDM range but outside the 1 GiB huge mapping to avoid splitting it.
    let va_base = HHDM_BASE + VGA_LIKE_OFFSET;
    let va_start = va_base + (fb_pa - pa_start);

    // Map pages
    // TODO: Implement mapping logic with new allocator/page management design.
    // let aspace = AddressSpace::new(...);

    let _pa = pa_start;
    let _va = va_start & !(page - 1);
    // TODO: Map framebuffer pages here using new allocation/mapping logic.
    (VirtAddr::from_u64(va_start), pa_end - pa_start)
}
