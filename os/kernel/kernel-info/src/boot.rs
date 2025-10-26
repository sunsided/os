//! # Kerrnel Boot Information

/// Kernel function pointer.
///
/// # ABI
/// The ABI is defined as `win64` since the kernel is called from a UEFI
/// (PE/COFF) application.
pub type KernelEntryFn = extern "win64" fn(*const KernelBootInfo) -> !;

/// Information the kernel needs right after `ExitBootServices`.
/// Keep this `#[repr(C)]` and prefer fixed-size integers over `u64` at the ABI boundary.
#[repr(C)]
#[derive(Clone)]
pub struct KernelBootInfo {
    /// Memory map information.
    pub mmap: MemoryMapInfo,

    /// RSDP (ACPI 2.0+) physical address, or 0 if not provided.
    pub rsdp_addr: u64,

    /// Framebuffer information, passed from UEFI GOP.
    pub fb: FramebufferInfo,
}

#[repr(C)]
#[derive(Clone)]
pub struct MemoryMapInfo {
    /// Pointer to the raw UEFI memory map buffer (array of `EFI_MEMORY_DESCRIPTOR` bytes).
    /// Pass 0 if you’re not handing the map to the kernel yet.
    pub mmap_ptr: u64,

    /// Length of the memory map buffer in **bytes**.
    pub mmap_len: u64,

    /// Size of a single memory descriptor in bytes (`EFI_MEMORY_DESCRIPTOR_VERSION` dependent).
    pub mmap_desc_size: u64,

    /// Descriptor version (from UEFI). Kernel can check it matches expectations.
    pub mmap_desc_version: u32,
}

#[repr(C)]
#[derive(Clone)]
pub struct FramebufferInfo {
    /// Linear framebuffer base address (CPU physical address). Valid to write after `ExitBootServices`.
    pub framebuffer_ptr: u64,

    /// Total framebuffer size in **bytes**. Helpful for bounds checks.
    pub framebuffer_size: u64,

    /// Visible width in **pixels**.
    pub framebuffer_width: u64,

    /// Visible height in **pixels**.
    pub framebuffer_height: u64,

    /// Pixels per scanline (a.k.a. stride). May be >= width due to padding.
    pub framebuffer_stride: u64,

    /// Pixel format tag (Rgb/Bgr/Bitmask/BltOnly). If `BltOnly`, you cannot draw directly.
    pub framebuffer_format: BootPixelFormat,

    /// Pixel bit masks (only meaningful when `framebuffer_format == Bitmask`).
    pub framebuffer_masks: BootPixelMasks,
}

/// Pixel format tag compatible with UEFI GOP.
/// We avoid Rust enums with payloads across the ABI boundary.
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum BootPixelFormat {
    /// UEFI `PixelFormat::Rgb` — 8:8:8 (or bitmask-equivalent), stored as R,G,B in low-to-high bytes.
    Rgb = 0,
    /// UEFI `PixelFormat::Bgr` — 8:8:8 (or bitmask-equivalent), stored as B,G,R in low-to-high bytes.
    Bgr = 1,
    /// UEFI `PixelFormat::Bitmask(mask)` — see the masks in `BootPixelMasks`.
    Bitmask = 2,
    /// UEFI `PixelFormat::BltOnly` — **no linear framebuffer available** (you can’t draw).
    BltOnly = 3,
}

/// Bit masks for `BootPixelFormat::Bitmask`.
/// For `Rgb`/`Bgr`, these are set to zero.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct BootPixelMasks {
    /// Mask of the red channel within a pixel (e.g., 0x00ff0000).
    pub red_mask: u32,
    /// Mask of the green channel within a pixel (e.g., 0x0000ff00).
    pub green_mask: u32,
    /// Mask of the blue channel within a pixel (e.g., 0x000000ff).
    pub blue_mask: u32,
    /// Mask of the alpha channel within a pixel (often 0x00000000 if opaque).
    pub alpha_mask: u32,
}
