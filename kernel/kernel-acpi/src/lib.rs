//! # ACPI

#![no_std]
#![allow(unsafe_code)]

#[derive(Clone)]
pub enum SystemDescriptor {
    Rsdp(Rsdp),
    Xsdp(Xsdp),
}

/// ACPI 1.0 Root System Description Pointer (RSDP)
#[derive(Clone)]
#[repr(C, packed)]
pub struct Rsdp {
    signature: [u8; 8], // "RSD PTR "
    checksum: u8,       // sum of first 20 bytes == 0
    oem_id: [u8; 6],
    revision: u8, // 0 for ACPI 1.0
    rsdt_addr: u32,
}

/// ACPI 2.0 Extended System Description Pointer (XSDP)
#[derive(Clone)]
#[repr(C, packed)]
pub struct Xsdp {
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

/// Validate the RSDP/XSDP from the physical address.
///
/// # Safety
/// This function validates that the provided address is non-zero (i.e., not `null`).
/// It validates the supported ACPI 1.0/2.0 variants by revision and checksum/extended checksum.
#[must_use]
pub unsafe fn validate_rsdp(rsdp_addr: u64) -> Option<SystemDescriptor> {
    if rsdp_addr == 0 {
        return None;
    }

    let rsdp = rsdp_addr as *const Rsdp;
    let rev = unsafe { (*rsdp).revision };

    // If revision == 0 (ACPI), validate regular checksum
    if rev == 0 {
        // Check signature
        if unsafe { core::slice::from_raw_parts((*rsdp).signature.as_ptr(), 8) != b"RSD PTR " } {
            return None;
        }
        // ACPI 1.0 checksum (first 20 bytes)
        let v1_bytes = unsafe { core::slice::from_raw_parts(rsdp.cast::<u8>(), 20) };
        if sum(v1_bytes) != 0 {
            return None;
        }

        return Some(SystemDescriptor::Rsdp(unsafe { (*rsdp).clone() }));
    }

    // If revision >= 2, validate extended checksum
    let rev = unsafe { (*rsdp).revision };
    if rev < 2 {
        return None;
    }

    let v2 = rsdp.cast::<Xsdp>();
    let len = unsafe { (*v2).length as usize };
    if len < size_of::<Xsdp>() {
        return None;
    }
    let all = unsafe { core::slice::from_raw_parts(v2.cast::<u8>(), len) };
    if sum(all) != 0 {
        return None;
    }

    Some(SystemDescriptor::Xsdp(unsafe { (*v2).clone() }))
}
