pub mod helpers;

use bitfield_struct::bitfield;
use kernel_memory_addresses::{PhysicalAddress, VirtualAddress};

#[derive(Debug)]
pub enum ElfErr {
    TooShort,
    BadMagic,
    BadClass,
    BadMachine,
    BadHeader,
    Oob,
    BadPh,
    MapFail,
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code, clippy::struct_field_names)]
pub struct Eh64 {
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: VirtualAddress,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    _e_shentsize: u16,
    _e_shnum: u16,
    _e_shstrndx: u16,
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code, clippy::struct_field_names)]
pub struct Ph64 {
    pub p_type: u32,
    pub p_flags: PFlags,
    pub p_offset: u64,
    pub p_vaddr: VirtualAddress,
    pub p_paddr: PhysicalAddress,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

/// `Elf64_Phdr.p_flags` (SVr4): bit0=X, bit1=W, bit2=R.
#[bitfield(u32)]
pub struct PFlags {
    #[bits(1)]
    pub execute: bool, // PF_X = 1
    #[bits(1)]
    pub write: bool, // PF_W = 2
    #[bits(1)]
    pub read: bool, // PF_R = 4
    #[bits(29)]
    __: u32,
}

const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;

#[inline]
fn le16(x: &[u8]) -> u16 {
    u16::from_le_bytes([x[0], x[1]])
}

#[inline]
fn le32(x: &[u8]) -> u32 {
    u32::from_le_bytes([x[0], x[1], x[2], x[3]])
}

#[inline]
fn le64(x: &[u8]) -> u64 {
    u64::from_le_bytes([x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]])
}

#[allow(dead_code)]
pub struct ElfView<'a> {
    bytes: &'a [u8],
    pub eh: Eh64,
    ph: PhSlice<'a>,
}

#[allow(clippy::cast_possible_truncation)]
pub fn elf64_view(bytes: &[u8]) -> Result<ElfView<'_>, ElfErr> {
    use ElfErr::{BadClass, BadHeader, BadMachine, BadMagic, Oob, TooShort};
    if bytes.len() < 64 {
        return Err(TooShort);
    }

    // e_ident
    if &bytes[0..4] != b"\x7FELF" {
        return Err(BadMagic);
    }

    // ELFCLASS64
    if bytes[4] != 2 {
        return Err(BadClass);
    }

    // little-endian
    if bytes[5] != 1 {
        return Err(BadHeader);
    }

    let eh = Eh64 {
        e_type: le16(&bytes[16..18]),
        e_machine: le16(&bytes[18..20]),
        e_version: le32(&bytes[20..24]),
        e_entry: VirtualAddress::new(le64(&bytes[24..32])),
        e_phoff: le64(&bytes[32..40]),
        e_shoff: le64(&bytes[40..48]),
        e_flags: le32(&bytes[48..52]),
        e_ehsize: le16(&bytes[52..54]),
        e_phentsize: le16(&bytes[54..56]),
        e_phnum: le16(&bytes[56..58]),
        _e_shentsize: le16(&bytes[58..60]),
        _e_shnum: le16(&bytes[60..62]),
        _e_shstrndx: le16(&bytes[62..64]),
    };

    if !(eh.e_type == ET_EXEC || eh.e_type == ET_DYN) {
        return Err(BadHeader);
    }

    if eh.e_machine != EM_X86_64 {
        return Err(BadMachine);
    }

    if eh.e_version != 1 {
        return Err(BadHeader);
    }

    if eh.e_ehsize as usize > bytes.len() {
        return Err(BadHeader);
    }

    if eh.e_phentsize as usize != 56 {
        return Err(BadHeader);
    }

    let phoff = eh.e_phoff as usize;
    let phnum = eh.e_phnum as usize;
    let entsz = eh.e_phentsize as usize;
    let need = phoff
        .checked_add(phnum.checked_mul(entsz).ok_or(Oob)?)
        .ok_or(Oob)?;
    if need > bytes.len() {
        return Err(Oob);
    }

    let ph = PhSlice {
        b: bytes,
        off: phoff,
        num: phnum,
        stride: entsz,
    };

    Ok(ElfView { bytes, eh, ph })
}

// Program-header “view” without allocations.
#[derive(Copy, Clone)]
pub struct PhSlice<'a> {
    b: &'a [u8],
    off: usize,
    num: usize,
    stride: usize,
}

impl PhSlice<'_> {
    fn get(&self, i: usize) -> Option<Ph64> {
        if i >= self.num {
            return None;
        }
        let p = self.off + i * self.stride;
        let s = self.b.get(p..p + 56)?; // bounds-checked
        Some(Ph64 {
            p_type: le32(&s[0..4]),
            p_flags: PFlags::from_bits(le32(&s[4..8])),
            p_offset: le64(&s[8..16]),
            p_vaddr: VirtualAddress::new(le64(&s[16..24])),
            p_paddr: PhysicalAddress::new(le64(&s[24..32])),
            p_filesz: le64(&s[32..40]),
            p_memsz: le64(&s[40..48]),
            p_align: {
                let a = le64(&s[48..56]);
                if a == 0 { 1 } else { a } // ELF permits 0 → no alignment requirement
            },
        })
    }

    const fn len(&self) -> usize {
        self.num
    }
}

pub struct PhIter<'a> {
    ps: PhSlice<'a>,
    i: usize,
}

impl Iterator for PhIter<'_> {
    type Item = Ph64;
    fn next(&mut self) -> Option<Self::Item> {
        let v = self.ps.get(self.i)?;
        self.i += 1;
        Some(v)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.ps.len().saturating_sub(self.i);
        (r, Some(r))
    }
}

impl core::iter::FusedIterator for PhIter<'_> {}

impl ElfView<'_> {
    /// Iterate all program headers.
    pub const fn iter_ph(&self) -> PhIter<'_> {
        PhIter { ps: self.ph, i: 0 }
    }

    /// Iterate only `PT_LOAD` headers (filtering at the edge).
    pub fn iter_pt_load(&self) -> impl Iterator<Item = Ph64> + '_ {
        self.iter_ph().filter(|ph| ph.p_type == PT_LOAD)
    }

    /// True for PIE (`ET_DYN`), false for fixed `ET_EXEC`.
    pub const fn is_pie(&self) -> bool {
        self.eh.e_type == ET_DYN
    }

    pub const fn entry(&self) -> VirtualAddress {
        self.eh.e_entry
    }
}
