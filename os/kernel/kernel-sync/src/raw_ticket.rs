use crate::{RawLock, RawUnlock};
use core::hint::spin_loop;
use core::sync::atomic::{AtomicUsize, Ordering};

/// A simple *ticket lock* implementation using atomics.
///
/// `RawTicket` provides a fair spinlock where threads acquire the lock
/// in the order they requested it. Each thread receives a ticket number,
/// and spins until its number matches the current owner.
///
/// Compared to [`RawSpin`](crate::RawSpin), this lock ensures fairness:
/// threads cannot starve as tickets are served strictly in order.
///
/// This lock is intended for very short critical sections where spinning
/// is cheaper than sleeping. It is not reentrant and does not support
/// poisoning or ownership tracking.
///
/// # Examples
///
/// ```
/// use kernel_sync::RawTicket;
///
/// let lock = RawTicket::new();
///
/// lock.lock();
/// // critical section
/// unsafe { lock.unlock(); }
/// ```
pub struct RawTicket {
    /// The next available ticket number.
    next: AtomicUsize,
    /// The ticket number currently being served.
    owner: AtomicUsize,
}

impl Default for RawTicket {
    fn default() -> Self {
        Self::new()
    }
}

impl RawTicket {
    /// Creates a new, unlocked ticket lock.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::RawTicket;
    ///
    /// let lock = RawTicket::new();
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: AtomicUsize::new(0),
            owner: AtomicUsize::new(0),
        }
    }

    /// Acquires the lock, spinning in FIFO order until available.
    ///
    /// Each caller obtains a ticket number and waits until that ticket
    /// matches the current owner. This guarantees first-come, first-served
    /// fairness among contending threads.
    ///
    /// While waiting, this function uses [`core::hint::spin_loop`]
    /// to signal a busy-wait loop to the processor.
    ///
    /// # Blocking
    ///
    /// This method never sleeps or yields; it spins until the lock is free.
    #[inline]
    pub fn lock(&self) {
        let ticket = self.next.fetch_add(1, Ordering::Relaxed);
        // Acquire when we observe our turn
        while self.owner.load(Ordering::Acquire) != ticket {
            spin_loop();
        }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// Returns `true` if the lock was successfully acquired, or `false`
    /// if another thread currently holds it.
    ///
    /// This method performs a fair check based on ticket order.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::RawTicket;
    ///
    /// let lock = RawTicket::new();
    /// assert!(lock.try_lock());
    /// assert!(!lock.try_lock());
    /// unsafe { lock.unlock(); }
    /// ```
    #[inline]
    pub fn try_lock(&self) -> bool {
        let owner = self.owner.load(Ordering::Relaxed);
        let next = self.next.load(Ordering::Relaxed);
        if next == owner {
            // attempt to claim the next ticket
            self.next
                .compare_exchange(next, next + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        } else {
            false
        }
    }

    /// Releases the lock and advances ownership to the next ticket.
    ///
    /// # Safety
    ///
    /// This method must only be called if the current thread holds the lock.
    /// Calling it without a matching successful [`lock`](Self::lock) or
    /// [`try_lock`](Self::try_lock) leads to undefined behavior.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::RawTicket;
    ///
    /// let lock = RawTicket::new();
    /// lock.lock();
    /// unsafe { lock.unlock(); }
    /// ```
    #[inline]
    pub unsafe fn unlock(&self) {
        // Release when we advance owner
        let t = self.owner.load(Ordering::Relaxed);
        self.owner.store(t + 1, Ordering::Release);
    }
}

impl RawLock for RawTicket {
    #[inline]
    fn raw_lock(&self) {
        self.lock();
    }

    #[inline]
    fn raw_try_lock(&self) -> bool {
        self.try_lock()
    }
}

impl RawUnlock for RawTicket {
    #[inline]
    unsafe fn raw_unlock(&self) {
        unsafe { self.unlock() }
    }
}
