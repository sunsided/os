use crate::{RawLock, RawUnlock};
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

pub struct Mutex<T, R> {
    raw: R,
    cell: UnsafeCell<T>,
    _no_send_sync: PhantomData<*mut ()>, // !Send/!Sync by default; we implement below
}

unsafe impl<T: Send, R: Sync> Sync for Mutex<T, R> {}
unsafe impl<T: Send, R: Send> Send for Mutex<T, R> {}

impl<T, R> Mutex<T, R> {
    pub const fn from_raw(raw: R, value: T) -> Self {
        Self {
            raw,
            cell: UnsafeCell::new(value),
            _no_send_sync: PhantomData,
        }
    }

    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.cell.get_mut()
    }
}

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
        unsafe { &*self.m.cell.get() }
    }
}

impl<T, R> DerefMut for MutexGuard<'_, T, R>
where
    R: RawUnlock,
{
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.m.cell.get() }
    }
}

impl<T, R> Drop for MutexGuard<'_, T, R>
where
    R: RawUnlock,
{
    fn drop(&mut self) {
        unsafe { self.m.raw.raw_unlock() }
    }
}

impl<T, R> Mutex<T, R>
where
    R: RawLock + RawUnlock,
{
    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, T, R> {
        self.raw.raw_lock();
        MutexGuard { m: self }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T, R>> {
        if self.raw.raw_try_lock() {
            Some(MutexGuard { m: self })
        } else {
            None
        }
    }
}
