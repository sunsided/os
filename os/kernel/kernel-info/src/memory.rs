//! # Memory Layout

/// Where the kernel executes (VMA), matches your linker script.
///
/// # Kernel Build
/// This information is sourced in the kernel's `build.rs` to configure
/// the linker.
pub const KERNEL_BASE: u64 = 0xffff_ffff_8000_0000;

/// Where you place the bytes in *physical* memory (LMA) before paging.
///
/// # Kernel Build
/// This information is sourced in the kernel's `build.rs` to configure
/// the linker.
pub const PHYS_LOAD: u64 = 0x0010_0000; // 1 MiB

/// A simple Higher Half Direct Map (HHDM) base.
/// Anything you map at [`HHDM_BASE`] + `pa` lets the kernel
/// access physical memory via a fixed offset.
pub const HHDM_BASE: u64 = 0xffff_8880_0000_0000;

/// Keep a tiny identity map so the paging switch code remains executable
/// right after CR3 reload (and to let you pass low pointers if you want).
pub const IDENTITY_LOW_BYTES: u64 = 0x20_0000; // 2 MiB
