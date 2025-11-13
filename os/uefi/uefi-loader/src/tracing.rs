//! # Trace output

use crate::TrampolineStackVirtualAddress;
use kernel_info::boot::KernelBootInfo;
use kernel_memory_addresses::VirtualAddress;

pub fn trace_boot_info(
    boot_info: &KernelBootInfo,
    bi_ptr_va: VirtualAddress,
    kernel_va: VirtualAddress,
    trampoline_stack_va: TrampolineStackVirtualAddress,
) {
    use log::info;

    info!(
        concat!(
            "Boot Info in UEFI Loader:\n",
            "  Kernel   = {kernel_va:?}\n",
            "  Trampol. = {trampoline_stack_va:?}\n",
            "  BI ptr   = {bi_ptr:#018x} (@{bi_mib} MiB)\n",
            "       VA  = {bi_ptr_va:?}\n",
            "  MMAP ptr = {mmap_ptr:#018x} (@{mmap_mib} MiB), ",
            "len = {mmap_len}, desc size = {mmap_desc_size}, ",
            "desc ver = {mmap_desc_ver}, rsdp addr = {rsdp_addr}\n",
            "  FB ptr   = {fb_ptr:#018x} (@{fb_mib} MiB), ",
            "size = {fb_size}, width = {fb_width}, height = {fb_height}, ",
            "stride = {fb_stride}, format = {fb_fmt}"
        ),
        kernel_va = kernel_va,
        trampoline_stack_va = trampoline_stack_va,
        bi_ptr = core::ptr::from_ref(boot_info) as usize,
        bi_mib = (core::ptr::from_ref(boot_info) as usize) / 1024 / 1024,
        bi_ptr_va = bi_ptr_va,
        mmap_ptr = boot_info.mmap.mmap_ptr,
        mmap_mib = boot_info.mmap.mmap_ptr / 1024 / 1024,
        mmap_len = boot_info.mmap.mmap_len,
        mmap_desc_size = boot_info.mmap.mmap_desc_size,
        mmap_desc_ver = usize::try_from(boot_info.mmap.mmap_desc_version).unwrap_or_default(),
        rsdp_addr = usize::try_from(boot_info.rsdp_addr).unwrap_or_default(),
        fb_ptr = boot_info.fb.framebuffer_ptr,
        fb_mib = boot_info.fb.framebuffer_ptr / 1024 / 1024,
        fb_size = boot_info.fb.framebuffer_size,
        fb_width = boot_info.fb.framebuffer_width,
        fb_height = boot_info.fb.framebuffer_height,
        fb_stride = boot_info.fb.framebuffer_stride,
        fb_fmt = match boot_info.fb.framebuffer_format {
            kernel_info::boot::BootPixelFormat::Rgb => "RGB",
            kernel_info::boot::BootPixelFormat::Bgr => "BGR",
            kernel_info::boot::BootPixelFormat::Bitmask => "Bitmask",
            kernel_info::boot::BootPixelFormat::BltOnly => "BltOnly",
        },
    );
}
