//! # Trace output

use kernel_info::boot::KernelBootInfo;
use kernel_qemu::qemu_trace;

pub fn trace_boot_info(boot_info: &KernelBootInfo) {
    qemu_trace!("Boot Info in UEFI Loader:\n");
    qemu_trace!(
        "   BI ptr = {:018x}",
        core::ptr::from_ref(boot_info) as usize
    );
    qemu_trace!("\n");
    qemu_trace!(" MMAP ptr = {:018x}", boot_info.mmap.mmap_ptr);
    qemu_trace!(", MMAP len = {}", boot_info.mmap.mmap_len);
    qemu_trace!(", MMAP desc size = {}", boot_info.mmap.mmap_desc_size);
    qemu_trace!(
        ", MMAP desc version = {}",
        usize::try_from(boot_info.mmap.mmap_desc_version).unwrap_or_default()
    );
    qemu_trace!(
        ", rsdp addr = {}",
        usize::try_from(boot_info.rsdp_addr).unwrap_or_default()
    );
    qemu_trace!("\n");
    qemu_trace!("   FB ptr = {:018x}", boot_info.fb.framebuffer_ptr);
    qemu_trace!(", FB size = {}", boot_info.fb.framebuffer_size);
    qemu_trace!(", FB width = {}", boot_info.fb.framebuffer_width);
    qemu_trace!(", FB height = {}", boot_info.fb.framebuffer_height);
    qemu_trace!(", FB stride = {}", boot_info.fb.framebuffer_stride);
    qemu_trace!(", FB format = ");
    match boot_info.fb.framebuffer_format {
        kernel_info::boot::BootPixelFormat::Rgb => qemu_trace!("RGB"),
        kernel_info::boot::BootPixelFormat::Bgr => qemu_trace!("BGR"),
        kernel_info::boot::BootPixelFormat::Bitmask => qemu_trace!("Bitmask"),
        kernel_info::boot::BootPixelFormat::BltOnly => qemu_trace!("BltOnly"),
    }
    qemu_trace!("\n");
}
