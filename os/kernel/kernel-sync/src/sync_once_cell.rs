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
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(UNINIT),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Returns `Some(&T)` if already initialized.
    #[inline]
    pub fn get(&self) -> Option<&T> {
        if self.state.load(Ordering::Acquire) == READY {
            // SAFETY: READY guarantees the write is done
            Some(unsafe { &*(*self.value.get()).as_ptr() })
        } else {
            None
        }
    }

    /// Initialize at most once and return `&T`.
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
