//! # Memory Layout

/// End of userspace VA range after which Kernel space begins.
pub const LAST_USERSPACE_ADDRESS: u64 = 0xffff_0000_0000_0000;

/// End of userspace VA range after which Kernel space begins.
pub const USERSPACE_END: u64 = 0xffff_0000_0000_0000;

/// A simple Higher Half Direct Map (HHDM) base.
/// Anything you map at [`HHDM_BASE`] + `pa` lets the kernel
/// access physical memory via a fixed offset.
pub const HHDM_BASE: u64 = 0xffff_8880_0000_0000;

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

/// Keep a tiny identity map so the paging switch code remains executable
/// right after CR3 reload (and to let you pass low pointers if you want).
pub const IDENTITY_LOW_BYTES: u64 = 0x20_0000; // 2 MiB

/// The size of the kernel stack in debug builds.
#[cfg(debug_assertions)]
pub const KERNEL_STACK_SIZE: usize = 32 * 1024;

/// The size of the kernel stack in release builds.
#[cfg(not(debug_assertions))]
pub const KERNEL_STACK_SIZE: usize = 32 * 1024;

const _: () = {
    assert!(KERNEL_STACK_SIZE.is_multiple_of(4096));
    assert!(HHDM_BASE >= LAST_USERSPACE_ADDRESS);
    assert!(KERNEL_BASE > HHDM_BASE);
};
