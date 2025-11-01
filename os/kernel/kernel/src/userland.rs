use kernel_alloc::vmm::Vmm;
use kernel_syscall::example_userland::USER_CODE;
use kernel_vmem::address_space::AddressSpaceMapRegionError;
use kernel_vmem::addresses::VirtualAddress;
use kernel_vmem::{FrameAlloc, PhysMapper, VirtualMemoryPageBits};

#[allow(clippy::similar_names)]
pub fn map_one_user_task<M: PhysMapper, A: FrameAlloc>(
    vmm: &mut Vmm<'_, M, A>,
    code_user_va: VirtualAddress,
    user_stack_top: VirtualAddress,
) -> Result<(VirtualAddress, VirtualAddress), AddressSpaceMapRegionError> {
    // Pick any free region in your user VA space.

    let nonleaf = VirtualMemoryPageBits::with_user_table_wb_data_only();
    let leaf_rx = VirtualMemoryPageBits::with_user_leaf_data_wb(); // US=1, P=1, NX=0
    let leaf_rw = VirtualMemoryPageBits::with_user_leaf_code_wb(); // US=1, P=1, NX=1

    let code_pa = todo!("alloc & copy USER_CODE into a phys page(s)");
    let stack_pa = todo!("alloc N pages for user stack");

    // map code (RX)
    vmm.map_region(
        code_user_va,
        code_pa,
        USER_CODE.len() as u64,
        nonleaf,
        leaf_rx,
    )?;

    // map stack (RW), growing down
    let stack_size = 8 * 4096u64;
    let stack_base = VirtualAddress::new(user_stack_top.as_u64() - stack_size);
    vmm.map_region(stack_base, stack_pa, stack_size, nonleaf, leaf_rw)?;

    Ok((code_user_va, user_stack_top))
}
