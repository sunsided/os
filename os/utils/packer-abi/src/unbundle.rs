use crate::{BUNDLE_MAGIC, Entry, Header};

/// Parsed bundle view over an in-memory blob.
pub struct Bundle<'a> {
    blob: &'a [u8],
    hdr: Header,
}

/// Iterator over (name, bytes) pairs; yields Result per entry.
pub struct Entries<'a> {
    b: &'a Bundle<'a>,
    idx: usize,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BundleError {
    TooShort,
    BadMagic,
    BadAlignment,
    OutOfBounds,
    Utf8,
}

#[inline]
fn is_aligned8(x: usize) -> bool {
    (x & 7) == 0
}

#[inline]
fn read_u32_le(buf: &[u8], off: usize) -> Result<u32, BundleError> {
    let end = off.checked_add(4).ok_or(BundleError::OutOfBounds)?;
    let s = buf.get(off..end).ok_or(BundleError::OutOfBounds)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

#[inline]
fn read_u64_le(buf: &[u8], off: usize) -> Result<u64, BundleError> {
    let end = off.checked_add(8).ok_or(BundleError::OutOfBounds)?;
    let s = buf.get(off..end).ok_or(BundleError::OutOfBounds)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

impl<'a> Bundle<'a> {
    /// Parse and validate a bundle blob.
    pub fn parse(blob: &'a [u8]) -> Result<Self, BundleError> {
        use BundleError::*;
        // Need at least a Header.
        if blob.len() < size_of::<Header>() {
            return Err(TooShort);
        }

        // Read header fields (LE).
        let magic = read_u64_le(blob, 0)?;
        if magic != BUNDLE_MAGIC {
            return Err(BadMagic);
        }

        let count = read_u32_le(blob, 8)?;
        let _resv = read_u32_le(blob, 12)?; // must be zero in your writer; ignore here
        let names_off = read_u64_le(blob, 16)? as usize;
        let files_off = read_u64_le(blob, 24)? as usize;
        let entries_off = read_u64_le(blob, 32)? as usize;

        // Alignment constraints (all sections 8-byte aligned).
        if !is_aligned8(names_off) || !is_aligned8(files_off) || !is_aligned8(entries_off) {
            return Err(BadAlignment);
        }

        // Ensure entries table fits.
        let ents_len = (count as usize)
            .checked_mul(size_of::<Entry>())
            .ok_or(OutOfBounds)?;
        let ents_end = entries_off.checked_add(ents_len).ok_or(OutOfBounds)?;
        if ents_end > blob.len() {
            return Err(OutOfBounds);
        }

        Ok(Bundle {
            blob,
            hdr: Header {
                count,
                names_off: names_off as u64,
                files_off: files_off as u64,
                entries_off: entries_off as u64,
                ..Header::default()
            },
        })
    }

    /// Number of files in the bundle.
    pub fn len(&self) -> usize {
        self.hdr.count as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over entries by index (0..count).
    pub fn entries(&self) -> Entries<'_> {
        Entries { b: self, idx: 0 }
    }

    /// Fetch the (name, bytes) pair for entry `i`.
    pub fn get(&self, i: usize) -> Result<(&'a str, &'a [u8]), BundleError> {
        use BundleError::*;
        if i >= self.len() {
            return Err(OutOfBounds);
        }
        let base = self.hdr.entries_off as usize;
        let off = base + i * size_of::<Entry>();

        // Entry fields (LE) read directly; we don't rely on target endianness/layout.
        let name_off_rel = read_u64_le(self.blob, off + 0)? as usize;
        let file_off_rel = read_u64_le(self.blob, off + 8)? as usize;
        let file_len = read_u64_le(self.blob, off + 16)? as usize;

        // Name: within names blob, NUL-terminated.
        let name_start = (self.hdr.names_off as usize)
            .checked_add(name_off_rel)
            .ok_or(OutOfBounds)?;

        let mut p = name_start;
        while p < self.blob.len() {
            if self.blob[p] == 0 {
                break;
            }
            p += 1;
        }

        if p >= self.blob.len() {
            return Err(OutOfBounds);
        }

        let name_bytes = &self.blob[name_start..p];
        let name = core::str::from_utf8(name_bytes).map_err(|_| Utf8)?;

        // File slice from files blob.
        let file_start = (self.hdr.files_off as usize)
            .checked_add(file_off_rel)
            .ok_or(OutOfBounds)?;
        let file_end = file_start.checked_add(file_len).ok_or(OutOfBounds)?;
        let bytes = self.blob.get(file_start..file_end).ok_or(OutOfBounds)?;

        Ok((name, bytes))
    }

    /// Find a file by exact name.
    pub fn find(&'a self, needle: &str) -> Option<&'a [u8]> {
        for (name, bytes) in self.entries().flatten() {
            if name == needle {
                return Some(bytes);
            }
        }
        None
    }

    /// Return the first file (name, bytes), if any.
    pub fn first(&self) -> Option<(&'a str, &'a [u8])> {
        self.get(0).ok()
    }
}

impl<'a> Iterator for Entries<'a> {
    type Item = Result<(&'a str, &'a [u8]), BundleError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.b.len() {
            return None;
        }
        let i = self.idx;
        self.idx += 1;
        Some(self.b.get(i))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let r = self.b.len().saturating_sub(self.idx);
        (r, Some(r))
    }
}

impl<'a> core::iter::FusedIterator for Entries<'a> {}
