//! # RSDP/XSDP (Root/Extended System Description Pointer)

use crate::{PhysMapRo, sum};

pub struct AcpiRoots {
    pub rsdp_addr: u64,
    pub xsdt_addr: Option<u64>,
    pub rsdt_addr: Option<u64>,
}

/// ACPI 1.0 Root System Description Pointer (RSDP)
#[derive(Clone)]
#[repr(C, packed)]
struct Rsdp {
    pub(crate) signature: [u8; 8], // "RSD PTR "
    checksum: u8,                  // sum of first 20 bytes == 0
    oem_id: [u8; 6],
    pub(crate) revision: u8, // 0 for ACPI 1.0
    pub(crate) rsdt_addr: u32,
}

/// ACPI 2.0 Extended System Description Pointer (XSDP)
#[derive(Clone)]
#[repr(C, packed)]
struct Xsdp {
    signature: [u8; 8], // "RSD PTR "
    checksum: u8,       // sum of first 20 bytes == 0
    oem_id: [u8; 6],
    revision: u8, // 2 for ACPI 2.0
    _deprecated: u32,
    pub(crate) length: u32,
    pub(crate) xsdt_addr: u64,
    ext_checksum: u8, // checksum of entire table
    reserved: [u8; 3],
}

impl AcpiRoots {
    /// Validate the RSDP/XSDP from the physical address.
    ///
    /// # Safety
    /// This function validates that the provided address is non-zero (i.e., not `null`).
    /// It validates the supported ACPI 1.0/2.0 variants by revision and checksum/extended checksum.
    #[must_use]
    #[allow(clippy::similar_names)]
    pub unsafe fn parse(map: &impl PhysMapRo, rsdp_addr: u64) -> Option<Self> {
        if rsdp_addr == 0 {
            return None;
        }

        unsafe {
            let v1 = map.map_ro(rsdp_addr, size_of::<Rsdp>());
            if &v1[0..8] != b"RSD PTR " {
                return None;
            }
            if sum(&v1[0..20]) != 0 {
                return None;
            }

            let v1p = &*v1.as_ptr().cast::<Rsdp>();
            let rsdt_addr = Some(u64::from(v1p.rsdt_addr));

            if v1p.revision >= 2 {
                // Need full v2 to read length + xsdt
                let min_v2 = core::mem::size_of::<Xsdp>();
                let v2 = map.map_ro(rsdp_addr, min_v2);
                let v2p = &*v2.as_ptr().cast::<Xsdp>();
                let len = v2p.length as usize;
                let full = map.map_ro(rsdp_addr, len);
                if sum(full) != 0 {
                    return None;
                }
                return Some(Self {
                    rsdp_addr,
                    xsdt_addr: Some(v2p.xsdt_addr),
                    rsdt_addr,
                });
            }

            Some(Self {
                rsdp_addr,
                xsdt_addr: None,
                rsdt_addr,
            })
        }
    }
}
