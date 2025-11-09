use core::{
    cell::UnsafeCell,
    hint::spin_loop,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

pub struct SpinLock<T> {
    /// lock state
    /// * `false`: unlocked
    /// * `true`: locked
    locked: AtomicBool,
    inner: UnsafeCell<T>,
}

// Safety: mutual exclusion; only T: Send may cross threads.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(inner),
        }
    }

    /// Try once; returns immediately.
    #[inline]
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }

    /// Spin until acquired (TATAS), then return a guard.
    #[inline]
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        // Fast path: take the lock if it looks free.
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Contended path: spin on a read (cheap), then retry CAS.
            while self.locked.load(Ordering::Relaxed) {
                spin_loop();
            }
            // try to take it when it flips to false
            while self
                .locked
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                while self.locked.load(Ordering::Relaxed) {
                    spin_loop();
                }
            }
        }
        SpinLockGuard { lock: self }
    }

    /// Closure convenience, built on the guard.
    #[inline]
    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut g = self.lock();
        f(&mut g)
    }

    /// Mutable access when you have `&mut self` (no contention possible).
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.inner.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.inner.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        // Release publishes the critical section.
        self.lock.locked.store(false, Ordering::Release);
    }
}
