//! # Kernel HHDM Framebuffer Mapping and Tiny Page-Table Allocator
//!
//! This module provides two small building blocks used right after
//! `ExitBootServices`:
//!
//! 1. A **bump allocator for 4 KiB page-table frames** sourced from a
//!    statically reserved `.bss` pool inside the kernel image.
//! 2. A helper to **map the framebuffer into the HHDM** (Higher-Half
//!    Direct Map) so early kernel code can draw pixels without going
//!    through UEFI protocols.
//!
//! ## Design notes
//!
//! - **HHDM**: We assume a higher-half direct map where virtual address
//!   `HHDM_BASE + PA` corresponds to physical address `PA`. This module
//!   uses that identity to zero newly allocated page-table frames and to
//!   derive a convenient VA for the framebuffer mapping.
//! - **Avoid splitting huge pages**: The framebuffer VA is chosen inside
//!   the HHDM but **offset by `VGA_LIKE_OFFSET`** to avoid colliding with
//!   the first 1 GiB region that is commonly mapped with a 1 GiB page.
//! - **Bootstrap-only allocator**: The page-table allocator here is a tiny
//!   bump allocator intended for early boot. It does **not** free and has
//!   a fixed pool size (`PT_POOL_BYTES`).
//!
//! ## Safety model
//!
////! - The static pool lives in `.bss.boot` and is addressed via the known
//!   kernel VA → PA relationship: `PA = PHYS_LOAD + (VA - KERNEL_BASE)`.
//! - Functions that dereference physical addresses do so through the HHDM
//!   via [`KernelPhysMapper`]. This assumes the HHDM mapping is present and
//!   covers the referenced range.
//!
//! ## When to use
//!
//! Use this module immediately after turning on paging / switching to your
//! kernel’s page tables, but before a full memory manager is online. Once a
//! real PMM/VMM exists, replace the bump allocator and (optionally) remap
//! the framebuffer wherever you prefer.

use crate::framebuffer::VGA_LIKE_OFFSET;
use kernel_info::boot::{BootPixelFormat, FramebufferInfo};
use kernel_info::memory::{HHDM_BASE, KERNEL_BASE, PHYS_LOAD};
use kernel_vmem::{
    AddressSpace, MemoryPageFlags, PageSize, PhysAddr, PhysMapper, VirtAddr, read_cr3_phys,
};

/// Total bytes reserved in `.bss.boot` for early page-table frames.
///
/// The bump allocator hands out **4 KiB** frames from this pool.
/// Increase this if you run out while mapping early regions.
const PT_POOL_BYTES: usize = 64 * 4096;

/// Small pool for allocating page-table frames in the kernel (after `ExitBootServices`).
///
/// Placed in a dedicated section so the address→offset calculation against
/// `KERNEL_BASE`/`PHYS_LOAD` works the same way during bootstrap.
#[unsafe(link_section = ".bss.boot")]
static mut PT_POOL: Align4K<{ PT_POOL_BYTES }> = Align4K([0; PT_POOL_BYTES]);

/// Compute the **physical** address range covered by the static page-table pool.
///
/// Relies on the kernel’s link-time relation:
/// `PA = PHYS_LOAD + (VA - KERNEL_BASE)`.
///
/// ### Returns
/// `(pa_start, pa_end)` as inclusive-exclusive physical bounds (in bytes).
///
/// ### Safety
/// Reads the address of a static symbol. No dereferences are performed.
///
/// ### Panics
/// Never panics.
fn pt_pool_phys_range() -> (u64, u64) {
    // Convert the VA of PT_POOL to a physical address using linker relationship: PHYS_LOAD + (va - KERNEL_BASE)
    #[allow(unused_unsafe)]
    let va = unsafe { core::ptr::addr_of!(PT_POOL) as u64 };
    let pa_start = PHYS_LOAD + (va - KERNEL_BASE);
    let pa_end = pa_start + (PT_POOL_BYTES as u64);
    (pa_start, pa_end)
}

