//! # ELF Header Parsing

extern crate alloc;

use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr::read_unaligned;
use kernel_vmem::{PhysAddr, VirtAddr};
use uefi::Status;

// Minimal ELF64 definitions
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: VirtAddr,
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
    p_vaddr: VirtAddr,
    p_paddr: PhysAddr,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const PT_LOAD: u32 = 1;
const EM_X86_64: u16 = 62;

// Public structures describing what to load
#[derive(Debug, Clone, Copy)]
pub struct LoadSegment {
    pub vaddr: VirtAddr,
    pub offset: u64,
    pub filesz: u64,
    pub memsz: u64,
    // TODO: Rework flags using bitflags or bitfield_struct
    pub flags: u32,
    pub align: u64,
}

#[derive(Debug)]
pub struct ElfHeader {
    pub entry: VirtAddr,
    pub segments: Vec<LoadSegment>,
}

impl ElfHeader {
    const EI_MAGIC_BYTES: [u8; 4] = [0x7F, b'E', b'L', b'F'];

    /// Parse a 64-bit little-endian x86-64 ELF image and collect `PT_LOAD` segments.
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
