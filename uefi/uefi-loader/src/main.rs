//! # UEFI Loader Main Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code, dead_code)]
extern crate alloc;

mod elf_loader;
mod elf_parser;
mod file_system;
mod framebuffer;
mod memory;
mod memory_mapper;
mod rsdp;

use crate::elf_loader::{LoadedKernelWithSegs, LoadedSeg, load_pt_load_segments};
use crate::elf_parser::ElfHeader;
use crate::file_system::load_file;
use crate::framebuffer::get_framebuffer;
use crate::memory_mapper::UefiIdentityMapper;
use crate::rsdp::find_rsdp_addr;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use kernel_acpi::rsdp::AcpiRoots;
use kernel_info::{KernelBootInfo, KernelEntry, MemoryMapInfo};

use uefi::boot::{AllocateType, MemoryType};
use uefi::cstr16;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::{Status, boot};

const KERNEL_BASE: u64 = 0xffffffff80000000;

// ─────────────────────────────────────────────────────────────────────────────
// Tracing helpers (unchanged)
// ─────────────────────────────────────────────────────────────────────────────
fn trace<S>(message: S)
where
    S: AsRef<[u8]>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print(message);
    }
}

fn trace_usize<N>(number: N)
where
    N: Into<usize>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print_usize(number);
    }
}

fn trace_u64<N>(number: N)
where
    N: Into<u64>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print_u64(number);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry
