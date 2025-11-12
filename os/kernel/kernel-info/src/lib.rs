//! # Kernel Helpers

#![cfg_attr(not(any(test, doctest)), no_std)]
#![deny(unsafe_code)]

pub mod boot;
pub mod memory;
