use kernel_info::memory;
use std::{env, path::PathBuf};

fn main() {
    // Point to the linker script
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ld = manifest_dir.join("kernel.ld");

    // Sanity checks (fail fast during build)
    let kernel_base = memory::KERNEL_BASE;
    let phys_load = memory::PHYS_LOAD;
    assert_eq!(
        kernel_base & ((1u64 << 21) - 1),
        0,
        "KERNEL_BASE must be 2 MiB aligned (got {kernel_base:#x})"
    );
    assert_eq!(
        phys_load & 0xfff,
        0,
        "PHYS_LOAD must be 4 KiB aligned (got {phys_load:#x})"
    );

    // Rebuild when inputs change
    println!("cargo:rerun-if-changed={}", ld.display());

    // Linker script
    println!("cargo:rustc-link-arg-bins=-T{}", ld.display());

    // Provide symbols to the linker script
    // (cargo:rustc-link-arg-bins passes args directly to the linker)
    println!("cargo:rustc-link-arg-bins=--defsym=KERNEL_BASE={kernel_base:#x}");
    println!("cargo:rustc-link-arg-bins=--defsym=PHYS_LOAD={phys_load:#x}");
}
