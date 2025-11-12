use crate::{Mutex, MutexGuard, RawLock, RawUnlock};

/// A mutex guard that also disables interrupts while held.
///
/// `IrqMutex` combines an interrupt guard with a regular [`MutexGuard`].
/// When created via [`Mutex::lock_irq`], it:
///
/// 1. saves the current interrupt state and disables interrupts, and
/// 2. acquires the underlying mutex,
///
/// releasing them in reverse order on drop.
///
/// This prevents interrupt handlers from preempting the critical section
/// and re-entering code that uses the same lock.
///
/// # Platform
///
/// Uses `cli/sti` and `pushfq/pop` and therefore targets `x86/x86_64`.
///
/// # Safety & Privilege
///
/// These operations must run in a context where `cli`/`sti` are legal
/// (e.g., kernel or a suitable hypervisor context). Calling from user space
/// or non-privileged modes is invalid.
///
/// # Examples
///
/// ```no_run
/// use kernel_sync::{Mutex, RawSpin};
/// use kernel_sync::irq::IrqMutex; // assuming this module
///
/// static M: Mutex<u64, RawSpin> = Mutex::from_raw(RawSpin::new(), 0);
///
/// // Disable interrupts and lock for the duration of the scope.
/// {
///     let _ig = M.lock_irq();
///     // critical section guarded from both threads and interrupts
/// }
/// // interrupts and mutex are released here
/// ```
pub struct IrqMutex<'a, T, R: RawLock + RawUnlock> {
    _irq: IrqGuard,
    _g: MutexGuard<'a, T, R>,
}

impl<T, R: RawLock + RawUnlock> Mutex<T, R> {
    /// Acquires the mutex with interrupts disabled for the guard’s lifetime.
    ///
    /// This constructs an [`IrqGuard`] to save/disable interrupts, then
    /// acquires the mutex and returns a paired [`IrqMutex`] guard. Dropping
    /// the guard releases the mutex and restores interrupts if they were
    /// previously enabled.
    ///
    /// # Platform / Privilege
    ///
    /// Requires `x86/x86_64` and a privileged execution context where
    /// `cli/sti` are permitted.
    #[inline]
    pub fn lock_irq(&self) -> IrqMutex<'_, T, R> {
        let ig = IrqGuard::new();
        let g = self.lock();
        IrqMutex { _irq: ig, _g: g }
    }
}

/// Disables hardware interrupts (`cli`).
///
/// # Platform
///
/// `x86/x86_64`.
///
/// # Safety & Privilege
///
/// Must only be called in contexts where `cli` is permitted. Misuse can
/// hang the system or violate execution environment rules.
#[inline]
pub fn cli_stop_interrupts() {
    unsafe { core::arch::asm!("cli", options(nomem, nostack, preserves_flags)) }
}

/// Enables hardware interrupts (`sti`).
///
/// # Platform
///
/// `x86/x86_64`.
///
/// # Safety & Privilege
///
/// Must only be called in contexts where `sti` is permitted. Typically used
/// to restore a previously disabled interrupt state.
#[inline]
pub fn sti_enable_interrupts() {
    unsafe { core::arch::asm!("sti", options(nomem, nostack, preserves_flags)) }
}

/// Returns the current `RFLAGS` value (via `pushfq/pop`).
///
/// Bit 9 (`IF`) indicates whether interrupts are enabled.
///
/// # Platform
///
/// `x86/x86_64`.
///
/// # Safety & Privilege
///
/// Requires an execution context where reading flags this way is valid.
#[inline]
#[must_use]
pub fn rflags() -> u64 {
    let r: u64;
    unsafe { core::arch::asm!("pushfq; pop {}", out(reg) r, options(nostack, preserves_flags)) }
    r
}

/// RAII guard that disables interrupts on creation and restores them on drop.
///
/// `IrqGuard::new()` snapshots the `IF` bit (bit 9 of `RFLAGS`). If interrupts
/// were enabled, it executes `cli`. On drop, it executes `sti` **only** if
/// they were previously enabled, preserving the original state.
///
/// # Platform / Privilege
///
/// Requires `x86/x86_64` and a privileged context permitting `cli/sti`.
///
/// # Examples
///
/// ```no_run
/// use kernel_sync::irq::{IrqGuard, rflags};
///
/// let before = rflags();
/// {
///     let _g = IrqGuard::new(); // interrupts disabled here if previously enabled
///     // critical section
/// }
/// let after = rflags(); // IF restored to prior state
/// ```
pub struct IrqGuard {
    /// Whether interrupts were enabled (IF=1) when the guard was created.
    were_enabled: bool,
}

impl Default for IrqGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl IrqGuard {
    /// Disables interrupts if they are currently enabled and remembers the state.
    ///
    /// Uses `rflags()` to read the IF bit and conditionally issues `cli`.
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
    /// Restores interrupts (`sti`) only if they were previously enabled.
    fn drop(&mut self) {
        if self.were_enabled {
            sti_enable_interrupts();
        }
    }
}
