use crate::msr::Msr;
use crate::{LoadRegisterUnsafe, StoreRegisterUnsafe};
use bitfield_struct::bitfield;
use kernel_memory_addresses::VirtualAddress;

/// `IA32_LSTAR` — 64-bit SYSCALL Target RIP (MSR `0xC000_0082`).
///
/// In 64-bit mode, `syscall` loads RIP from this register.
#[bitfield(u64)]
pub struct Ia32LStar {
    /// Bits 0–63 — 64-bit RIP target for `syscall`.
    ///
    /// Must be a canonical kernel virtual address.
    #[bits(64)]
    pub syscall_rip: VirtualAddress,
}

impl Ia32LStar {
    /// MSR index for `IA32_LSTAR`.
    pub const IA32_LSTAR: u32 = 0xC000_0082;

    /// The MSR.
    pub const MSR: Msr = Msr::new(Self::IA32_LSTAR);

    #[must_use]
    pub const fn from(rip: VirtualAddress) -> Self {
        Self::new().with_syscall_rip(rip)
    }
}

impl From<VirtualAddress> for Ia32LStar {
    fn from(address: VirtualAddress) -> Self {
        Self::from(address)
    }
}

#[cfg(feature = "asm")]
impl LoadRegisterUnsafe for Ia32LStar {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn load_unsafe() -> Self {
        let msr = unsafe { Self::MSR.load_raw() };
        Self::from_bits(msr)
    }
}

#[cfg(feature = "asm")]
impl StoreRegisterUnsafe for Ia32LStar {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    unsafe fn store_unsafe(self) {
        unsafe { Self::MSR.store_raw(self.into_bits()) }
    }
}
