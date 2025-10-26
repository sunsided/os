//! # Kernel Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

// Bring in the global allocator.
extern crate kernel_alloc;

use core::hint::spin_loop;
use kernel_info::boot::{BootPixelFormat, FramebufferInfo, KernelBootInfo};
use kernel_info::memory::{HHDM_BASE, KERNEL_BASE, PHYS_LOAD};
use kernel_qemu::qemu_trace;
use kernel_vmem::{
    AddressSpace, MemoryPageFlags, PageSize, PhysAddr, PhysMapper, VirtAddr, read_cr3_phys,
};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        spin_loop();
    }
}

/// Stack size.
const BOOT_STACK_SIZE: usize = 64 * 1024;

/// 16-byte aligned stack
#[repr(align(16))]
struct Aligned<const N: usize>([u8; N]);

#[unsafe(link_section = ".bss.boot")]
#[unsafe(no_mangle)]
static mut BOOT_STACK: Aligned<BOOT_STACK_SIZE> = Aligned([0; BOOT_STACK_SIZE]);

/// The kernel entry point
///
/// # UEFI Interaction
/// The UEFI loader will jump here after `ExitBootServices`.
///
/// # ABI
/// The ABI is defined as `win64` since the kernel is called from a UEFI
/// (PE/COFF) application. This passes the `boot_info` pointer as `RCX`
/// (as opposed to `RDI` for the SysV ABI).
///
/// # Naked function & Stack
/// This is a naked function in order to set up the stack ourselves. Without
/// the `naked` attribute (and the [`naked_asm`](core::arch::naked_asm) instruction), Rust
/// compiler would apply its own assumptions based on the C ABI and would attempt to
/// unwind the stack on the call into [`kernel_entry`]. Since we're clearing out the stack
/// here, this would cause UB.
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "win64" fn _start_kernel(_boot_info: *const KernelBootInfo) {
    core::arch::naked_asm!(
        "cli",

        // These OUTs need no memory; if you see them, trampoline code page is mapped in new CR3.
        "mov    dx, 0x402",
        "mov    al, 'C'",
        "out    dx, al",
        // continue as usual

        // save RCX (boot_info per Win64)
        "mov r12, rcx",

        // Build our own kernel stack and establish a valid call frame for kernel_entry
        "lea rax, [rip + {stack_sym}]",
        "add rax, {stack_size}",
        // Align down to 16
        "and rax, -16",
        // Reserve 32-byte shadow space
        "sub rax, 32",
        // Set RSP to the prepared value
        "mov rsp, rax",
        // Emulate a CALL by pushing a dummy return address (so RSP % 16 == 8 at entry)
        "push 0",
        "xor rbp, rbp",

        // Restore boot_info into the expected arg register (SysV/C ABI)
        "mov rdi, r12",

        // These OUTs need no memory; if you see them, trampoline code page is mapped in new CR3.
        "mov    dx, 0x402",
        "mov    al, 'D'",
        "out    dx, al",
        // continue as usual

        // Jump to Rust entry and never return
        "jmp {rust_entry}",
        stack_sym = sym BOOT_STACK,
        stack_size = const BOOT_STACK_SIZE,
        rust_entry = sym kernel_entry,
    );
}

/// Kernel entry running on normal stack.
///
/// # Notes
/// * `no_mangle` is used so that [`_start_kernel`] can jump to it by name.
/// * It uses C ABI to have a defined convention when calling in from ASM.
/// * The [`_start_kernel`] function keeps `boot_info` in `RDI`, matching C ABI expectations.
#[unsafe(no_mangle)]
extern "C" fn kernel_entry(boot_info: *const KernelBootInfo) -> ! {
    #[cfg(feature = "qemu")]
    qemu_trace!("Kernel reporting to QEMU!\n");

    // (You can enable interrupts here when ready.)
    let bi = unsafe { &*boot_info };
    kernel_main(bi)
}

fn kernel_main(bi: &KernelBootInfo) -> ! {
    // Map the framebuffer into HHDM at a VGA-like offset and use it virtually
    let (fb_va_base, mapped_len) = unsafe { map_framebuffer_into_hhdm(&bi.fb) };
    let mut fb_virt = bi.fb.clone();
    fb_virt.framebuffer_ptr = fb_va_base;
    #[cfg(feature = "qemu")]
    {
        qemu_trace!("Entering Kernel main loop ...\n");
        trace_boot_info(bi);
    }

    #[cfg(feature = "qemu")]
    match bi.fb.framebuffer_format {
        BootPixelFormat::Rgb => qemu_trace!("RGB framebuffer\n"),
        BootPixelFormat::Bgr => qemu_trace!("BGR framebuffer\n"),
        BootPixelFormat::Bitmask => qemu_trace!("Bitmask framebuffer\n"),
        BootPixelFormat::BltOnly => qemu_trace!("BltOnly framebuffer\n"),
    }

    loop {
        // TODO: framebuffer mapped into HHDM at VGA-like offset
        qemu_trace!("loop-de-loop\n");
        unsafe { fill_solid(&fb_virt, 255, 0, 0) };
        spin_loop();
    }
}

