//! # Kernel Tracing helpers

use kernel_info::boot::{BootPixelFormat, KernelBootInfo};
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
        BootPixelFormat::Rgb => qemu_trace!("RGB"),
        BootPixelFormat::Bgr => qemu_trace!("BGR"),
        BootPixelFormat::Bitmask => qemu_trace!("Bitmask"),
        BootPixelFormat::BltOnly => qemu_trace!("BltOnly"),
    }
    qemu_trace!("\n");
}

pub fn log_ctrl_bits() {
    unsafe {
        let (mut cr4, mut efer): (u64, u64);
        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nostack, preserves_flags));
        // read MSR EFER (0xC000_0080)
        let lo: u32;
        let hi: u32;
        core::arch::asm!("rdmsr", in("ecx") 0xC000_0080u32, out("eax") lo, out("edx") hi);
        efer = ((hi as u64) << 32) | (lo as u64);
        qemu_trace!(
            "CR4={:016x} (SMEP={} SMAP={}) EFER={:016x} (NXE={})\n",
            cr4,
            (cr4 >> 20) & 1,
            (cr4 >> 21) & 1,
            efer,
            (efer >> 11) & 1
        );
    }
}
