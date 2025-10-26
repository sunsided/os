//! # ELF file handling

pub mod loader;
pub mod parser;
pub mod vmem;

const PAGE_SIZE: u64 = 4096;

const PF_X: u32 = 0x1;
const PF_W: u32 = 0x2;
const PF_R: u32 = 0x4;