// ─────────────────────────────────────────────────────────────────────────────
#[entry]
#[allow(clippy::too_many_lines)]
fn efi_main() -> Status {
    if uefi::helpers::init().is_err() {
        return Status::UNSUPPORTED;
    }

    trace("UEFI Loader reporting to QEMU\n");
    uefi::println!("Attempting to load kernel.elf ...");

    let elf_bytes = match load_file(cstr16!("\\EFI\\Boot\\kernel.elf")) {
        Ok(bytes) => bytes,
        Err(status) => {
            uefi::println!("Failed to load kernel.elf. Exiting.");
            return status;
        }
    };

    // Parse ELF64, collect PT_LOAD segments and entry address
    let Ok(parsed) = ElfHeader::parse_elf64(&elf_bytes) else {
        uefi::println!("kernel.elf is not a valid x86_64 ELF64");
        return Status::UNSUPPORTED;
    };

    // 1) Load all PT_LOAD segments at their *physical* LMAs (paddr)
    uefi::println!("Loading kernel segments into memory ...");
    let loaded: LoadedKernelWithSegs = match load_pt_load_segments(&elf_bytes, &parsed) {
        Ok(k) => k,
        Err(e) => {
            uefi::println!("Failed to load PT_LOAD segments: {e:?}");
            return Status::UNSUPPORTED;
        }
    };

    uefi::println!(
        "kernel.elf loaded successfully: entry=0x{:x}, segments={}",
        parsed.entry,
        loaded.segs.len()
    );

    // 2) Build our page tables *before* ExitBootServices:
    //    - identity map 0..1GiB
    //    - map each segment's VMA → LMA with 2MiB pages (RWX for now)
    let pml4_phys = unsafe {
        match build_kernel_mappings_per_segment(&loaded) {
            Ok(p) => p,
            Err(e) => {
                uefi::println!("Failed to build page tables: {e:?}");
                return Status::UNSUPPORTED;
            }
        }
    };

    // (Optional) Get framebuffer while GOP is available; kernel can use identity map initially.
    let fb = match get_framebuffer() {
        Ok(fb) => fb,
        Err(status) => {
            return status;
        }
    };

    let mapper = UefiIdentityMapper;

    // 3) Find RSDP while config table is available
    let rsdp_addr: u64 = find_rsdp_addr();

    #[cfg(feature = "qemu")]
    {
        if let Some(roots) = unsafe { AcpiRoots::parse(&mapper, rsdp_addr) } {
            if let Some(addr) = roots.rsdt_addr {
                trace("Found RSDT for ACPI 1.0 at ");
                trace_u64(addr);
                trace("\n");
            } else if let Some(addr) = roots.xsdt_addr {
                trace("Found XSDT for ACPI 2.0 at ");
                trace_u64(addr);
                trace("\n");
            } else {
                trace("Found unknown ACPI variant\n");
            }
        } else {
            trace("No ACPI RSDP found in UEFI configuration table\n");
        }
    }

    // 4) Prepare BootInfo (memory map is filled after ExitBootServices)
    let boot_info = KernelBootInfo {
        mmap: MemoryMapInfo {
            mmap_ptr: 0,
            mmap_len: 0,
            mmap_desc_size: 0,
            mmap_desc_version: 0,
        },
        rsdp_addr,
        fb,
    };
    let boot_info = Box::leak(Box::new(boot_info));

    uefi::println!("Booting kernel ...");
    trace("Booting kernel ...\n");

    // Pre-allocate a buffer while UEFI allocator is still alive.
    let mut mmap_copy = match allocate_mmap_buffer() {
        Ok(buf) => buf,
        Err(status) => {
            return status;
        }
    };
    let mmap_copy_ptr = mmap_copy.as_mut_ptr();

    // 5) Exit boot services; copy final memory map into our buffer
    let owned_map = unsafe { boot::exit_boot_services(None) };
    let src = owned_map.buffer().as_ptr();
    let mmap_length = owned_map.buffer().len();
    if mmap_length > mmap_copy.len() {
        trace("Memory map size assertion failed: Expected ");
        trace_usize(mmap_copy.len());
        trace(", got ");
        trace_usize(mmap_length);
        return Status::BUFFER_TOO_SMALL;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(src, mmap_copy_ptr, mmap_length);
    }
    boot_info.mmap.mmap_ptr = mmap_copy_ptr as u64;
    boot_info.mmap.mmap_len = mmap_length as u64;
    boot_info.mmap.mmap_desc_size = owned_map.meta().desc_size as u64;
    boot_info.mmap.mmap_desc_version = owned_map.meta().desc_version;
    core::mem::forget(mmap_copy);

    // 6) Now that firmware is out of the way, switch CR3 to *our* PML4
    //    Disable interrupts first so nothing vectors into dead firmware.
    trace("Switching CR3 now...\n");
    trace("   PML4 phys = ");
    trace_u64(pml4_phys);
    trace("\n");
    trace("   Entry VMA = ");
    trace_u64(parsed.entry);
    trace("\n");
    unsafe {
        core::arch::asm!("cli");
    }
    unsafe { activate_paging(pml4_phys) };
    trace("CR3 switched; probing entry byte...\n");
    unsafe {
        let _ = core::ptr::read_volatile(parsed.entry as *const u8);
    }
    trace("Probe OK; jumping to kernel...\n");

    // Quick sanity: is the entry address inside any mapped PT_LOAD?
    if !entry_is_covered(parsed.entry, &loaded.segs) {
        uefi::println!(
            "BUG: entry 0x{:x} not covered by any PT_LOAD mapping",
            parsed.entry
        );
        return Status::UNSUPPORTED;
    }

    // (Optional) Tiny probe to ensure the entry page is readable/executable.
    // We'll read a few bytes at the entry VMA to ensure the mapping is live.
    unsafe {
        let probe = parsed.entry as *const u8;
        let _b0 = core::ptr::read_volatile(probe);
        let _b1 = core::ptr::read_volatile(probe.add(1));
    }

    // 7) Jump to the higher-half entry (VMA)
    run_kernel(&parsed, boot_info)
}

/// Jump into the kernel code (higher-half VMA)
fn run_kernel(parsed: &ElfHeader, boot_info: &KernelBootInfo) -> ! {
    trace_boot_info(boot_info);
    trace("UEFI is now jumping into Kernel land. Ciao Kakao ...\n");
    let entry: KernelEntry = unsafe { core::mem::transmute(parsed.entry) };
    let bi_ptr: *const KernelBootInfo = boot_info as *const KernelBootInfo;
    entry(bi_ptr)
}

// ─────────────────────────────────────────────────────────────────────────────
// Page table builder (2 MiB pages; identity 1 GiB + per-segment VMA→LMA maps)
// ─────────────────────────────────────────────────────────────────────────────

const PAGE_SIZE: u64 = 4096;
const P: u64 = 1 << 0; // present
const RW: u64 = 1 << 1; // writable
const PS: u64 = 1 << 7; // 2MiB PDE
const NX: u64 = 1u64 << 63;

