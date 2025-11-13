//! # Typed `X84_64` Registers

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code)]

#[cfg(feature = "efer")]
pub mod efer;

#[cfg(feature = "cr4")]
pub mod cr4;

pub trait LoadRegisterUnsafe {
    /// # Safety
    /// The caller must uphold the implementation-specific safety requirements.
    /// For example, the register access might be privileged and require kernel mode (Ring 0).
    unsafe fn load_unsafe() -> Self;
}

pub trait StoreRegisterUnsafe {
    /// # Safety
    /// The caller must uphold the implementation-specific safety requirements.
    /// For example, the register access might be privileged and require kernel mode (Ring 0).
    unsafe fn store_unsafe(self);
}

pub trait LoadRegister {
    /// # Safety
    /// It is generally safe to load this register even from user mode.
    fn load() -> Self;
}

pub trait StoreRegister {
    /// # Safety
    /// It is generally safe to store this register even from user mode.
    fn store(self);
}

impl<T> LoadRegisterUnsafe for T
where
    T: LoadRegister,
{
    #[inline]
    unsafe fn load_unsafe() -> Self {
        <Self as LoadRegister>::load()
    }
}

impl<T> StoreRegisterUnsafe for T
where
    T: StoreRegister,
{
    #[inline]
    unsafe fn store_unsafe(self) {
        <Self as StoreRegister>::store(self);
    }
}
