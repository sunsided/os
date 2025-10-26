//! # Kernel Framebuffer helpers

use kernel_info::boot::{BootPixelFormat, FramebufferInfo};

#[allow(clippy::missing_safety_doc)]
pub unsafe fn fill_solid(fb: &FramebufferInfo, r: u8, g: u8, b: u8) {
    unsafe {
        if matches!(fb.framebuffer_format, BootPixelFormat::BltOnly) {
            return; // nothing to draw to
        }

        let bpp = 4usize; // bytes per pixel
        let row_bytes =
            usize::try_from(usize::try_from(fb.framebuffer_stride).unwrap_or_default() * bpp)
                .unwrap_or_default();

        let start_x = fb.framebuffer_width / 4;
        let end_x = fb.framebuffer_width * 3 / 4;
        let start_y = fb.framebuffer_height / 4;
        let end_y = fb.framebuffer_height * 3 / 4;

        for y in start_y..end_y {
            // move to the start of this row
            let mut row = (fb.framebuffer_ptr as *mut u8)
                .add(usize::try_from(y).unwrap_or_default() * row_bytes);
            // move to the start_x pixel
            row = row.add((start_x as usize) * bpp);

            for _x in start_x..end_x {
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
                    BootPixelFormat::BltOnly | BootPixelFormat::Bitmask => {}
                }
                row = row.add(bpp);
            }
        }
    }
}
