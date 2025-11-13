use crate::alloc::KernelVmm;
use crate::gdt::{USER_CS, USER_DS};
use crate::tracing::log_ctrl_bits;
use crate::{alloc, userland_demo};
use kernel_alloc::phys_mapper::HhdmPhysMapper;
use kernel_alloc::vmm::{AllocationTarget, Vmm, VmmError};
use kernel_memory_addresses::{PageSize, Size4K, VirtualAddress};
use kernel_vmem::{PhysFrameAlloc, PhysMapper, VirtualMemoryPageBits};
use log::info;

pub unsafe fn enter_user_mode(entry: VirtualAddress, user_sp: VirtualAddress) -> ! {
    let rip = entry.as_u64();
    let cs = u64::from(USER_CS) | 3;
    let ss = u64::from(USER_DS) | 3;
    let rsp = user_sp.as_u64();
    let rflags: u64 = 0x202;

    // prove we got here
    info!("Entering user mode ...");

    unsafe {
        core::arch::asm!(
            "push {ss}",
            "push {rsp}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",
            "iretq",
            ss = in(reg) ss, rsp = in(reg) rsp, rflags = in(reg) rflags,
            cs = in(reg) cs, rip = in(reg) rip,
            options(noreturn)
        )
    }
}

pub fn boot_single_user_task(vmm: &mut KernelVmm) -> ! {
    // Choose stable user VAs
    let code_va = VirtualAddress::new(0x0000_4000_0000);
    let ustack_top = VirtualAddress::new(0x0000_7fff_f000);

    let blob = userland_demo::user_demo_bytes();

    info!("Mapping user demo ...");
    let (entry, user_sp_top) =
        map_user_demo(vmm, code_va, ustack_top, blob).expect("map user demo");

    info!("About to enter user mode ...");
    log_ctrl_bits();

    // TODO: Remove later
    alloc::debug::dump_walk(&HhdmPhysMapper, VirtualAddress::new(0x0000_0000_4000_0000));

    info!("About to flush TLB ...");
    unsafe {
        vmm.local_tlb_flush_all();
    }

    unsafe { enter_user_mode(entry, user_sp_top) } // iretq; never returns
}

#[allow(clippy::similar_names)]
fn map_user_demo<M: PhysMapper, A: PhysFrameAlloc>(
    vmm: &mut Vmm<'_, M, A>,
    code_va: VirtualAddress,
    ustack_top: VirtualAddress,
    blob: &[u8],
) -> Result<(VirtualAddress, VirtualAddress), VmmError> {
    // non-leaf: allow user traversal (U/S=1), WB, Present, Write=1 (harmless), NX don't-care
    let nonleaf = VirtualMemoryPageBits::user_table_wb_exec(); // must set US=1

    // leaf for CODE (RX): Present=1, User=1, Write=0, NX=0
    let leaf_rx = VirtualMemoryPageBits::user_leaf_code_wb(); // NX=0, US=1

    // leaf for DATA/STACK (RW, NX): Present=1, User=1, Write=1, NX=1
    let leaf_rw = VirtualMemoryPageBits::user_leaf_data_wb(); // NX=1, US=1

    // Code: map RW temporarily, copy, then flip to RX
    let code_len = blob.len() as u64;
    let code_len_4k = (code_len + Size4K::SIZE - 1) & !(Size4K::SIZE - 1);

    vmm.map_anon_4k_pages(
        AllocationTarget::User,
        code_va,
        0,           // no guard for code
        code_len_4k, // whole pages
        nonleaf,
        leaf_rw, // writable to copy in
    )?;

    unsafe {
        vmm.copy_to_mapped_user(code_va, blob)?;
    }

    // Now switch code pages to RX
    vmm.make_region_rx(code_va, code_len, nonleaf, leaf_rx)?;

    // Stack: 32 KiB RW with a 4 KiB guard below
    let stack_size = 8 * Size4K::SIZE;
    let guard = Size4K::SIZE;
    let stack_base = VirtualAddress::new(ustack_top.as_u64() - guard - stack_size);

    vmm.map_anon_4k_pages(
        AllocationTarget::User,
        stack_base,
        guard, // leave guard unmapped (fault on underflow)
        stack_size,
        nonleaf,
        leaf_rw, // writable, NX
    )?;

    Ok((code_va, ustack_top))
}