#[repr(C, align(4096))]
struct PageTable([u64; 512]);

#[inline(always)]
fn align_down(x: u64, a: u64) -> u64 {
    x & !(a - 1)
}
#[inline(always)]
fn align_up(x: u64, a: u64) -> u64 {
    (x + a - 1) & !(a - 1)
}

#[inline(always)]
fn idxs(va: u64) -> (usize, usize, usize) {
    let l4 = ((va >> 39) & 0x1ff) as usize;
    let l3 = ((va >> 30) & 0x1ff) as usize;
    let l2 = ((va >> 21) & 0x1ff) as usize;
    (l4, l3, l2)
}

unsafe fn alloc_zero_page_low(mem_type: MemoryType) -> Result<u64, Status> {
    // Keep page tables under 1 GiB so our identity map can trivially cover them.
    let max = 0x0000_0000_3FFF_F000u64; // highest 4KiB page fully below 1 GiB
    let p = boot::allocate_pages(AllocateType::MaxAddress(max), mem_type, 1)
        .map_err(|_| Status::OUT_OF_RESOURCES)?
        .as_ptr() as u64;
    core::ptr::write_bytes(p as *mut u8, 0, 4096);
    Ok(p)
}

unsafe fn ensure_next(
    parent_phys: u64,
    idx: usize,
    mem_type: MemoryType,
    tb: &mut TableBuild,
) -> Result<u64, Status> {
    let parent = &mut *(parent_phys as *mut PageTable);
    let e = parent.0[idx];
    if e & P != 0 {
        Ok(e & 0x000f_ffff_ffff_f000)
    } else {
        let child = alloc_zero_page_low(mem_type)?;
        parent.0[idx] = child | P | RW;
        tb.max_pt_phys = tb.max_pt_phys.max(child + 4096);
        Ok(child)
    }
}

unsafe fn map_2m(
    pml4_phys: u64,
    va: u64,
    pa: u64,
    flags: u64,
    mem_type: MemoryType,
    tb: &mut TableBuild,
) -> Result<(), Status> {
    let (l4, l3, l2) = idxs(va);
    let pdpt_phys = ensure_next(pml4_phys, l4, mem_type, tb)?;
    let pd_phys = ensure_next(pdpt_phys, l3, mem_type, tb)?;
    let pd = &mut *(pd_phys as *mut PageTable);

    debug_assert_eq!(va & ((1 << 21) - 1), 0);
    debug_assert_eq!(pa & ((1 << 21) - 1), 0);
    pd.0[l2] = (pa & 0x000f_ffff_ffff_f000) | P | RW | PS | (flags & !PS);
    Ok(())
}

unsafe fn map_2m_range(
    pml4_phys: u64,
    va_start: u64,
    pa_start: u64,
    size: u64,
    flags: u64,
    mem_type: MemoryType,
    tb: &mut TableBuild,
) -> Result<(), Status> {
    let size = align_up(size, 2 * 1024 * 1024);
    let mut i = 0;
    while i < size {
        let va = va_start + i;
        let pa = pa_start + i;
        map_2m(pml4_phys, va, pa, flags, mem_type, tb)?;
        i += 2 * 1024 * 1024;
    }
    Ok(())
}

struct TableBuild {
    pml4_phys: u64,
    max_pt_phys: u64,
}

unsafe fn build_kernel_mappings_per_segment(loaded: &LoadedKernelWithSegs) -> Result<u64, Status> {
    let mem_type = MemoryType::LOADER_DATA;
    let mut tb = TableBuild {
        pml4_phys: alloc_zero_page_low(mem_type)?,
        max_pt_phys: 0,
    };
    tb.max_pt_phys = tb.pml4_phys + 4096;

    // 1) We’ll identity map after we know how high our page tables went.

    // 2) Map each PT_LOAD (rounded to 2 MiB), VMA → LMA
    for seg in &loaded.segs {
        if seg.memsz == 0 {
            continue;
        }
        let va0 = align_down(seg.vaddr, 2 * 1024 * 1024);
        let pa0 = align_down(seg.paddr, 2 * 1024 * 1024);
        let len = align_up(seg.vaddr + seg.memsz, 2 * 1024 * 1024) - va0;

        map_2m_range(
            tb.pml4_phys,
            va0,
            pa0,
            len,
            RW, /* exec */
            mem_type,
            &mut tb,
        )?;
    }

    // 1) Identity map low memory enough to cover all page-table frames
    let ident_top = ident_ceiling(tb.max_pt_phys);
    let mut off = 0;
    while off < ident_top {
        map_2m(tb.pml4_phys, off, off, RW | NX, mem_type, &mut tb)?;
        off += 2 * 1024 * 1024;
    }

    Ok(tb.pml4_phys)
}