const VGA_LIKE_OFFSET: u64 = (1u64 << 30) + 0x000B_8000; // 1 GiB + 0xB8000 inside HHDM range

struct KernelPhysMapper;
impl PhysMapper for KernelPhysMapper {
    unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T {
        let va = (HHDM_BASE + pa.as_u64()) as *mut T;
        unsafe { &mut *va }
    }
}

#[repr(align(4096))]
struct Align4K<const N: usize>([u8; N]);

const PT_POOL_BYTES: usize = 64 * 4096;

// Small pool for allocating page-table frames in the kernel (after ExitBootServices)
#[unsafe(link_section = ".bss.boot")]
static mut PT_POOL: Align4K<{ PT_POOL_BYTES }> = Align4K([0; PT_POOL_BYTES]);

fn pt_pool_phys_range() -> (u64, u64) {
    // Convert the VA of PT_POOL to a physical address using linker relationship: PHYS_LOAD + (va - KERNEL_BASE)
    let va = unsafe { core::ptr::addr_of!(PT_POOL) as u64 };
    let pa_start = PHYS_LOAD + (va - KERNEL_BASE);
    let pa_end = pa_start + (PT_POOL_BYTES as u64);
    (pa_start, pa_end)
}

struct KernelBumpAlloc {
    next: u64,
    end: u64,
}

impl KernelBumpAlloc {
    fn new() -> Self {
        let (start, end) = pt_pool_phys_range();
        let next = (start + 0xfff) & !0xfff; // align up
        Self { next, end }
    }
}

impl kernel_vmem::FrameAlloc for KernelBumpAlloc {
    fn alloc_4k(&mut self) -> Option<PhysAddr> {
        if self.next + 4096 > self.end {
            return None;
        }
        let pa = self.next;
        self.next += 4096;
        // Zero the frame via HHDM
        let mapper = KernelPhysMapper;
        unsafe {
            core::ptr::write_bytes(mapper.phys_to_mut::<u8>(PhysAddr::from_u64(pa)), 0, 4096);
        }
        Some(PhysAddr::from_u64(pa))
    }
}

unsafe fn map_framebuffer_into_hhdm(fb: &FramebufferInfo) -> (u64, u64) {
    if matches!(fb.framebuffer_format, BootPixelFormat::BltOnly) {
        return (0, 0);
    }

    let fb_pa = fb.framebuffer_ptr;
    let fb_len = fb.framebuffer_size;

    let page = 4096u64;
    let pa_start = fb_pa & !(page - 1);
    let pa_end = (fb_pa + fb_len + page - 1) & !(page - 1);

    // Choose a VA inside HHDM range but outside the 1 GiB huge mapping to avoid splitting it.
    let va_base = HHDM_BASE + VGA_LIKE_OFFSET;
    let va_start = va_base + (fb_pa - pa_start);

    // Map pages
    let mapper = KernelPhysMapper;
    let mut alloc = KernelBumpAlloc::new();
    let aspace = AddressSpace::new(&mapper, unsafe { read_cr3_phys() });

    let mut pa = pa_start;
    let mut va = va_start & !(page - 1);
    while pa < pa_end {
        aspace
            .map_one(
                &mut alloc,
                VirtAddr::from_u64(va),
                PhysAddr::from_u64(pa),
                PageSize::Size4K,
                MemoryPageFlags::WRITABLE | MemoryPageFlags::GLOBAL | MemoryPageFlags::NX,
            )
            .expect("map framebuffer page");
        pa += page;
        va += page;
    }

    (va_start, pa_end - pa_start)
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn fill_solid(fb: &FramebufferInfo, r: u8, g: u8, b: u8) {
    unsafe {
        if matches!(fb.framebuffer_format, BootPixelFormat::BltOnly) {
            return; // nothing to draw to
        }

        let mut p = fb.framebuffer_ptr as *mut u8;
        let bpp = 4; // common on PC GOP; for Bitmask you could compute bpp from masks
        let row_bytes = fb.framebuffer_stride * bpp;
        let row_bytes = usize::try_from(row_bytes).unwrap_or_default(); // TODO: Use a panic here

        for _y in 0..fb.framebuffer_height {
            let mut row = p;
            for _x in 0..fb.framebuffer_width {
                match fb.framebuffer_format {
                    BootPixelFormat::Rgb => {
                        core::ptr::write_unaligned(row.add(0), r);
                        core::ptr::write_unaligned(row.add(1), g);
                        core::ptr::write_unaligned(row.add(2), b);
                        core::ptr::write_unaligned(row.add(3), 0xff);
                    }
                    BootPixelFormat::Bgr => {
                        core::ptr::write_unaligned(row.add(0), b);
                        core::ptr::write_unaligned(row.add(1), g);
                        core::ptr::write_unaligned(row.add(2), r);
                        core::ptr::write_unaligned(row.add(3), 0xff);
                    }
                    BootPixelFormat::BltOnly | BootPixelFormat::Bitmask => {
                        // Convert (r,g,b) to a u32 using masks; left as an exercise.
                    }
                }
                row = row.add(4);
            }
            p = p.add(row_bytes);
        }
    }
}

#[cfg(feature = "qemu")]
fn trace_boot_info(boot_info: &KernelBootInfo) {
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
