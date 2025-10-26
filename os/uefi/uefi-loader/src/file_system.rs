extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use uefi::proto::media::file::{File, FileAttribute, FileMode, RegularFile};
use uefi::{CStr16, Status, boot};

/// Loads a file from the EFI file system.
///
/// # Error
/// Returns a [`Status`] in case of error.
pub fn load_file(path: &CStr16) -> Result<Vec<u8>, Status> {
    let image_handle = boot::image_handle();
    let mut sfs = match boot::get_image_file_system(image_handle) {
        Ok(fs) => fs,
        Err(e) => {
            uefi::println!("Failed to get file system: {e:?}");
            return Err(Status::UNSUPPORTED);
        }
    };

    let mut volume = match sfs.open_volume() {
        Ok(dir) => dir,
        Err(e) => {
            uefi::println!("Failed to open root directory: {e:?}");
            return Err(Status::UNSUPPORTED);
        }
    };

    let handle = match volume.open(path, FileMode::Read, FileAttribute::empty()) {
        Ok(handle) => handle,
        Err(e) => {
            uefi::println!("Failed to read file: {e:?}");
            return Err(Status::UNSUPPORTED);
        }
    };

    let Some(mut file) = handle.into_regular_file() else {
        uefi::println!("Failed to read file: not a file");
        return Err(Status::UNSUPPORTED);
    };

    // Get file size
    if let Err(e) = file.set_position(RegularFile::END_OF_FILE) {
        uefi::println!("Failed to seek to file end: {e:?}");
        return Err(Status::UNSUPPORTED);
    }

    let size = match file.get_position() {
        Ok(size) => size,
        Err(e) => {
            uefi::println!("Failed to get file size: {e:?}");
            return Err(Status::UNSUPPORTED);
        }
    };

    // Seek back for actual reading
    if let Err(e) = file.set_position(0) {
        uefi::println!("Failed to seek to file start: {e:?}");
        return Err(Status::UNSUPPORTED);
    }

    let Ok(size) = usize::try_from(size) else {
        uefi::println!("Failed to get file size: invalid pointer widths");
        return Err(Status::UNSUPPORTED);
    };

    let mut buf = vec![0u8; size];
    let read = match file.read(&mut buf) {
        Ok(size) => size,
        Err(e) => {
            uefi::println!("Failed to read file contents: {e:?}");
            return Err(Status::UNSUPPORTED);
        }
    };

    if read != size {
        uefi::println!("Mismatch in file size: read {read} bytes, expected {size} bytes");
        return Err(Status::UNSUPPORTED);
    }

    Ok(buf)
}