fn ident_ceiling(max_pt_phys: u64) -> u64 {
    // Also include where the loader lives (current RIP/RSP likely < 1 GiB)
    // We’ll just ensure at least 1 GiB, and extend to cover page-table top.
    let floor = 1u64 << 30; // 1 GiB
    floor.max(align_up(max_pt_phys, 2 * 1024 * 1024))
}

#[inline(always)]
unsafe fn activate_paging(pml4_phys: u64) {
    // Load our page tables (paging already on from UEFI)
    core::arch::asm!("mov cr3, {}", in(reg) pml4_phys, options(nostack, preserves_flags));
}

// ─────────────────────────────────────────────────────────────────────────────
// Memory map copy helpers (unchanged)
// ─────────────────────────────────────────────────────────────────────────────

/// Allocate a buffer to hold a copy of the memory map returned from `ExitBootServices`.
fn allocate_mmap_buffer() -> Result<Vec<u8>, Status> {
    const EXTRA_DESCS: usize = 32;

    let probe = match boot::memory_map(MemoryType::LOADER_DATA) {
        Ok(probe) => probe,
        Err(e) => {
            uefi::println!("Failed to get memory map: {e:?}");
            return Err(Status::UNSUPPORTED);
        }
    };

    let desc_size = probe.meta().desc_size;
    let mut needed_size = probe.meta().map_size;
    drop(probe);

    needed_size += EXTRA_DESCS * desc_size;

    let buf = vec![0u8; needed_size];
    Ok(buf)
}

fn trace_boot_info(boot_info: &KernelBootInfo) {
    trace("Boot Info in UEFI Loader:\n");
    trace("   BI ptr = ");
    trace_usize(core::ptr::from_ref(boot_info) as usize);
    trace("\n");
    trace(" MMAP ptr = ");
    trace_u64(boot_info.mmap.mmap_ptr);
    trace(", MMAP len = ");
    trace_u64(boot_info.mmap.mmap_len);
    trace(", MMAP desc size = ");
    trace_u64(boot_info.mmap.mmap_desc_size);
    trace(", MMAP desc version = ");
    trace_usize(usize::try_from(boot_info.mmap.mmap_desc_version).unwrap_or_default());
    trace(", rsdp addr = ");
    trace_usize(usize::try_from(boot_info.rsdp_addr).unwrap_or_default());
    trace("\n");
    trace("   FB ptr = ");
    trace_u64(boot_info.fb.framebuffer_ptr);
    trace(", FB size = ");
    trace_u64(boot_info.fb.framebuffer_size);
    trace(", FB width = ");
    trace_u64(boot_info.fb.framebuffer_width);
    trace(", FB height = ");
    trace_u64(boot_info.fb.framebuffer_height);
    trace(", FB stride = ");
    trace_u64(boot_info.fb.framebuffer_stride);
    trace(", FB format = ");
    match boot_info.fb.framebuffer_format {
        kernel_info::BootPixelFormat::Rgb => trace("RGB"),
        kernel_info::BootPixelFormat::Bgr => trace("BGR"),
        kernel_info::BootPixelFormat::Bitmask => trace("Bitmask"),
        kernel_info::BootPixelFormat::BltOnly => trace("BltOnly"),
    }
    trace("\n");
}

fn entry_is_covered(entry_vma: u64, segs: &[LoadedSeg]) -> bool {
    segs.iter().any(|s| {
        let start = s.vaddr;
        let end = s.vaddr.saturating_add(s.memsz);
        entry_vma >= start && entry_vma < end
    })
}
