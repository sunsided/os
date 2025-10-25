//! # Kernel Helpers

#![no_std]
#![deny(unsafe_code)]

pub type KernelEntry = extern "C" fn(*const KernelBootInfo) -> !;

/// Information the kernel needs right after `ExitBootServices`.
/// Keep this `#[repr(C)]` and prefer fixed-size integers over `usize` at the ABI boundary.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct KernelBootInfo {
    // ---------------- Memory map (optional but recommended) ----------------
    /// Pointer to the raw UEFI memory map buffer (array of `EFI_MEMORY_DESCRIPTOR` bytes).
    /// Pass 0 if you’re not handing the map to the kernel yet.
    pub mmap_ptr: usize,

    /// Length of the memory map buffer in **bytes**.
    pub mmap_len: usize,

    /// Size of a single memory descriptor in bytes (`EFI_MEMORY_DESCRIPTOR_VERSION` dependent).
    pub mmap_desc_size: usize,

    /// Descriptor version (from UEFI). Kernel can check it matches expectations.
    pub mmap_desc_version: u32,

    // ---------------- Firmware tables (optional) ----------------
    /// RSDP (ACPI 2.0+) physical address, or 0 if not provided.
    pub rsdp_addr: u64,

    // ---------------- Framebuffer ----------------
    /// Linear framebuffer base address (CPU physical address). Valid to write after `ExitBootServices`.
    pub framebuffer_ptr: usize,

    /// Total framebuffer size in **bytes**. Helpful for bounds checks.
    pub framebuffer_size: usize,

    /// Visible width in **pixels**.
    pub framebuffer_width: usize,

    /// Visible height in **pixels**.
    pub framebuffer_height: usize,

    /// Pixels per scanline (a.k.a. stride). May be >= width due to padding.
    pub framebuffer_stride: usize,

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
