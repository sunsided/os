//! # Trace output

use kernel_info::boot::KernelBootInfo;

pub fn trace<S>(message: S)
where
    S: AsRef<[u8]>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print(message);
    }
}

pub fn trace_usize<N>(number: N)
where
    N: Into<usize>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print_usize(number);
    }
}

pub fn trace_u64<N>(number: N)
where
    N: Into<u64>,
{
    #[cfg(feature = "qemu")]
    {
        kernel_qemu::dbg_print_u64(number);
    }
}

pub fn trace_boot_info(boot_info: &KernelBootInfo) {
    trace("Boot Info in UEFI Loader:\n");
    trace("   BI ptr = ");
    trace_usize(core::ptr::from_ref(boot_info) as usize);
    trace("\n");
    trace(" MMAP ptr = ");
    trace_u64(boot_info.mmap.mmap_ptr);
    trace(", MMAP len = ");
    trace_u64(boot_info.mmap.mmap_len);
    trace(", MMAP desc size = ");
    trace_u64(boot_info.mmap.mmap_desc_size);
    trace(", MMAP desc version = ");
    trace_usize(usize::try_from(boot_info.mmap.mmap_desc_version).unwrap_or_default());
    trace(", rsdp addr = ");
    trace_usize(usize::try_from(boot_info.rsdp_addr).unwrap_or_default());
    trace("\n");
    trace("   FB ptr = ");
    trace_u64(boot_info.fb.framebuffer_ptr);
    trace(", FB size = ");
    trace_u64(boot_info.fb.framebuffer_size);
    trace(", FB width = ");
    trace_u64(boot_info.fb.framebuffer_width);
    trace(", FB height = ");
    trace_u64(boot_info.fb.framebuffer_height);
    trace(", FB stride = ");
    trace_u64(boot_info.fb.framebuffer_stride);
    trace(", FB format = ");
    match boot_info.fb.framebuffer_format {
        kernel_info::boot::BootPixelFormat::Rgb => trace("RGB"),
        kernel_info::boot::BootPixelFormat::Bgr => trace("BGR"),
        kernel_info::boot::BootPixelFormat::Bitmask => trace("Bitmask"),
        kernel_info::boot::BootPixelFormat::BltOnly => trace("BltOnly"),
    }
    trace("\n");
}
