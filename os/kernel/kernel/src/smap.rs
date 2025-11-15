//! Temporary SMAP-disabling guard.
//!
//! This module provides [`SmapGuard`], a small RAII helper that lets kernel
//! code *temporarily* access user-memory while SMAP (Supervisor Mode Access
//! Prevention) is enabled.
//!
//! # Overview
//!
//! When CR4.SMAP is set, the CPU blocks supervisor-mode loads and stores to
//! pages marked as user (`U=1`). To perform a safe copy from or to user-space,
//! the kernel must:
//!
//! 1. Set RFLAGS.AC (`stac`) to allow user memory access.
//! 2. Perform the intended reads/writes.
//! 3. Clear RFLAGS.AC (`clac`) immediately afterward.
//!
//! [`SmapGuard`] automates this using RAII. On creation, it executes `stac`.
//! When the guard is dropped, it automatically executes `clac` again.
//!
//! # Example
//!
//! ```no_run
//! use crate::smap::SmapGuard;
//!
//! fn copy_from_user(dst: *mut u8, src: *const u8, len: usize) {
//!     let _guard = SmapGuard::enter();
//!     unsafe {
//!         core::ptr::copy_nonoverlapping(src, dst, len);
//!     }
//! } // AC is cleared automatically here
//! ```
//!
//! # Safety
//!
//! - `SmapGuard` uses inline assembly to modify EFLAGS.AC.
//! - The caller must ensure that:
//!   - User pointers are validated before use.
//!   - No kernel pointer dereferences rely on AC staying set after the guard
//!     ends.
//!   - Code inside the guard never calls into subsystems that expect SMAP
//!     protection to still be active.
//!
//! Misuse can reintroduce the very class of bugs SMAP is meant to prevent.

/// RAII guard that temporarily disables SMAP to allow supervisor code to
/// access user-space memory.
///
/// Creating a guard executes `stac`, which sets RFLAGS.AC and permits
/// supervisor-mode loads/stores to user pages. When the guard is dropped,
/// it executes `clac` to clear RFLAGS.AC and restore SMAP protection.
pub struct SmapGuard;

impl SmapGuard {
    /// Enter a temporary SMAP-disabled region.
    ///
    /// This executes the `stac` instruction, enabling supervisor-mode access
    /// to user-space pages for the lifetime of the returned guard.
    ///
    /// The returned guard **must** be allowed to drop to restore SMAP
    /// correctly. Use a local binding to ensure proper scope.
    #[inline(always)]
    #[allow(clippy::inline_always)]
    #[must_use]
    pub fn enter() -> Self {
        unsafe {
            core::arch::asm!("stac", options(nomem, nostack));
        }
        Self
    }

    /// Restore SMAP protection by executing `clac` when the guard goes out
    /// of scope.
    ///
    /// Clearing the AC flag re-enables the CPU’s protection against accidental
    /// supervisor access to user-space memory.
    #[inline(always)]
    #[allow(dead_code, clippy::inline_always)]
    pub fn exit(self) {
        drop(self);
    }
}

impl Drop for SmapGuard {
    /// Restore SMAP protection by executing `clac` when the guard goes out
    /// of scope.
    ///
    /// Clearing the AC flag re-enables the CPU’s protection against accidental
    /// supervisor access to user-space memory.
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn drop(&mut self) {
        unsafe {
            core::arch::asm!("clac", options(nomem, nostack));
        }
    }
}
