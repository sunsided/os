#![allow(dead_code)]

use kernel_alloc::vmm::Vmm;
use kernel_vmem::addresses::VirtualAddress;
use kernel_vmem::{PhysFrameAlloc, PhysMapper};

pub struct Task<'m, M: PhysMapper, A: PhysFrameAlloc> {
    pub cr3: u64,              // PML4 phys
    pub entry: VirtualAddress, // user RIP
    pub user_stack_top: VirtualAddress,
    pub kstack_top: VirtualAddress,
    pub vmm: Vmm<'m, M, A>, // handle to map/unmap in this AS
}