/// Minimal bump allocator for 4 KiB **page-table frames**.
///
/// This is intended for early boot and never frees. It zero-fills each frame via
/// the HHDM before handing it out.
///
/// ### Invariants
/// - `next` and `end` are physical addresses aligned to 4 KiB (`next` is rounded up).
/// - Allocation fails cleanly by returning `None` when the pool is exhausted.
struct KernelBumpAlloc {
    next: u64,
    end: u64,
}

impl KernelBumpAlloc {
    /// Construct the allocator from the statically reserved `.bss` pool.
    ///
    /// The starting address is rounded **up** to 4 KiB.
    ///
    /// ### Panics
    /// Never panics.
    fn new() -> Self {
        let (start, end) = pt_pool_phys_range();
        let next = (start + 0xfff) & !0xfff; // align up
        Self { next, end }
    }
}

impl kernel_vmem::FrameAlloc for KernelBumpAlloc {
    /// Allocate one **4 KiB** physical frame for page tables.
    ///
    /// The returned frame is **zero-initialized** via the HHDM.
    ///
    /// ### Returns
    /// - `Some(PhysAddr)` on success
    /// - `None` if the pool is exhausted
    ///
    /// ### Safety
    /// - Assumes HHDM is present and `HHDM_BASE + pa` is writable for 4096 bytes.
    /// - Writes exactly one page of zeros into the mapped region.
    fn alloc_4k(&mut self) -> Option<PhysAddr> {
        if self.next + 4096 > self.end {
            return None;
        }
        let pa = self.next;
        self.next += 4096;
        // Zero the frame via HHDM
        let mapper = KernelPhysMapper;
        unsafe {
            core::ptr::write_bytes(mapper.phys_to_mut::<u8>(PhysAddr::from_u64(pa)), 0, 4096);
        }
        Some(PhysAddr::from_u64(pa))
    }
}

/// A physical→virtual translator that views **physical memory through the HHDM**.
///
/// Given a physical address `pa`, returns `&mut T` at virtual address
/// `HHDM_BASE + pa`. All dereferences are **unsafe** by nature.
struct KernelPhysMapper;

impl PhysMapper for KernelPhysMapper {
    /// Convert a physical address to a **mutable reference** via the HHDM.
    ///
    /// ### Safety
    /// - The HHDM must be mapped and writable for the entire `T` region.
    /// - `pa` must point to valid, uniquely-borrowed memory for `T`.
    /// - Aliasing mutable references or mapping device MMIO as `&mut T` is undefined behavior.
    unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T {
        let va = (HHDM_BASE + pa.as_u64()) as *mut T;
        unsafe { &mut *va }
    }
}

/// A wrapper type to force **4 KiB alignment** of a byte array.
///
/// Useful for static pools that should start on a page boundary.
#[repr(align(4096))]
struct Align4K<const N: usize>([u8; N]);

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
/// - `va_start` is the **virtual address** of the first framebuffer byte
///   (respecting the original physical offset within the first page),
/// - `len` is the **byte length** of the framebuffer region mapped.
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
pub unsafe fn map_framebuffer_into_hhdm(fb: &FramebufferInfo) -> (VirtAddr, u64) {
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
    let mapper = KernelPhysMapper;
    let mut alloc = KernelBumpAlloc::new();
    let aspace = AddressSpace::new(&mapper, unsafe { read_cr3_phys() });

    let mut pa = pa_start;
    let mut va = va_start & !(page - 1);
    while pa < pa_end {
        aspace
            .map_one(
                &mut alloc,
                VirtAddr::from_u64(va),
                PhysAddr::from_u64(pa),
                PageSize::Size4K,
                MemoryPageFlags::WRITABLE | MemoryPageFlags::GLOBAL | MemoryPageFlags::NX,
            )
            .expect("map framebuffer page");
        pa += page;
        va += page;
    }

    (VirtAddr::from_u64(va_start), pa_end - pa_start)
}
