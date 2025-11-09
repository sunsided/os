use crate::{RawLock, RawUnlock};
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

pub struct RawSpin {
    held: AtomicBool,
}

impl Default for RawSpin {
    fn default() -> Self {
        Self::new()
    }
}

impl RawSpin {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn lock(&self) {
        // Fast path: try once, then spin with backoff
        while self.held.swap(true, Ordering::Acquire) {
            while self.held.load(Ordering::Relaxed) {
                spin_loop();
            }
        }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        !self.held.swap(true, Ordering::Acquire)
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        self.held.store(false, Ordering::Release);
    }
}

impl RawLock for RawSpin {
    fn raw_lock(&self) {
        self.lock();
    }

    fn raw_try_lock(&self) -> bool {
        self.try_lock()
    }
}

impl RawUnlock for RawSpin {
    unsafe fn raw_unlock(&self) {
        unsafe { self.unlock() }
    }
}
