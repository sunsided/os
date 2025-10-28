//! # Virtual Memory Support
//!
//! Minimal x86-64 paging helpers for a hobby OS loader/kernel.
//!
//! ## What you get
//! - An [`address space`](address_space) describing a `PML4` root page table.
//! - Tiny [`PhysAddr`]/[`VirtAddr`] newtypes (u64) to avoid mixing address kinds.
//! - A [`PageSize`] enum for 4 KiB / 2 MiB / 1 GiB mappings.
//! - x86-64 page-table [`MemoryPageFlags`] with practical explanations.
//! - A 4 KiB-aligned [`PageTable`] wrapper and index helpers.
//! - A tiny allocator/mapper interface ([`FrameAlloc`], [`PhysMapper`]).
//!
//! ## x86-64 Virtual Address → Physical Address Walk
//!
//! Each 48-bit virtual address is divided into five fields:
//!
//! ```text
//! | 47‒39 | 38‒30 | 29‒21 | 20‒12 | 11‒0   |
//! |  PML4 |  PDPT |   PD  |   PT  | Offset |
//! ```
//!
//! The CPU uses these fields as **indices** into four levels of page tables,
//! each level containing 512 (2⁹) entries of 8 bytes (64 bits) each.
//!
//! ```text
//!  PML4  →  PDPT  →  PD  →  PT  →  Physical Page
//!   │        │        │        │
//!   │        │        │        └───► PTE   (Page Table Entry)  → maps 4 KiB page
//!   │        │        └────────────► PDE   (Page Directory Entry) → PS=1 → 2 MiB page
//!   │        └─────────────────────► PDPTE (Page Directory Pointer Table Entry) → PS=1 → 1 GiB page
//!   └──────────────────────────────► PML4E (Page Map Level 4 Entry)
//! ```
//!
//! ### Levels and their roles
//!
//! | Level | Table name | Entry name | Description |
//! |:------|:------------|:-----------|:-------------|
//! | 1 | **PML4** (Page Map Level 4) | **PML4E** | Top-level table; each entry points to a PDPT. One PML4 table per address space, referenced by Control Register 3 ([`CR3`](https://wiki.osdev.org/CPU_Registers_x86#CR3)). |
//! | 2 | **PDPT** (Page Directory Pointer Table) | **PDPTE** | Each entry points to a PD. If `PS=1`, it directly maps a 1 GiB page (leaf). |
//! | 3 | **PD** (Page Directory) | **PDE** | Each entry points to a PT. If `PS=1`, it directly maps a 2 MiB page (leaf). |
//! | 4 | **PT** (Page Table) | **PTE** | Each entry maps a 4 KiB physical page (always a leaf). |
//!
//! ### Leaf vs. non-leaf entries
//!
//! - A **leaf entry** directly maps physical memory — it contains the physical base address
//!   and the permission bits ([`PRESENT`](PageTableEntry::present), [`WRITABLE`](PageTableEntry::writable), [`USER`](PageTableEntry::user), [`GLOBAL`](PageTableEntry::global), [`NX`](PageTableEntry::nx), etc.).
//!   - A **PTE** is always a leaf (maps 4 KiB).
//!   - A **PDE** with `PS=1` is a leaf (maps 2 MiB).
//!   - A **PDPTE** with `PS=1` is a leaf (maps 1 GiB).
//!
//! - A **non-leaf entry** points to the next lower table level and continues the walk.
//!   For example, a PML4E points to a PDPT, and a PDE with `PS=0` points to a PT.
//!
//! ### Offset
//!
//! - The final **Offset** field (bits 11–0) selects the byte inside the 4 KiB (or larger) page.
//!
//! ### Summary
//!
//! A canonical 48-bit virtual address is effectively:
//!
//! ```text
//! VA = [PML4:9] [PDPT:9] [PD:9] [PT:9] [Offset:12]
//! ```
//!
//! This creates a four-level translation tree that can map up to **256 TiB** of
//! virtual address space, using leaf pages of 1 GiB, 2 MiB, or 4 KiB depending
//! on which level the translation stops.

#![cfg_attr(not(test), no_std)]
#![allow(unsafe_code, clippy::inline_always)]

pub mod addr2;
pub mod address_space;
mod page_entry_bits;
pub mod table2;

pub use crate::address_space::AddressSpace;
pub use crate::page_entry_bits::PageEntryBits;

use crate::addr2::PhysicalAddress;
/// Re-export constants as info module.
pub use kernel_info::memory as info;

/// Reads the current value of the **CR3 register** (the page table base register)
/// and returns the physical address of the top-level page table (PML4).
///
/// # Safety
/// This function is **unsafe** because it directly accesses a CPU control register.
/// It must only be called in privileged (ring 0) code where paging is active and
/// the CR3 contents are valid. Calling it from user mode or before enabling paging
/// will cause undefined behavior.
///
/// # Details
/// - On x86-64, CR3 holds the **physical base address** of the currently active
///   PML4 (Page Map Level 4) table.
/// - The low 12 bits of CR3 contain **flags** (e.g., PCID, reserved bits),
///   so this function masks them out to obtain a 4 KiB-aligned physical address.
/// - The returned address represents the root of the current virtual memory
///   hierarchy used for address translation.
///
/// # Returns
/// The 4 KiB-aligned [`PhysAddr`] of the current PML4 table.
///
/// # Example
/// ```no_run
/// use kernel_vmem::read_cr3_phys;
///
/// // SAFETY: Only call from kernel mode with paging enabled.
/// let current_pml4 = unsafe { read_cr3_phys() };
/// println!("Active PML4 base: {:#x}", current_pml4.as_u64());
/// ```
#[allow(clippy::inline_always)]
#[inline(always)]
#[must_use]
pub unsafe fn read_cr3_phys() -> PhysicalAddress {
    let mut cr3: u64;
    unsafe {
        core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
    }

    // CR3 holds PML4 physical base with low bits as flags
    PhysicalAddress::from(cr3 & 0x000f_ffff_ffff_f000)
}
