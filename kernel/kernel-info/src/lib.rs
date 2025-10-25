//! # Kernel Helpers

#![no_std]
#![deny(unsafe_code)]

#[repr(C)]
pub struct KernelBootInfo {
    pub framebuffer_ptr: u64,
    pub framebuffer_width: usize,
    pub framebuffer_height: usize,
    pub framebuffer_stride: usize,
    pub reserved: u32,
}

pub type KernelEntry = extern "C" fn(*const KernelBootInfo) -> !;
