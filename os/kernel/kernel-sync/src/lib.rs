//! # `kernel_sync` — kernel-friendly synchronization primitives
//!
//! Minimal `no_std` locks and cells designed for kernels and bare-metal systems.
//! Primitives aim to be small, predictable, and explicit about safety and
//! memory-ordering. No poisoning, no allocation, and no OS dependencies.
//!
//! ## Modules & types
//! - [`SpinLock`]/[`SpinLockGuard`]: TATAS spinlock for a single value.
//! - [`RawSpin`], [`RawTicket`]: raw, low-level lock primitives.
//! - [`Mutex<T, R>`]/[`MutexGuard`]: generic RAII mutex over any raw lock `R`.
//! - [`SpinMutex<T>`], [`TicketMutex<T>`]: convenient mutex aliases.
//! - [`IrqGuard`], [`IrqMutex`]: scope-based interrupt disable + mutex guard
//!   (`x86/x86_64`, privileged mode).
//! - [`SyncOnceCell<T>`]: single-writer, multi-reader, spin-based once-cell.
//!
//! ## Concurrency model
//! These primitives rely on acquire/release atomics and CPU-local spinning.
//! They are intended for **short** critical sections under **low contention**.
//! On SMP, some operations (e.g., TLB invalidation) act on the **local** CPU;
//! coordinate cross-CPU work as needed.
//!
//! ## Safety
//! Many APIs are safe to *call* but have **correctness requirements** (e.g.,
//! only use `invlpg` for the current address space; interrupts must be legal
//! to disable in your context). Unsafe methods clearly document their preconditions.
//!
//! ## `no_std`
//! This crate is `no_std` by default and uses inline assembly on `x86/x86_64`
//! for interrupt control where applicable.

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code)]

pub mod irq;
mod mutex;
mod raw_spin;
mod raw_ticket;
mod spin_lock;
mod sync_once_cell;

pub use irq::{IrqGuard, IrqMutex};
pub use mutex::{Mutex, MutexGuard};
pub use raw_spin::RawSpin;
pub use raw_ticket::RawTicket;
pub use spin_lock::{SpinLock, SpinLockGuard};
pub use sync_once_cell::SyncOnceCell;

/// A `Mutex` backed by a [`RawSpin`] lock.
///
/// Fairness: none. Prefer under light contention.
pub type SpinMutex<T> = Mutex<T, RawSpin>;

/// A `Mutex` backed by a [`RawTicket`] lock.
///
/// Fairness: FIFO. Slightly higher latency, avoids starvation.
pub type TicketMutex<T> = Mutex<T, RawTicket>;

impl<T> SpinMutex<T> {
    /// Creates a new [`SpinMutex`] containing `value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SpinMutex;
    /// let m = SpinMutex::new(0u32);
    /// *m.lock() = 1;
    /// ```
    pub const fn new(value: T) -> Self {
        Self::from_raw(RawSpin::new(), value)
    }
}

impl<T> TicketMutex<T> {
    /// Creates a new [`TicketMutex`] containing `value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::TicketMutex;
    /// let m = TicketMutex::new(0u32);
    /// *m.lock() = 1;
    /// ```
    pub const fn new(value: T) -> Self {
        Self::from_raw(RawTicket::new(), value)
    }
}

/// A low-level lock interface used by [`Mutex`].
///
/// Implementors provide the raw operations; higher-level types (like [`Mutex`])
/// add RAII and safe access to protected data.
///
/// ### Semantics
/// - `raw_lock` must provide mutual exclusion and publish the critical section
///   with **acquire** semantics for the caller.
/// - `raw_try_lock` attempts to acquire without blocking.
///
/// Implementations may spin, sleep, or otherwise block, depending on context.
pub trait RawLock {
    /// Acquires the lock (may spin or block) and returns when the caller holds it.
    fn raw_lock(&self);

    /// Attempts to acquire the lock without blocking.
    ///
    /// Returns `true` on success, `false` if the lock is currently held.
    fn raw_try_lock(&self) -> bool;
}

/// A companion trait that releases a raw lock.
///
/// # Safety
/// The caller must ensure the current execution context **holds** the lock
/// being unlocked. Violating this precondition can corrupt memory or break
/// invariants.
///
/// Implementations must:
/// - Use **release** semantics to publish writes from the critical section.
/// - Leave the lock in a state that another `raw_lock`/`raw_try_lock` can acquire.
pub trait RawUnlock {
    /// Releases the lock previously acquired by this context.
    ///
    /// # Safety
    /// Caller must currently hold the lock.
    unsafe fn raw_unlock(&self);
}
