//! # GOP for the Kernel

use crate::{trace, trace_usize};
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

    let count = iter_modes(&gop).iter().count();
    trace("Found ");
    trace_usize(count);
    trace(" GOP modes\n");

    let mode = gop.current_mode_info();
    let (framebuffer_width, framebuffer_height) = mode.resolution();
    trace("Current mode ");
    trace_usize(framebuffer_width);
    trace(" x ");
    trace_usize(framebuffer_height);
    trace(" px\n");

    // Prefer 1080p over others; if none found, pick the largest one.
    if let Some(mode) = iter_modes(&gop).iter().find(|mode| {
        let (_w, h) = mode.info().resolution();
        h == 1080
    }) {
        if let Err(err) = gop.set_mode(mode) {
            uefi::println!("Failed to set GOP mode: {err:?}");
            return Err(Status::UNSUPPORTED);
        }
    } else if let Some(mode) = iter_modes(&gop).iter().next() {
        if let Err(err) = gop.set_mode(mode) {
            uefi::println!("Failed to set GOP mode: {err:?}");
            return Err(Status::UNSUPPORTED);
        }
    } else {
        uefi::println!("No suitable GOP graphics mode found.");
        return Err(Status::UNSUPPORTED);
    }

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

/// Iterate modes by resolution.
fn iter_modes(gop: &ScopedProtocol<GraphicsOutput>) -> Option<Mode> {
    gop.modes()
        .filter(|mode| {
            mode.info().pixel_format() == PixelFormat::Bgr
                || mode.info().pixel_format() == PixelFormat::Rgb
        })
        .max_by_key(|mode| {
            let (w, h) = mode.info().resolution();
            w * h
        })
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
