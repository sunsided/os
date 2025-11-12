use crate::{RawLock, RawUnlock};
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

/// A generic mutex that delegates locking to a raw lock implementation.
///
/// `Mutex<T, R>` protects a value of type `T` using a raw lock `R` that
/// implements [`RawLock`] and [`RawUnlock`]. This design lets you pair a
/// convenient, RAII-style guard with different low-level primitives (e.g.
/// a spin lock or a ticket lock) without duplicating code.
///
/// The mutex itself does not track poisoning or ownership; it simply
/// acquires/releases the underlying raw lock and provides access to `T`
/// through a guard that unlocks on drop.
///
/// # Examples
///
/// ```
/// use kernel_sync::{Mutex, RawSpin};
///
/// let m: Mutex<u32, RawSpin> = Mutex::from_raw(RawSpin::new(), 0);
///
/// {
///     let mut g = m.lock();
///     *g = 1;
/// } // guard drops, unlocking
///
/// assert_eq!(*m.lock(), 1);
/// ```
///
/// Using `try_lock`:
///
/// ```
/// use kernel_sync::{Mutex, RawSpin};
///
/// let m: Mutex<&'static str, RawSpin> = Mutex::from_raw(RawSpin::new(), "hi");
/// if let Some(mut g) = m.try_lock() {
///     *g = "hello";
/// }
/// assert!(*m.lock() == "hello" || *m.lock() == "hi");
/// ```
///
/// # Concurrency & Safety
///
/// - The type is `!Send`/`!Sync` by default (via a `PhantomData` marker).
/// - We unsafely implement:
///
///   - `Sync` for `Mutex<T, R>` if `T: Send` and `R: Sync`.
///   - `Send` for `Mutex<T, R>` if `T: Send` and `R: Send`.
///
///   These bounds ensure the protected data may cross threads and that the
///   raw lock is safe to share/move as required.
/// - The guard unlocks in `Drop` via [`RawUnlock::raw_unlock`].
///
/// The correctness of cross-thread access relies on `R` providing mutual
/// exclusion and proper memory ordering.
pub struct Mutex<T, R> {
    /// The underlying raw lock primitive.
    raw: R,
    /// The protected value.
    cell: UnsafeCell<T>,
    /// Prevent default auto-`Send`/`Sync`; we add them with bounds below.
    _no_send_sync: PhantomData<*mut ()>, // !Send/!Sync by default; we implement below
}

// Safety: mutual exclusion is delegated to `R`; data may only cross threads if `T: Send`.
unsafe impl<T: Send, R: Sync> Sync for Mutex<T, R> {}
unsafe impl<T: Send, R: Send> Send for Mutex<T, R> {}

impl<T, R> Mutex<T, R> {
    /// Constructs a `Mutex` from a raw lock and an initial value.
    ///
    /// This does not acquire the lock; it just pairs `value` with `raw`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::{Mutex, RawSpin};
    /// let m = Mutex::from_raw(RawSpin::new(), 42);
    /// assert_eq!(*m.lock(), 42);
    /// ```
    pub const fn from_raw(raw: R, value: T) -> Self {
        Self {
            raw,
            cell: UnsafeCell::new(value),
            _no_send_sync: PhantomData,
        }
    }

    /// Returns a mutable reference to the inner value.
    ///
    /// Because you hold `&mut self`, no other references can exist, so
    /// locking is unnecessary.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::{Mutex, RawSpin};
    /// let mut m = Mutex::from_raw(RawSpin::new(), 1);
    /// *m.get_mut() = 2;
    /// assert_eq!(*m.lock(), 2);
    /// ```
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.cell.get_mut()
    }
}

/// A guard that releases a [`Mutex`] when dropped.
///
/// Created by [`Mutex::lock`] and [`Mutex::try_lock`]. Implements
/// [`Deref`] and [`DerefMut`] to access the protected value.
pub struct MutexGuard<'a, T, R>
where
    R: RawUnlock,
{
    m: &'a Mutex<T, R>,
}

impl<T, R> Deref for MutexGuard<'_, T, R>
where
    R: RawUnlock,
{
    type Target = T;

    fn deref(&self) -> &T {
        // Safety: the guard holds the lock exclusively.
        unsafe { &*self.m.cell.get() }
    }
}

impl<T, R> DerefMut for MutexGuard<'_, T, R>
where
    R: RawUnlock,
{
    fn deref_mut(&mut self) -> &mut T {
        // Safety: the guard holds the lock exclusively.
        unsafe { &mut *self.m.cell.get() }
    }
}

impl<T, R> Drop for MutexGuard<'_, T, R>
where
    R: RawUnlock,
{
    fn drop(&mut self) {
        // Unlock on scope exit.
        unsafe { self.m.raw.raw_unlock() }
    }
}

impl<T, R> Mutex<T, R>
where
    R: RawLock + RawUnlock,
{
    /// Acquires the lock and returns a guard that unlocks on drop.
    ///
    /// This delegates to [`RawLock::raw_lock`]. The exact blocking behavior
    /// depends on the chosen raw primitive (spin, ticket, etc.).
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::{Mutex, RawSpin};
    /// let m = Mutex::from_raw(RawSpin::new(), 0);
    /// let mut g = m.lock();
    /// *g = 10;
    /// ```
    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, T, R> {
        self.raw.raw_lock();
        MutexGuard { m: self }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// Returns `Some(guard)` on success, or `None` if the lock
    /// could not be acquired at the moment.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::{Mutex, RawSpin};
    /// let m = Mutex::from_raw(RawSpin::new(), 1);
    /// if let Some(mut g) = m.try_lock() {
    ///     *g += 1;
    /// }
    /// ```
    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T, R>> {
        if self.raw.raw_try_lock() {
            Some(MutexGuard { m: self })
        } else {
            None
        }
    }
}
