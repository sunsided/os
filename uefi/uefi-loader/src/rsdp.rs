//! # Root/Extended System Description Pointer

use uefi::prelude::*;
use uefi::table::cfg::{ACPI_GUID, ACPI2_GUID};

/// Returns the physical address of the RSDP if present, else 0.
pub fn find_rsdp_addr() -> u64 {
    system::with_config_table(|table| {
        // Prefer ACPI 2.0 RSDP if available
        table
            .iter()
            .find(|entry| entry.guid == ACPI2_GUID)
            .map_or_else(
                || {
                    // Find ACPI 1.0 RSDP
                    table
                        .iter()
                        .find(|entry| entry.guid == ACPI_GUID)
                        .map_or(0, |entry| entry.address as usize as u64)
                },
                |entry| entry.address as usize as u64,
            )
    })
}

#[repr(C, packed)]
struct RsdpV1 {
    signature: [u8; 8], // "RSD PTR "
    checksum: u8,       // sum of first 20 bytes == 0
    oem_id: [u8; 6],
    revision: u8, // 0 for ACPI 1.0
    rsdt_addr: u32,
}

#[repr(C, packed)]
struct RsdpV2 {
    signature: [u8; 8], // "RSD PTR "
    checksum: u8,       // sum of first 20 bytes == 0
    oem_id: [u8; 6],
    revision: u8, // 2 for ACPI 2.0
    _deprecated: u32,
    length: u32,
    xsdt_addr: u64,
    ext_checksum: u8, // checksum of entire table
    reserved: [u8; 3],
}

fn sum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, &b| a.wrapping_add(b))
}

pub unsafe fn validate_rsdp(rsdp_addr: u64) -> bool {
    if rsdp_addr == 0 {
        return false;
    }

    let rsdp = rsdp_addr as *const RsdpV1;
    // Check signature
    if unsafe { core::slice::from_raw_parts((*rsdp).signature.as_ptr(), 8) != b"RSD PTR " } {
        return false;
    }
    // ACPI 1.0 checksum (first 20 bytes)
    let v1_bytes = unsafe { core::slice::from_raw_parts(rsdp.cast::<u8>(), 20) };
    if sum(v1_bytes) != 0 {
        return false;
    }

    // If revision >= 2, validate extended checksum
    let rev = unsafe { (*rsdp).revision };
    if rev >= 2 {
        let v2 = rsdp.cast::<RsdpV2>();
        let len = unsafe { (*v2).length as usize };
        if len < size_of::<RsdpV2>() {
            return false;
        }
        let all = unsafe { core::slice::from_raw_parts(v2.cast::<u8>(), len) };
        if sum(all) != 0 {
            return false;
        }
    }
    true
}
