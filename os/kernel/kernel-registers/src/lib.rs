//! # Typed `X84_64` Registers

#![cfg_attr(not(any(test, doctest)), no_std)]
#![allow(unsafe_code)]

#[cfg(feature = "efer")]
pub mod efer;

#[cfg(feature = "cr4")]
pub mod cr4;

pub trait LoadRegister {
    /// # Safety
    /// The caller must uphold the implementation-specific safety requirements.
    unsafe fn load() -> Self;
}

pub trait StoreRegister {
    /// # Safety
    /// The caller must uphold the implementation-specific safety requirements.
    unsafe fn store(self);
}
