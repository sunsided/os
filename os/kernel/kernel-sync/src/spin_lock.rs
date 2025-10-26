//! # Spin Lock

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// A tiny spinlock for short critical sections.
///
/// This lock is suitable for **uniprocessor** or early boot stages where:
/// - Preemption is either disabled or non-existent.
/// - Critical sections are very short (no I/O, no blocking).
///
/// # Guarantees
/// - Provides mutual exclusion for access to the protected value.
/// - `Sync` is implemented when `T: Send`, allowing shared references across
///   threads (the lock enforces interior mutability).
///
/// # Caveats
/// - Does **not** disable interrupts.
/// - Busy-waits with `spin_loop`, so keep critical sections small.
pub struct SpinLock<T> {
    /// Lock state (`false` = unlocked, `true` = locked).
    locked: AtomicBool,
    /// The protected value.
    inner: UnsafeCell<T>,
}

// Safety: SpinLock provides mutual exclusion; it can be shared across threads as long as T is Send.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Create a new spinlock wrapping `inner`.
    pub const fn new(inner: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(inner),
        }
    }

    /// Execute `f` with exclusive access to the inner value.
    ///
    /// Spins until the lock is acquired, then releases it after `f` returns.
    ///
    /// # Panics
    /// Never panics by itself; panics in `f` will unwind through the critical section.
    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // Spin until we acquire the lock.
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }

        // SAFETY: We have exclusive access while the lock is held.
        let res = {
            let inner = unsafe { &mut *self.inner.get() };
            f(inner)
        };
        self.locked.store(false, Ordering::Release);
        res
    }
}
