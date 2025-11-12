//! # Kernel Tracing helpers

use kernel_info::boot::{BootPixelFormat, KernelBootInfo};
use log::info;

pub fn trace_boot_info(boot_info: &KernelBootInfo) {
    info!(
        concat!(
            "Boot Info in Kernel:\n",
            "  BI ptr   = {bi:#018x}\n",
            "  MMAP ptr = {mmap_ptr:#018x}, len = {mmap_len}, desc size = {mmap_desc_size}, desc ver = {mmap_desc_ver}, rsdp addr = {rsdp_addr}\n",
            "  FB ptr   = {fb_ptr:#018x}, size = {fb_size}, width = {fb_width}, height = {fb_height}, stride = {fb_stride}, format = {fb_fmt}"
        ),
        bi = core::ptr::from_ref(boot_info) as usize,
        mmap_ptr = boot_info.mmap.mmap_ptr,
        mmap_len = boot_info.mmap.mmap_len,
        mmap_desc_size = boot_info.mmap.mmap_desc_size,
        mmap_desc_ver = usize::try_from(boot_info.mmap.mmap_desc_version).unwrap_or_default(),
        rsdp_addr = usize::try_from(boot_info.rsdp_addr).unwrap_or_default(),
        fb_ptr = boot_info.fb.framebuffer_ptr,
        fb_size = boot_info.fb.framebuffer_size,
        fb_width = boot_info.fb.framebuffer_width,
        fb_height = boot_info.fb.framebuffer_height,
        fb_stride = boot_info.fb.framebuffer_stride,
        fb_fmt = match boot_info.fb.framebuffer_format {
            BootPixelFormat::Rgb => "RGB",
            BootPixelFormat::Bgr => "BGR",
            BootPixelFormat::Bitmask => "Bitmask",
            BootPixelFormat::BltOnly => "BltOnly",
        },
    );
}

pub fn log_ctrl_bits() {
    unsafe {
        let (mut cr4, efer): (u64, u64);
        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nostack, preserves_flags));
        // read MSR EFER (0xC000_0080)
        let lo: u32;
        let hi: u32;
        core::arch::asm!("rdmsr", in("ecx") 0xC000_0080u32, out("eax") lo, out("edx") hi);
        efer = (u64::from(hi) << 32) | u64::from(lo);
        info!(
            "CR4={:016x} (SMEP={} SMAP={}) EFER={:016x} (NXE={})",
            cr4,
            (cr4 >> 20) & 1,
            (cr4 >> 21) & 1,
            efer,
            (efer >> 11) & 1
        );
    }
}
