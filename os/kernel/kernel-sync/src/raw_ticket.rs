use crate::{RawLock, RawUnlock};
use core::hint::spin_loop;
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct RawTicket {
    next: AtomicUsize,
    owner: AtomicUsize,
}

impl Default for RawTicket {
    fn default() -> Self {
        Self::new()
    }
}

impl RawTicket {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: AtomicUsize::new(0),
            owner: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn lock(&self) {
        let ticket = self.next.fetch_add(1, Ordering::Relaxed);
        // Acquire when we observe our turn
        while self.owner.load(Ordering::Acquire) != ticket {
            spin_loop();
        }
    }

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

    #[inline]
    pub unsafe fn unlock(&self) {
        // Release when we advance owner
        let t = self.owner.load(Ordering::Relaxed);
        self.owner.store(t + 1, Ordering::Release);
    }
}

impl RawLock for RawTicket {
    fn raw_lock(&self) {
        self.lock();
    }

    fn raw_try_lock(&self) -> bool {
        self.try_lock()
    }
}

impl RawUnlock for RawTicket {
    unsafe fn raw_unlock(&self) {
        unsafe { self.unlock() }
    }
}
