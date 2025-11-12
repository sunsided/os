use core::{
    cell::UnsafeCell,
    hint::spin_loop,
    mem::MaybeUninit,
    sync::atomic::{AtomicU8, Ordering},
};

/// 0 = UNINIT, 1 = INITING, 2 = READY
const UNINIT: u8 = 0;
const INITING: u8 = 1;
const READY: u8 = 2;

/// A minimal, lock-free, spin-based `OnceCell`.
///
/// `SyncOnceCell<T>` lazily initializes a `T` at most once and then
/// provides shared access to it. The first caller to observe the
/// uninitialized state runs the initializer; all others spin until the
/// value becomes available.
///
/// This type uses a single-writer, multi-reader pattern with
/// acquire/release ordering and a short busy-wait during initialization.
/// It does **not** handle panics in the initializer: if the initializer
/// panics, other threads will spin forever on `INITING`.
///
/// # Examples
///
/// ```
/// use kernel_sync::SyncOnceCell;
///
/// static CELL: SyncOnceCell<String> = SyncOnceCell::new();
///
/// let s1 = CELL.get_or_init(|| "hello".to_owned());
/// let s2 = CELL.get().unwrap();
/// assert_eq!(&s1, &s2);
/// ```
///
/// # Concurrency
///
/// - Single initializer wins via `compare_exchange`.
/// - Readers observe readiness via `Acquire` loads after the value is
///   fully written and published with `Release`.
///
/// # Panics
///
/// If the initializer closure panics, the cell remains stuck in the
/// `INITING` state and all future calls will spin forever.
pub struct SyncOnceCell<T> {
    state: AtomicU8,
    value: UnsafeCell<MaybeUninit<T>>,
}

impl<T> Default for SyncOnceCell<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SyncOnceCell<T> {
    /// Creates a new, empty `SyncOnceCell`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SyncOnceCell;
    ///
    /// static CELL: SyncOnceCell<u32> = SyncOnceCell::new();
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(UNINIT),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Returns `Some(&T)` if the cell has been initialized.
    ///
    /// Returns `None` if the cell is not ready yet.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SyncOnceCell;
    ///
    /// let cell = SyncOnceCell::new();
    /// assert!(cell.get().is_none());
    /// let _ = cell.get_or_init(|| 7);
    /// assert_eq!(cell.get(), Some(&7));
    /// ```
    #[inline]
    pub fn get(&self) -> Option<&T> {
        if self.state.load(Ordering::Acquire) == READY {
            // SAFETY: READY guarantees the write is done
            Some(unsafe { &*(*self.value.get()).as_ptr() })
        } else {
            None
        }
    }

    /// Initializes the cell at most once and returns `&T`.
    ///
    /// If the cell is already initialized, returns a shared reference
    /// to the existing value. Otherwise, it runs the provided closure
    /// to produce the value. While another thread is initializing, this
    /// call spins until the value becomes ready.
    ///
    /// The reference is tied to the lifetime of the cell.
    ///
    /// # Ordering
    ///
    /// - The winning initializer uses `Acquire` on CAS and publishes the
    ///   value with a `Release` store to `READY`.
    /// - Readers use `Acquire` loads to observe the published value.
    ///
    /// # Panics
    ///
    /// If `init` panics, the cell remains in the `INITING` state and all
    /// subsequent calls will spin forever. Ensure the initializer cannot
    /// panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use kernel_sync::SyncOnceCell;
    ///
    /// let cell = SyncOnceCell::new();
    /// let v1 = cell.get_or_init(|| 42);
    /// let v2 = cell.get_or_init(|| unreachable!());
    /// assert_eq!(v1, v2);
    /// ```
    pub fn get_or_init(&self, init: impl FnOnce() -> T) -> &T {
        // Fast path
        if let Some(v) = self.get() {
            return v;
        }

        // Try to take initialization
        if self
            .state
            .compare_exchange(UNINIT, INITING, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // We are the initializer
            let v = init();
            unsafe {
                (*self.value.get()).write(v);
            }
            // Publish value before marking READY
            self.state.store(READY, Ordering::Release);
            // SAFETY: just wrote it
            return unsafe { &*(*self.value.get()).as_ptr() };
        }

        // Someone else is initializing; wait until READY
        while self.state.load(Ordering::Acquire) != READY {
            spin_loop();
        }
        // SAFETY: READY
        unsafe { &*(*self.value.get()).as_ptr() }
    }
}

// Safety: shared after READY; initialization is single-writer.
unsafe impl<T: Sync> Sync for SyncOnceCell<T> {}
unsafe impl<T: Send> Send for SyncOnceCell<T> {}
