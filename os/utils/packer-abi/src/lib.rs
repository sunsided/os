#![no_std]

#[cfg(feature = "unbundle")]
pub mod unbundle;

/// Magic signature identifying a valid bundle file.
///
/// The ASCII bytes correspond to `"INIT_BUN"` (in little-endian)  — short for **Init Bundle**.
/// This marker allows the kernel to quickly verify that the blob follows the
/// expected layout before attempting to parse offsets or load files.
pub const BUNDLE_MAGIC: u64 = 0x4E55_425F_5449_4E49; // "NUB_TINI"

/// Fixed-size header describing the layout of a multi-file init bundle.
///
/// All offsets are **absolute byte offsets** within the bundle blob and are
/// guaranteed to be 8-byte aligned.
/// The header is followed by:
/// 1. An array of [`Entry`] structs (`count` elements)
/// 2. A concatenated, NUL-terminated name blob
/// 3. The file-data blob
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Header {
    /// Constant [`BUNDLE_MAGIC`] value identifying the file as a valid bundle.
    pub magic: u64,

    /// Number of [`Entry`] records in the table.
    pub version: u32,

    /// Number of [`Entry`] records in the table.
    pub count: u32,

    /// Reserved for future use (alignment or versioning); must be zero.
    pub reserved: u64,

    /// Absolute offset, in bytes, from the start of the bundle to the name blob.
    ///
    /// The name blob contains UTF-8, NUL-terminated file names concatenated
    /// back-to-back and padded to an 8-byte boundary.
    pub names_off: u64,

    /// Absolute offset, in bytes, to the start of the file-data blob.
    ///
    /// Each [`Entry::file_off`] is relative to this base.
    pub files_off: u64,

    /// Absolute offset, in bytes, to the first [`Entry`] in the table.
    pub entries_off: u64,
}

/// Table entry describing one file within the bundle.
///
/// Each entry refers into both the name blob and the file-data blob using
/// **relative offsets** from the bases given in [`Header`].
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entry {
    /// Offset, relative to [`Header::names_off`], of this file’s UTF-8 name.
    ///
    /// The string is NUL-terminated within the names blob.
    pub name_off: u64,

    /// Offset, relative to [`Header::files_off`], of this file’s byte contents.
    pub file_off: u64,

    /// Length of the file data in bytes.
    pub file_len: u64,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            magic: BUNDLE_MAGIC,
            version: 0,
            count: 0,
            reserved: 0,
            names_off: 0,
            files_off: 0,
            entries_off: 0,
        }
    }
}
