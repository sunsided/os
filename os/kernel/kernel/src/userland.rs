use crate::alloc::KernelVmm;
use crate::elf::helpers::{pie_bias, segment_file_bytes};
use crate::elf::{ElfErr, PFlags, elf64_view};
use crate::gdt::{USER_CS, USER_DS};
use core::num::NonZeroU64;
use kernel_alloc::vmm::AllocationTarget;
use kernel_info::boot::UserBundleInfo;
use kernel_memory_addresses::{PageSize, Size4K, VirtualAddress};
use kernel_vmem::VirtualMemoryPageBits;
use log::{debug, info};
use packer_abi::unbundle::Bundle;

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

pub type UserStackTop = VirtualAddress;
pub type UserCode = VirtualAddress;

#[allow(clippy::cast_possible_truncation)]
pub fn parse_userland_bundle(
    bundle: &UserBundleInfo,
    vmm: &mut KernelVmm,
    user_stack_top: VirtualAddress,
    stack_pages_4k: NonZeroU64,
) -> Result<(UserCode, UserStackTop), ElfErr> {
    let slice: &[u8] = unsafe {
        core::slice::from_raw_parts(bundle.bytes_ptr as *const u8, bundle.length as usize)
    };

    let bundle = Bundle::parse(slice).expect("failed to parse userland bundle");
    info!("Userland bundle has {num} entries", num = bundle.len());

    let init_bytes = bundle
        .entries()
        .filter_map(Result::ok)
        .find(|(name, _bytes)| "init".eq(*name))
        .map(|(_name, bytes)| bytes)
        .expect("userland bundle has no init binary");
    info!("Init binary is {len} bytes", len = init_bytes.len());

    parse_elf_bytes(init_bytes, vmm, user_stack_top, stack_pages_4k)
}

fn parse_elf_bytes(
    bytes: &[u8],
    vmm: &mut KernelVmm,
    user_stack_top: VirtualAddress,
    stack_pages_4k: NonZeroU64,
) -> Result<(UserCode, UserStackTop), ElfErr> {
    let view = elf64_view(bytes).expect("failed to parse init binary ELF");

    // Optional bias for ET_DYN (0 for ET_EXEC with your linker script)
    let bias = pie_bias(&view).unwrap_or(0);

    // Common non-leaf flags for user traversal (US=1, WB, Present)
    let nonleaf = VirtualMemoryPageBits::user_table_wb_exec();

    // Temporary leaf while loading: RW, NX, User
    let temp_leaf_nx = VirtualMemoryPageBits::user_leaf_data_wb(); // RW,NX

    // Map and load each PT_LOAD
    debug!("Mapping user binary code ...");
    for ph in view.iter_pt_load() {
        debug!("{ph:#?}");

        if ph.p_memsz < ph.p_filesz {
            return Err(ElfErr::BadPh);
        }

        // Align the segment mapping range
        let align = core::cmp::max(ph.p_align, Size4K::SIZE);
        let seg_va = ph.p_vaddr.as_u64();
        debug!("Mapping segment to VA {seg_va} ...", seg_va = ph.p_vaddr);

        let seg_start = round_down(seg_va, align);
        let seg_end = round_up_4k(seg_va + ph.p_memsz);
        let seg_len = seg_end.checked_sub(seg_start).ok_or(ElfErr::BadPh)?;
        let map_at = VirtualAddress::new(seg_start + bias);
        let write_at = VirtualAddress::new(seg_va + bias);

        // Anonymous, zeroed user pages → temp RW,NX
        vmm.map_anon_4k_pages(
            AllocationTarget::User,
            map_at,
            0,
            seg_len,
            nonleaf,
            temp_leaf_nx,
        )
        .map_err(|_| ElfErr::MapFail)?;

        // Copy file payload to its exact virtual address (handles intra-page offsets)
        let file_bytes = segment_file_bytes(bytes, &ph)?; // length = filesz
        unsafe {
            vmm.copy_to_mapped_user(write_at, file_bytes)
                .map_err(|_| ElfErr::MapFail)?;
        }

        // ---- Final protections (W^X) ----
        let final_perm = want_perm(ph.p_flags);

        match final_perm {
            // Executable: flip ONLY the file-backed pages to RX.
            FinalPerm::Rx => {
                let file_base = write_at.as_u64();
                let file_end = file_base + ph.p_filesz;
                let rx_start = file_base & !(Size4K::SIZE - 1);
                let rx_end = (file_end + Size4K::SIZE - 1) & !(Size4K::SIZE - 1);

                // Per-page re-protect to work around helpers that don't clear NX
                let leaf_rx = VirtualMemoryPageBits::user_leaf_code_wb()
                    .with_writable(false)
                    .with_no_execute(false); // <- ensure NX=0

                let mut addr = rx_start;
                while addr < rx_end {
                    vmm.make_region_rx(VirtualAddress::new(addr), Size4K::SIZE, nonleaf, leaf_rx)
                        .map_err(|_| ElfErr::MapFail)?;
                    addr += Size4K::SIZE;
                }
            }

            // Writable, not executable: keep as RW,NX (already correct).
            FinalPerm::Rw => { /* no-op */ }

            // Read-only, not executable: flip whole segment to RO,NX.
            FinalPerm::Ro => {
                vmm.make_region_ro(
                    map_at,
                    seg_len,
                    nonleaf,
                    VirtualMemoryPageBits::user_leaf_data_wb().with_writable(false),
                )
                .map_err(|_| ElfErr::MapFail)?;
            }
        }
    }

    // Map user stack with guard page
    debug!("Mapping user binary stack ...");
    let guard = Size4K::SIZE;
    let stack_size = stack_pages_4k.get() * Size4K::SIZE;
    let stack_base = VirtualAddress::new(user_stack_top.as_u64() - guard - stack_size);

    vmm.map_anon_4k_pages(
        AllocationTarget::User,
        stack_base,
        guard,
        stack_size,
        nonleaf.with_no_execute(true),
        VirtualMemoryPageBits::user_leaf_data_wb(), // RW, NX
    )
    .map_err(|_| ElfErr::MapFail)?;

    // Entrypoint
    let entry = VirtualAddress::new(view.entry().as_u64() + bias);
    Ok((entry, user_stack_top))
}

#[inline]
const fn round_up_4k(x: u64) -> u64 {
    (x + (Size4K::SIZE - 1)) & !(Size4K::SIZE - 1)
}

#[inline]
fn round_down(x: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    x & !(align - 1)
}

/// Final desired permission "bucket" from ELF `p_flags`.
#[derive(Copy, Clone, Eq, PartialEq)]
enum FinalPerm {
    Rx, // execute, no write
    Rw, // writable, NX
    Ro, // read-only, NX
}

const fn want_perm(p_flags: PFlags) -> FinalPerm {
    let x = p_flags.execute();
    let w = p_flags.write();
    if x {
        FinalPerm::Rx
    } else if w {
        FinalPerm::Rw
    } else {
        FinalPerm::Ro
    }
}
