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

#[allow(static_mut_refs)]
pub unsafe fn init_pmm() -> &'static mut BitmapFrameAlloc {
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
