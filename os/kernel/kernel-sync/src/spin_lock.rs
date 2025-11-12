use core::{
    cell::UnsafeCell,
    hint::spin_loop,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

/// A simple spinlock providing mutual exclusion for a single value.
///
/// `SpinLock` protects access to an inner value using a busy-wait
/// (TATAS — *test-and-test-and-set*) loop.
/// Threads attempting to acquire the lock will repeatedly test its
/// state until it becomes free.
///
/// This lock is **not fair** and **does not block the thread** — it spins
/// until it succeeds. It’s designed for **short, low-contention** critical
/// sections, such as synchronizing access to shared data between CPU cores.
///
/// # Examples
///
/// ```
/// use kernel_sync::SpinLock;
///
/// let lock = SpinLock::new(0);
///
/// {
///     let mut guard = lock.lock();
///     *guard += 1;
/// } // guard drops here, unlocking
///
/// assert_eq!(*lock.lock(), 1);
/// ```
///
/// # Performance
///
/// This lock uses a *test-and-test-and-set* strategy:
/// - It first checks if the lock looks free using a relaxed load.
/// - If contended, it spins with `spin_loop()` hints.
/// - Once free, it retries a `compare_exchange` to acquire.
///
/// This approach reduces memory contention under light load while keeping
/// latency low for uncontended paths.
///
/// # Safety
///
/// The type is `Sync` only if `T: Send`, meaning the protected data can
/// safely cross threads but access remains mutually exclusive.
///
/// # Panics
///
/// The lock itself never panics. The user-provided closure in
/// [`with_lock`](Self::with_lock) may panic.
pub struct SpinLock<T> {
    /// Lock state:
    /// - `false`: unlocked
    /// - `true`: locked
    locked: AtomicBool,
    /// The protected value.
    inner: UnsafeCell<T>,
}

// Safety: only one thread may hold the lock at a time; `T` must be `Send`.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Creates a new `SpinLock` containing the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SpinLock;
    ///
    /// let lock = SpinLock::new(5);
    /// assert_eq!(*lock.lock(), 5);
    /// ```
    #[must_use]
    pub const fn new(inner: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(inner),
        }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// Returns `Some(guard)` if successful, or `None` if the lock
    /// is already held.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SpinLock;
    ///
    /// let lock = SpinLock::new(1);
    /// if let Some(mut g) = lock.try_lock() {
    ///     *g = 2;
    /// }
    /// ```
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

    /// Acquires the lock, spinning until it becomes available.
    ///
    /// This method uses a *test-and-test-and-set* (TATAS) loop:
    /// it first checks the flag with a relaxed read and only performs
    /// an atomic exchange when the lock looks free.
    ///
    /// # Blocking
    ///
    /// This function **never sleeps** or **yields**; it busy-waits until the
    /// lock is released. For long critical sections, prefer a blocking mutex.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SpinLock;
    ///
    /// let lock = SpinLock::new(String::new());
    /// {
    ///     let mut g = lock.lock();
    ///     g.push_str("hello");
    /// }
    /// assert_eq!(&*lock.lock(), "hello");
    /// ```
    #[inline]
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        // Fast path: take the lock if it looks free.
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Contended path: spin on read, then retry CAS.
            while self.locked.load(Ordering::Relaxed) {
                spin_loop();
            }
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

    /// Executes a closure with exclusive access to the inner value.
    ///
    /// This is equivalent to calling [`lock`](Self::lock),
    /// running the closure, and dropping the guard.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SpinLock;
    ///
    /// let lock = SpinLock::new(0);
    ///
    /// let value = lock.with_lock(|v| {
    ///     *v += 1;
    ///     *v
    /// });
    ///
    /// assert_eq!(value, 1);
    /// ```
    #[inline]
    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut g = self.lock();
        f(&mut g)
    }

    /// Returns a mutable reference to the inner value.
    ///
    /// Because you hold `&mut self`, no other thread can access
    /// the data, so locking is unnecessary.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SpinLock;
    ///
    /// let mut lock = SpinLock::new(10);
    /// *lock.get_mut() += 5;
    /// assert_eq!(*lock.lock(), 15);
    /// ```
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

/// A guard that releases a [`SpinLock`] when dropped.
///
/// Created by [`SpinLock::lock`] or [`SpinLock::try_lock`].
/// Implements [`Deref`] and [`DerefMut`] so you can access the
/// protected data directly.
///
/// # Examples
///
/// ```
/// use kernel_sync::SpinLock;
///
/// let lock = SpinLock::new(1);
/// {
///     let mut guard = lock.lock();
///     *guard += 1;
/// }
/// assert_eq!(*lock.lock(), 2);
/// ```
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
