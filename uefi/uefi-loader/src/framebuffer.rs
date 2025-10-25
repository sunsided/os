//! # GOP for the Kernel

use kernel_info::FramebufferInfo;
use uefi::boot::ScopedProtocol;
use uefi::proto::console::gop::{GraphicsOutput, Mode, PixelFormat};
use uefi::{Status, boot};

/// Fetch an optimal framebuffer for the Kernel.
pub fn get_framebuffer() -> Result<FramebufferInfo, Status> {
    uefi::println!("Obtaining Graphics Output Protocol (GOP)");
    let mut gop = match get_gop() {
        Ok(gop) => gop,
        Err(e) => {
            uefi::println!("Failed to get GOP: {e:?}");
            return Err(Status::UNSUPPORTED);
        }
    };

    let mode = gop.current_mode_info();
    let (framebuffer_width, framebuffer_height) = mode.resolution();

    let (framebuffer_format, framebuffer_masks) = match mode.pixel_format() {
        PixelFormat::Rgb => (
            kernel_info::BootPixelFormat::Rgb,
            kernel_info::BootPixelMasks {
                red_mask: 0,
                green_mask: 0,
                blue_mask: 0,
                alpha_mask: 0,
            },
        ),
        PixelFormat::Bgr => (
            kernel_info::BootPixelFormat::Bgr,
            kernel_info::BootPixelMasks {
                red_mask: 0,
                green_mask: 0,
                blue_mask: 0,
                alpha_mask: 0,
            },
        ),
        PixelFormat::Bitmask if mode.pixel_bitmask().is_some() => {
            let mask = mode.pixel_bitmask().unwrap();
            (
                kernel_info::BootPixelFormat::Bitmask,
                kernel_info::BootPixelMasks {
                    red_mask: mask.red,
                    green_mask: mask.green,
                    blue_mask: mask.blue,
                    alpha_mask: mask.reserved,
                },
            )
        }
        PixelFormat::BltOnly | PixelFormat::Bitmask => {
            uefi::println!("Unsupported pixel format: Bitmask");
            return Err(Status::UNSUPPORTED);
        }
    };

    let mut fb = gop.frame_buffer();
    let framebuffer_ptr = fb.as_mut_ptr();
    let framebuffer_size = fb.size();
    let framebuffer_stride = mode.stride();

    let fb = FramebufferInfo {
        framebuffer_ptr: framebuffer_ptr as u64,
        framebuffer_size: framebuffer_size as u64,
        framebuffer_width: framebuffer_width as u64,
        framebuffer_height: framebuffer_height as u64,
        framebuffer_stride: framebuffer_stride as u64,
        framebuffer_format,
        framebuffer_masks,
    };

    Ok(fb)
}

/// Fetch the Graphics Output Protocol (GOP).
fn get_gop() -> Result<ScopedProtocol<GraphicsOutput>, uefi::Error> {
    let handle = boot::get_handle_for_protocol::<GraphicsOutput>().map_err(|e| {
        uefi::println!("Failed to get GOP handle: {e:?}");
        uefi::Error::new(Status::ABORTED, ())
    })?;

    let gop = boot::open_protocol_exclusive::<GraphicsOutput>(handle).map_err(|e| {
        uefi::println!("Failed to open GOP exclusively: {e:?}");
        uefi::Error::new(Status::ABORTED, ())
    })?;
    Ok(gop)
}
