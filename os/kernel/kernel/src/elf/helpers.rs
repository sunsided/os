use crate::elf::{ElfErr, ElfView, PFlags, Ph64};
use bitfield_struct::bitfield;

/// Compute a load bias for `ET_DYN`: min(vaddr of `PT_LOAD`), aligned to `max(p_align`, 0x1000).
pub fn pie_bias(view: &ElfView<'_>) -> Option<u64> {
    if !view.is_pie() {
        return Some(0);
    }
    let mut min = u64::MAX;
    let mut max_align = 0x1000u64;
    for ph in view.iter_pt_load() {
        let va = ph.p_vaddr.as_u64();
        if va < min {
            min = va;
        }
        if ph.p_align > max_align {
            max_align = ph.p_align;
        }
    }
    if min == u64::MAX {
        None
    } else {
        Some(min & !(max_align - 1))
    }
}

/// Get the file bytes backing a `PT_LOAD` (filesz may be < memsz).
pub fn segment_file_bytes<'a>(bytes: &'a [u8], ph: &Ph64) -> Result<&'a [u8], ElfErr> {
    let off = usize::try_from(ph.p_offset).map_err(|_| ElfErr::Oob)?;
    let sz = usize::try_from(ph.p_filesz).map_err(|_| ElfErr::Oob)?;
    let end = off.checked_add(sz).ok_or(ElfErr::Oob)?;
    bytes.get(off..end).ok_or(ElfErr::Oob)
}

#[bitfield(u64)]
pub struct VmFlags {
    #[bits(1)]
    pub user: bool,
    #[bits(1)]
    pub write: bool,
    #[bits(1)]
    pub execute: bool,
    #[bits(61)]
    __: u64,
}

impl From<PFlags> for VmFlags {
    fn from(value: PFlags) -> Self {
        Self::new()
            .with_user(true)
            .with_execute(value.execute())
            .with_write(value.write())
    }
}
