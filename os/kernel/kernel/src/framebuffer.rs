//! # Kernel Framebuffer helpers

use kernel_info::boot::{BootPixelFormat, FramebufferInfo};

/// Virtual offset inside the HHDM where we map the framebuffer.
///
/// We pick **`HHDM_BASE + 1 GiB + 0xB8000`** (“VGA-like”) to stay well clear
/// of a potential 1 GiB huge page mapping at the very start of the HHDM.
///
/// This reduces the risk of having to split a 1 GiB page into 4 KiB pages.
pub const VGA_LIKE_OFFSET: u64 = 1u64 << 30; // 1 GiB inside HHDM range

#[allow(clippy::missing_safety_doc, clippy::many_single_char_names)]
pub unsafe fn fill_solid(fb: &FramebufferInfo, r: u8, g: u8, b: u8) {
    // Nothing to draw into
    if matches!(fb.framebuffer_format, BootPixelFormat::BltOnly) {
        return;
    }

    // Precompute the packed pixel once (little-endian):
    // - RGB format => bytes [R, G, B, 0xFF] -> value 0xFF_BB_GG_RR
    // - BGR format => bytes [B, G, R, 0xFF] -> value 0xFF_RR_GG_BB
    let px: u32 = match fb.framebuffer_format {
        BootPixelFormat::Rgb => {
            (0xFFu32 << 24) | (u32::from(b) << 16) | (u32::from(g) << 8) | u32::from(r)
        }
        BootPixelFormat::Bgr => {
            (0xFFu32 << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
        }
        BootPixelFormat::Bitmask | BootPixelFormat::BltOnly => return,
    };

    // 32-bit pixels
    let base = fb.framebuffer_ptr as *mut u32;

    // pixels per row (GOP "PixelsPerScanLine")
    let stride = usize::try_from(fb.framebuffer_stride).unwrap_or_default();
    if stride == 0 {
        return;
    }

    let w = usize::try_from(fb.framebuffer_width).unwrap_or_default();
    let h = usize::try_from(fb.framebuffer_height).unwrap_or_default();

    let start_x = w / 4;
    let end_x = w * 3 / 4;
    let start_y = h / 4;
    let end_y = h * 3 / 4;

    for y in start_y..end_y {
        // Pointer to first pixel in this row at start_x
        let mut p = unsafe { base.add(y * stride + start_x) };

        // Fill [start_x, end_x)
        for _ in start_x..end_x {
            unsafe {
                p.write_volatile(px);
                p = p.add(1);
            }
        }
    }
}
