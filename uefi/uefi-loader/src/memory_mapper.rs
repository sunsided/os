//! # Physical Memory Mapper

use core::{ptr::NonNull, slice};
use kernel_acpi::PhysMapRo;

pub struct UefiIdentityMapper;

impl PhysMapRo for UefiIdentityMapper {
    /// # Safety
    /// We're just assuming here that this operation is safe.
    unsafe fn map_ro<'a>(&self, paddr: u64, len: usize) -> &'a [u8] {
        let ptr = NonNull::new(paddr as *mut u8)
            .expect("null physical address")
            .as_ptr()
            .cast_const();

        // On x86_64 UEFI, firmware page tables provide a 1:1 mapping pre-ExitBootServices.
        unsafe { slice::from_raw_parts(ptr, len) }
    }
}
