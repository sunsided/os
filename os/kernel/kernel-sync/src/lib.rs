//! # Kernel synchronization primitives

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code)]

pub mod irq;
mod mutex;
mod raw_spin;
mod raw_ticket;
mod spin_lock;
mod sync_once_cell;

pub use irq::{IrqGuard, IrqMutex};
pub use mutex::{Mutex, MutexGuard};
pub use raw_spin::RawSpin;
pub use raw_ticket::RawTicket;
pub use spin_lock::{SpinLock, SpinLockGuard};
pub use sync_once_cell::SyncOnceCell;

pub type SpinMutex<T> = Mutex<T, RawSpin>;
pub type TicketMutex<T> = Mutex<T, RawTicket>;

impl<T> SpinMutex<T> {
    pub fn new(value: T) -> Self {
        Self::from_raw(RawSpin::new(), value)
    }
}

impl<T> TicketMutex<T> {
    pub fn new(value: T) -> Self {
        Self::from_raw(RawTicket::new(), value)
    }
}

pub trait RawLock {
    fn raw_lock(&self);
    fn raw_try_lock(&self) -> bool;
}

pub trait RawUnlock {
    unsafe fn raw_unlock(&self);
}
