//! # Kernel synchronization primitives

#![cfg_attr(not(test), no_std)]
#![allow(unsafe_code)]

pub mod spin_lock;
