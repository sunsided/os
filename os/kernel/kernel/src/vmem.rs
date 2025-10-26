use kernel_info::boot::{BootPixelFormat, FramebufferInfo};
use kernel_info::memory::{HHDM_BASE, KERNEL_BASE, PHYS_LOAD};
use kernel_vmem::{
    AddressSpace, MemoryPageFlags, PageSize, PhysAddr, PhysMapper, VirtAddr, read_cr3_phys,
};

const VGA_LIKE_OFFSET: u64 = (1u64 << 30) + 0x000B_8000; // 1 GiB + 0xB8000 inside HHDM range

const PT_POOL_BYTES: usize = 64 * 4096;

// Small pool for allocating page-table frames in the kernel (after ExitBootServices)
#[unsafe(link_section = ".bss.boot")]
static mut PT_POOL: Align4K<{ PT_POOL_BYTES }> = Align4K([0; PT_POOL_BYTES]);

fn pt_pool_phys_range() -> (u64, u64) {
    // Convert the VA of PT_POOL to a physical address using linker relationship: PHYS_LOAD + (va - KERNEL_BASE)
    #[allow(unused_unsafe)]
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

struct KernelPhysMapper;

impl PhysMapper for KernelPhysMapper {
    unsafe fn phys_to_mut<'a, T>(&self, pa: PhysAddr) -> &'a mut T {
        let va = (HHDM_BASE + pa.as_u64()) as *mut T;
        unsafe { &mut *va }
    }
}

#[repr(align(4096))]
struct Align4K<const N: usize>([u8; N]);

pub unsafe fn map_framebuffer_into_hhdm(fb: &FramebufferInfo) -> (u64, u64) {
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
