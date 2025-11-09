use crate::{Mutex, MutexGuard, RawLock, RawUnlock};

pub struct IrqMutex<'a, T, R: RawLock + RawUnlock> {
    _irq: IrqGuard,
    _g: MutexGuard<'a, T, R>,
}

impl<T, R: RawLock + RawUnlock> Mutex<T, R> {
    pub fn lock_irq(&self) -> IrqMutex<'_, T, R> {
        let ig = IrqGuard::new();
        let g = self.lock();
        IrqMutex { _irq: ig, _g: g }
    }
}

#[inline]
pub fn cli_stop_interrupts() {
    unsafe { core::arch::asm!("cli", options(nomem, nostack, preserves_flags)) }
}

#[inline]
pub fn sti_enable_interrupts() {
    unsafe { core::arch::asm!("sti", options(nomem, nostack, preserves_flags)) }
}

#[inline]
#[must_use]
pub fn rflags() -> u64 {
    let r: u64;
    unsafe { core::arch::asm!("pushfq; pop {}", out(reg) r, options(nostack, preserves_flags)) }
    r
}

pub struct IrqGuard {
    were_enabled: bool,
}

impl Default for IrqGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl IrqGuard {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        let enabled = (rflags() & (1 << 9)) != 0;
        if enabled {
            cli_stop_interrupts();
        }
        Self {
            were_enabled: enabled,
        }
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        if self.were_enabled {
            sti_enable_interrupts();
        }
    }
}
