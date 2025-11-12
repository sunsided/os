use crate::{RawLock, RawUnlock};
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

/// A simple spinlock implementation based on an atomic flag.
///
/// `RawSpin` provides a minimal, low-level synchronization primitive that
/// busy-waits until the lock becomes available. It’s useful for short
/// critical sections where blocking or sleeping would be more expensive
/// than spinning.
///
/// This type is not fair and does not provide reentrancy.
/// Spinning threads continuously check the lock state, consuming CPU
/// cycles until it becomes free.
///
/// # Examples
///
/// ```
/// use kernel_sync::RawSpin;
///
/// let lock = RawSpin::new();
///
/// lock.lock();
/// // critical section
/// unsafe { lock.unlock(); }
/// ```
pub struct RawSpin {
    /// Indicates whether the lock is currently held.
    held: AtomicBool,
}

impl Default for RawSpin {
    fn default() -> Self {
        Self::new()
    }
}

impl RawSpin {
    /// Creates a new unlocked `RawSpin`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::RawSpin;
    ///
    /// let lock = RawSpin::new();
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
        }
    }

    /// Acquires the lock, spinning until it becomes available.
    ///
    /// This method repeatedly checks and sets the internal flag until the
    /// lock can be acquired. While spinning, it uses [`core::hint::spin_loop`]
    /// to hint to the processor that the thread is in a busy-wait loop.
    ///
    /// # Blocking
    ///
    /// This function never yields or blocks the current thread — it spins.
    /// Prefer this only for very short critical sections.
    #[inline]
    pub fn lock(&self) {
        // Fast path: try once, then spin with backoff
        while self.held.swap(true, Ordering::Acquire) {
            while self.held.load(Ordering::Relaxed) {
                spin_loop();
            }
        }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// Returns `true` if the lock was successfully acquired, or `false`
    /// if it was already held.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::RawSpin;
    ///
    /// let lock = RawSpin::new();
    /// assert!(lock.try_lock());
    /// assert!(!lock.try_lock());
    /// unsafe { lock.unlock(); }
    /// ```
    #[inline]
    pub fn try_lock(&self) -> bool {
        !self.held.swap(true, Ordering::Acquire)
    }

    /// Releases the lock.
    ///
    /// # Safety
    ///
    /// This method must only be called if the current thread holds the lock.
    /// Calling it without a prior successful call to [`lock`](Self::lock)
    /// or [`try_lock`](Self::try_lock) leads to undefined behavior.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::RawSpin;
    ///
    /// let lock = RawSpin::new();
    /// lock.lock();
    /// unsafe { lock.unlock(); }
    /// ```
    #[inline]
    pub unsafe fn unlock(&self) {
        self.held.store(false, Ordering::Release);
    }
}

impl RawLock for RawSpin {
    #[inline]
    fn raw_lock(&self) {
        self.lock();
    }

    #[inline]
    fn raw_try_lock(&self) -> bool {
        self.try_lock()
    }
}

impl RawUnlock for RawSpin {
    #[inline]
    unsafe fn raw_unlock(&self) {
        unsafe { self.unlock() }
    }
}
