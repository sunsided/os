//! # ELF file handling

pub mod loader;
pub mod parser;

pub const PAGE_SIZE: u64 = 4096;

pub(crate) const PF_X: u32 = 0x1;
pub(crate) const PF_W: u32 = 0x2;
pub(crate) const PF_R: u32 = 0x4;
