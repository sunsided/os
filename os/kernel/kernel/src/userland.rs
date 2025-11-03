use crate::gdt::{USER_CS, USER_DS};
use kernel_alloc::vmm::Vmm;
use kernel_vmem::address_space::AddressSpaceMapRegionError;
use kernel_vmem::addresses::VirtualAddress;
use kernel_vmem::{FrameAlloc, PhysMapper, VirtualMemoryPageBits};

#[repr(C, packed)]
struct IretFrame {
    rip: VirtualAddress,
    cs: u64,
    rflags: u64,
    rsp: VirtualAddress,
    ss: u64,
}

pub unsafe fn enter_user_mode(entry: VirtualAddress, user_sp: VirtualAddress) -> ! {
    // Constants for the frame
    let rip = entry.as_u64();
    let cs = u64::from(USER_CS) | 3;
    let ss = u64::from(USER_DS) | 3;
    let rsp = user_sp.as_u64();
    let rflags: u64 = 0x202; // IF=1, bit 1 reserved

    unsafe {
        core::arch::asm!(
            // Push IRET frame (reverse order: SS, RSP, RFLAGS, CS, RIP)
            "push {ss}",
            "push {rsp}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",
            "iretq",
            ss = in(reg) ss,
            rsp = in(reg) rsp,
            rflags = in(reg) rflags,
            cs = in(reg) cs,
            rip = in(reg) rip,
            options(noreturn)
        )
    }
}

pub fn boot_single_user_task<M: PhysMapper, A: FrameAlloc>(mut vmm: Vmm<'_, M, A>) -> ! {
    // 1) Make sure IDT has our 0x80 gate (DPL=3) and is loaded.
    // 2) Ensure TSS.RSP0 points to a kernel stack.

    // 3) Pick user addresses
    let code_va = VirtualAddress::new(0x0000_4000_0000); // example
    let ustack_top = VirtualAddress::new(0x0000_7fff_f000); // example top

    // 4) Map the user code + stack
    let (entry, user_sp_top) =
        map_one_user_task(&mut vmm, code_va, ustack_top).expect("map user task");

    unsafe {
        // 5) Jump to Ring-3 and never return
        enter_user_mode(entry, user_sp_top);
    }
}

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

/// tiny user code (SysV x86_64); assembled by hand to keep it self-contained.
///
/// Layout:
///
/// ```asm
///   start:
///     mov edi, 7          ; arg0
///     mov esi, 35         ; arg1
///     call add_fn
///     ; rax holds 42
///     mov rax, 2          ; SYSCALL: Bogus
///     int 0x80
///     mov dil, 0x41       ; 'A'
///     mov rax, 1          ; SYSCALL: DebugWrite
///     int 0x80
///   spin: jmp spin
///   add_fn:
///     mov eax, edi
///     add eax, esi
///     ret
/// ```
#[rustfmt::skip]
pub static USER_CODE: &[u8] = &[
    0xbf, 0x07, 0x00, 0x00, 0x00,       // mov edi, 7
    0xbe, 0x23, 0x00, 0x00, 0x00,       // mov esi, 35
    0xe8, 0x0e, 0x00, 0x00, 0x00,       // call +0x0e -> add_fn
    0x48, 0xc7, 0xc0, 0x02, 0x00, 0x00, 0x00, // mov rax, 2
    0xcd, 0x80,                         // int 0x80
    0x40, 0xb7, 0x41,                   // mov dil, 'A'
    0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1
    0xcd, 0x80,                         // int 0x80
    0xeb, 0xfe,                         // spin: jmp $
    // add_fn:
    0x89, 0xf8,                         // mov eax, edi
    0x01, 0xf0,                         // add eax, esi
    0xc3,                               // ret
];
