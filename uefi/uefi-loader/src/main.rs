//! # UEFI Loader Main Entry Point

#![no_std]
#![no_main]
#![allow(unsafe_code)]

use uefi::prelude::*;

#[entry]
fn main() -> Status {
    if uefi::helpers::init().is_err() {
        return Status::UNSUPPORTED;
    }

    uefi::println!("Hello from UEFI Loader!");
    boot::stall(1_000_000);

    Status::SUCCESS
}
