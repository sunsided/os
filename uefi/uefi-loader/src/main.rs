//! # UEFI Loader Main Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

#[cfg(not(all(target_arch = "x86_64", target_vendor = "unknown", target_os = "uefi")))]
compile_error!("uefi-loader must be built with --target x86_64-unknown-uefi");

use uefi::prelude::*;

#[entry]
fn main() -> Status {
    Status::SUCCESS
}
