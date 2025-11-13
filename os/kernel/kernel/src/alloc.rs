//! # Kernel Memory Management
//!
//! This module provides the core memory management infrastructure for the kernel,
//! including physical frame allocation, virtual memory management, and page table
//! manipulation. It serves as the central interface between the kernel and the
//! underlying memory management subsystems.
//!
//! ## Architecture
//!
//! The memory management system is built on three key components:
//!
//! * **Physical Frame Allocator**: [`BitmapFrameAlloc`] manages 4KiB physical frames
//!   using a bitmap-based approach for tracking free/used pages
//! * **Physical Mapper**: [`HhdmPhysMapper`] provides Higher Half Direct Mapping (HHDM)
//!   for efficient access to physical memory from kernel virtual addresses
//! * **Virtual Memory Manager**: [`Vmm`] handles page table manipulation, mapping/unmapping
//!   operations, and address space management
//!
//! ## Key Types
//!
//! * [`KernelVmm`] - Type alias for the kernel's Virtual Memory Manager configured
//!   with HHDM mapper and bitmap allocator
//! * [`KernelVm`] - Container holding the mapper and allocator with thread-safe access
//! * [`FlushTlb`] - Policy enum controlling when TLB flushes occur during operations
//!
//! ## Initialization
//!
//! Memory management is initialized in two phases:
//!
//! 1. **Physical Allocator Setup**: [`init_physical_memory_allocator_once`] creates
//!    the bitmap allocator in a dedicated BSS section (`.bss.pmm`)
//! 2. **VMM Initialization**: [`init_kernel_vmm`] combines the allocator and mapper
//!    into a globally accessible kernel VMM instance
//!
//! ## Usage Patterns
//!
//! The module provides two primary access patterns:
//!
//! * [`with_kernel_vmm`] - Execute operations with automatic VMM lifecycle management
//! * [`try_with_kernel_vmm`] - Execute fallible operations with configurable TLB flushing
//!
//! ## Safety
//!
//! This module contains extensive unsafe code for:
//! - Direct physical memory access via HHDM
//! - Page table manipulation and TLB management
//! - Static initialization of allocator structures
//! - Raw pointer operations for memory mapping
//!
//! All unsafe operations are carefully isolated behind safe abstractions and
//! documented for their safety requirements.
//!
//! ## Debugging
//!
//! The [`debug`] submodule provides utilities for inspecting page table state,
//! walking virtual address translations, and debugging memory management issues.

pub mod debug;

use core::mem::MaybeUninit;
use kernel_alloc::frame_alloc::BitmapFrameAlloc;
use kernel_alloc::phys_mapper::HhdmPhysMapper;
use kernel_alloc::vmm::Vmm;
use kernel_sync::{RawSpin, SpinMutex, SyncOnceCell};
use kernel_vmem::{PhysFrameAlloc, PhysMapper};

pub type KernelVmm<'alloc> = Vmm<'alloc, HhdmPhysMapper, BitmapFrameAlloc>;

pub struct KernelVm<M: PhysMapper, A: PhysFrameAlloc + 'static> {
    pub mapper: M,
    pub alloc: SpinMutex<&'static mut A>,
}

#[unsafe(link_section = ".bss.pmm")]
static mut PMM: MaybeUninit<BitmapFrameAlloc> = MaybeUninit::uninit();

#[doc(alias = "init_pmm_once")]
#[allow(static_mut_refs)]
pub unsafe fn init_physical_memory_allocator_once() -> &'static mut BitmapFrameAlloc {
    // Construct in place; allowed because we're in early single-core init.
    unsafe {
        PMM.write(BitmapFrameAlloc::new());
        &mut *PMM.as_mut_ptr()
    }
}

static KVM: SyncOnceCell<KernelVm<HhdmPhysMapper, BitmapFrameAlloc>> = SyncOnceCell::new();

/// Call once in very early boot.
pub unsafe fn init_kernel_vmm(mapper: HhdmPhysMapper, alloc: &'static mut BitmapFrameAlloc) {
    let _ = KVM.get_or_init(|| KernelVm {
        mapper,
        alloc: SpinMutex::from_raw(RawSpin::new(), alloc),
    });
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum FlushTlb {
    Never,
    OnSuccess,
    Always,
}

#[inline]
pub fn with_kernel_vmm(f: impl FnOnce(&mut KernelVmm)) {
    let kvm = KVM.get().expect("Kernel VM not initialized");
    let mut alloc = kvm.alloc.lock();

    // Safety: CR3 points to a valid PML4; mapper is valid for kernel lifetime.
    let mut vmm = unsafe { Vmm::from_current(&kvm.mapper, *alloc) };
    f(&mut vmm);
}

#[inline]
pub fn try_with_kernel_vmm<R, E>(
    flush: FlushTlb,
    f: impl FnOnce(&mut KernelVmm) -> Result<R, E>,
) -> Result<R, E> {
    let kvm = KVM.get().expect("Kernel VM not initialized");
    let mut alloc = kvm.alloc.lock();

    // Safety: CR3 points to a valid PML4; mapper is valid for kernel lifetime.
    let mut vmm = unsafe { Vmm::from_current(&kvm.mapper, *alloc) };
    match f(&mut vmm) {
        Ok(r) => {
            if matches!(flush, FlushTlb::Always | FlushTlb::OnSuccess) {
                unsafe {
                    vmm.local_tlb_flush_all();
                }
            }
            Ok(r)
        }
        Err(e) => {
            if matches!(flush, FlushTlb::Always) {
                unsafe {
                    vmm.local_tlb_flush_all();
                }
            }
            Err(e)
        }
    }
}
