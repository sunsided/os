//! # HHDM-based [`PhysMapper`] for Kernel Virtual Memory
//!
//! This module provides a [`PhysMapper`] implementation for kernels that use a
//! higher-half direct map (HHDM). It allows safe and portable access to physical
//! memory by converting a physical address to a usable pointer in the current
//! virtual address space.
//!
//! ## Why is this needed?
//! - Rust and C code can only dereference virtual addresses, not physical ones.
//! - When manipulating page tables or other physical memory, you need a way to
//!   "see" or "touch" a physical address from your code.
//! - The mapping strategy (HHDM, identity, temporary) may differ between bootloader,
//!   kernel, and tests, so this trait abstracts over those details.
//!
//! ## How does it work?
//! - With HHDM, every physical address is mapped at `HHDM_BASE + pa`.
//! - This implementation simply adds the HHDM base to the physical address and
//!   returns a pointer.
//!
//! ## Example
//! ```rust,no_run
//! use kernel_vmem::{addresses::PhysicalAddress, page_table::pt::PageTable, PhysMapper};
//! use kernel_alloc::phys_mapper::HhdmPhysMapper;
//! let phys = PhysicalAddress::new(0x1234_0000);
//! let mapper = HhdmPhysMapper;
//! unsafe {
//!     let table: &mut PageTable = mapper.phys_to_mut(phys);
//! }
//! ```
//!
//! ## See also
//! - [`PhysMapper`] trait in `kernel-vmem`
//! - Your kernel's memory layout and HHDM configuration

use kernel_info::memory::HHDM_BASE;
use kernel_vmem::PhysMapper;
use kernel_vmem::addresses::PhysicalAddress;

/// [`PhysMapper`] implementation for kernels with a higher-half direct map (HHDM).
///
/// This type allows you to convert a physical address to a usable pointer in the
/// kernel's virtual address space by adding `HHDM_BASE` to the physical address.
///
/// # Safety
/// - The HHDM mapping must be present and cover the referenced physical range.
/// - The returned pointer must only be used for valid, mapped, and writable memory.
///
/// # Example
/// ```rust,no_run
/// use kernel_vmem::{addresses::PhysicalAddress, page_table::pt::PageTable, PhysMapper};
/// use kernel_alloc::phys_mapper::HhdmPhysMapper;
/// let phys = PhysicalAddress::new(0x1234_0000);
/// let mapper = HhdmPhysMapper;
/// unsafe {
///     let table: &mut PageTable = mapper.phys_to_mut(phys);
/// }
/// ```
pub struct HhdmPhysMapper;

impl PhysMapper for HhdmPhysMapper {
    unsafe fn phys_to_mut<T>(&self, pa: PhysicalAddress) -> &mut T {
        let va = (HHDM_BASE + pa.as_u64()) as *mut T;
        // SAFETY: Caller must ensure the physical address is valid and mapped via HHDM.
        unsafe { &mut *va }
    }
}
