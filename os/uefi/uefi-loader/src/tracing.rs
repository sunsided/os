//! # Trace output

use kernel_info::boot::KernelBootInfo;
use kernel_qemu::qemu_trace;

pub fn trace_boot_info(boot_info: &KernelBootInfo, bi_ptr_va: u64, kernel_va: u64) {
    qemu_trace!("Boot Info in UEFI Loader:\n");
    qemu_trace!(
        "   Kernel = {kernel_va:018x} (@{} MiB)",
        kernel_va / 1024 / 1024
    );
    qemu_trace!("\n");
    qemu_trace!(
        "   BI ptr = {:018x} (@{} MiB)",
        core::ptr::from_ref(boot_info) as usize,
        core::ptr::from_ref(boot_info) as usize / 1024 / 1024
    );
    qemu_trace!("\n");
    qemu_trace!(
        "       VA = {bi_ptr_va:018x} (@{} MiB)",
        bi_ptr_va / 1024 / 1024
    );
    qemu_trace!("\n");
    qemu_trace!(
        " MMAP ptr = {:018x} (@{} MiB)",
        boot_info.mmap.mmap_ptr,
        boot_info.mmap.mmap_ptr / 1024 / 1024
    );
    qemu_trace!(", len = {}", boot_info.mmap.mmap_len);
    qemu_trace!(", desc size = {}", boot_info.mmap.mmap_desc_size);
    qemu_trace!(
        ", desc version = {}",
        usize::try_from(boot_info.mmap.mmap_desc_version).unwrap_or_default()
    );
    qemu_trace!(
        ", rsdp addr = {}",
        usize::try_from(boot_info.rsdp_addr).unwrap_or_default()
    );
    qemu_trace!("\n");
    qemu_trace!(
        "   FB ptr = {:018x} (@{} MiB)",
        boot_info.fb.framebuffer_ptr,
        boot_info.fb.framebuffer_ptr / 1024 / 1024
    );
    qemu_trace!(", size = {}", boot_info.fb.framebuffer_size);
    qemu_trace!(", width = {}", boot_info.fb.framebuffer_width);
    qemu_trace!(", height = {}", boot_info.fb.framebuffer_height);
    qemu_trace!(", stride = {}", boot_info.fb.framebuffer_stride);
    qemu_trace!(", format = ");
    match boot_info.fb.framebuffer_format {
        kernel_info::boot::BootPixelFormat::Rgb => qemu_trace!("RGB"),
        kernel_info::boot::BootPixelFormat::Bgr => qemu_trace!("BGR"),
        kernel_info::boot::BootPixelFormat::Bitmask => qemu_trace!("Bitmask"),
        kernel_info::boot::BootPixelFormat::BltOnly => qemu_trace!("BltOnly"),
    }
    qemu_trace!("\n");
}
