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
