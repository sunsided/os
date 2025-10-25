extern crate alloc;

use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr::{self, read_unaligned};
use uefi::Status;
use uefi::boot::{self, AllocateType, MemoryType};

const PAGE_SIZE: u64 = 4096;
const PF_X: u32 = 0x1;
const PF_W: u32 = 0x2;
const PF_R: u32 = 0x4;

// Minimal ELF64 definitions
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const PT_LOAD: u32 = 1;
const EM_X86_64: u16 = 62;

// Public structures describing what to load
#[derive(Debug, Clone, Copy)]
pub struct LoadSegment {
    pub vaddr: u64,
    pub offset: u64,
    pub filesz: u64,
    pub memsz: u64,
    pub flags: u32,
    pub align: u64,
}

#[derive(Debug)]
pub struct ElfHeader {
    pub entry: u64,
    pub segments: Vec<LoadSegment>,
}

impl ElfHeader {
    const EI_MAGIC_BYTES: [u8; 4] = [0x7F, b'E', b'L', b'F'];

    /// Parse a 64-bit little-endian `x86_64` ELF image and collect `PT_LOAD` segments.
    /// Returns `Status::UNSUPPORTED` for any validation or bounds failure.
    pub fn parse_elf64(bytes: &[u8]) -> Result<Self, Status> {
        // Bounds for header
        if bytes.len() < size_of::<Elf64Ehdr>() {
            return Err(Status::UNSUPPORTED);
        }

        // SAFETY: We just checked bounds; using read_unaligned to avoid alignment assumptions.
        let ehdr = unsafe { read_unaligned(bytes.as_ptr().cast::<Elf64Ehdr>()) };

        // Validate magic 0x7F 'E''L''F'
        if ehdr.e_ident[0..4] != Self::EI_MAGIC_BYTES {
            return Err(Status::UNSUPPORTED);
        }
        // Class = 2 (ELF64), Data = 1 (little-endian), Version = 1
        if ehdr.e_ident[4] != 2 || ehdr.e_ident[5] != 1 || ehdr.e_ident[6] != 1 {
            return Err(Status::UNSUPPORTED);
        }

        if ehdr.e_machine != EM_X86_64 {
            return Err(Status::UNSUPPORTED);
        }

        if ehdr.e_phentsize as usize != size_of::<Elf64Phdr>() {
            return Err(Status::UNSUPPORTED);
        }

        // Program header table bounds
        let phoff = usize::try_from(ehdr.e_phoff).map_err(|_| Status::UNSUPPORTED)?;
        let phentsize = ehdr.e_phentsize as usize;
        let phnum = ehdr.e_phnum as usize;

        // Compute end of the table and check overflow/bounds
        let table_size = phentsize.checked_mul(phnum).ok_or(Status::UNSUPPORTED)?;
        let end = phoff.checked_add(table_size).ok_or(Status::UNSUPPORTED)?;
        if end > bytes.len() {
            return Err(Status::UNSUPPORTED);
        }

        let mut segments = Vec::new();

        for i in 0..phnum {
            let off = phoff + i * phentsize;
            // SAFETY: off + sizeof(Phdr) is within bytes by earlier bound check.
            let ph = unsafe { read_unaligned(bytes.as_ptr().add(off).cast::<Elf64Phdr>()) };
            if ph.p_type == PT_LOAD {
                segments.push(LoadSegment {
                    vaddr: ph.p_vaddr,
                    offset: ph.p_offset,
                    filesz: ph.p_filesz,
                    memsz: ph.p_memsz,
                    flags: ph.p_flags,
                    align: ph.p_align,
                });
            }
        }

        Ok(Self {
            entry: ehdr.e_entry,
            segments,
        })
    }
}

#[allow(clippy::inline_always)]
#[inline(always)]
fn align_down(x: u64, a: u64) -> u64 {
    debug_assert!(a.is_power_of_two());
    x & !(a - 1)
}

#[allow(clippy::inline_always)]
#[inline(always)]
fn align_up(x: u64, a: u64) -> u64 {
    debug_assert!(a.is_power_of_two());
    (x + a - 1) & !(a - 1)
}

/// Load all `PT_LOAD` segments into memory at their virtual addresses.
/// Returns `UNSUPPORTED` if exact address allocation fails or bounds are invalid.
pub fn load_pt_load_segments(elf_bytes: &[u8], hdr: &ElfHeader) -> Result<(), Status> {
    for seg in &hdr.segments {
        if seg.memsz == 0 {
            continue;
        }

        // Calculate page-aligned allocation range that fully covers the segment
        let seg_start = seg.vaddr;
        let seg_end = seg
            .vaddr
            .checked_add(seg.memsz)
            .ok_or(Status::UNSUPPORTED)?;
        let alloc_start = align_down(seg_start, PAGE_SIZE);
        let alloc_end = align_up(seg_end, PAGE_SIZE);
        let pages = (alloc_end - alloc_start) / PAGE_SIZE;

        // Executable segments in LOADER_CODE, others in LOADER_DATA
        let mem_type = if (seg.flags & PF_X) != 0 {
            MemoryType::LOADER_CODE
        } else {
            MemoryType::LOADER_DATA
        };

        // Reserve the exact address range
        // UEFI allocate_pages returns a pointer to the allocated region.
        let ptr =
            boot::allocate_pages(AllocateType::Address(alloc_start), mem_type, pages as usize)
                .map_err(|_| Status::UNSUPPORTED)?;
        // Compute the in-segment base pointer based on what firmware returned.
        // We requested [alloc_start, alloc_end), so seg_start may be inside that range.
        let base = ptr.as_ptr() as u64;
        let in_seg_off = seg_start - alloc_start; // safe: seg_start >= alloc_start by construction
        let dst_seg_ptr = (base + in_seg_off) as *mut u8;

        // Zero initialize the in-memory size for this segment (.bss etc.)
        let mem_len = usize::try_from(seg.memsz).map_err(|_| Status::UNSUPPORTED)?;
        unsafe {
            ptr::write_bytes(dst_seg_ptr, 0, mem_len);
        }

        // Copy file payload if any
        if seg.filesz != 0 {
            let src_off = usize::try_from(seg.offset).map_err(|_| Status::UNSUPPORTED)?;
            let file_len = usize::try_from(seg.filesz).map_err(|_| Status::UNSUPPORTED)?;
            let src_end = src_off.checked_add(file_len).ok_or(Status::UNSUPPORTED)?;
            if src_end > elf_bytes.len() {
                return Err(Status::UNSUPPORTED);
            }
            unsafe {
                ptr::copy_nonoverlapping(elf_bytes.as_ptr().add(src_off), dst_seg_ptr, file_len);
            }
        }
    }

    Ok(())
}
