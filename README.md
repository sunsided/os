# Rusty OS

[![Docs](https://img.shields.io/github/actions/workflow/status/sunsided/os/gh-pages.yaml?branch=main&label=Docs&logo=github)](https://sunsided.github.io/os/)
[![Build Kernel](https://img.shields.io/github/actions/workflow/status/sunsided/os/build-kernel.yaml?branch=main&label=Kernel%20Build&logo=github)](https://github.com/sunsided/os/actions/workflows/build-kernel.yaml)
[![Build UEFI Loader](https://img.shields.io/github/actions/workflow/status/sunsided/os/build-uefi.yaml?branch=main&label=UEFI%20Loader%20Build&logo=github)](https://github.com/sunsided/os/actions/workflows/build-uefi.yaml)

A toy x86-64 operating system written in Rust, using UEFI to boot and load a kernel image.

![A screenshot of the "OS" running](docs/screenshot.png)

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

- [Features](#features)
- [Building the Project](#building-the-project)
  - [Quick Start](#quick-start)
  - [Rust targets](#rust-targets)
  - [Pitfalls for Compiling](#pitfalls-for-compiling)
  - [Example Build Commands](#example-build-commands)
- [Example output](#example-output)
- [Related Projects](#related-projects)
- [License](#license)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

## Features

* [x] UEFI bootloader
* [x] Kernel image loading
* [x] Basic memory management
* [x] APIC timer interrupt calibrated on TSC
* [x] Basic INT80h and syscall support
* [ ] Basic I/O
* [ ] Basic process management
* [ ] Basic file system
* [ ] Basic shell
* [ ] Basic networking
* [x] Basic UEFI GOP framebuffer
* [ ] Basic graphics
* [ ] Basic audio

## Building the Project

### Quick Start

To build and run the project on QEMU, use:

```shell
task qemu
# or
task qemu PROFILE=release
```

### Rust targets

To build for UEFI and plain ELF you'll need the following:

```shell
rustup target add x86_64-unknown-uefi
rustup target add x86_64-unknown-none
```

For a simplified experience, run

```shell
task setup
```

### Pitfalls for Compiling

The workspace targets require different target architectures, for example `x86_64-unknown-uefi` for
the UEFI loader package. At this moment, `cargo build`
cannot be configured for per-package targets, so
using `cargo build` from the workspace root is bound to
fail.

For the easiest build path, use `task build` instead
of `cargo build`, or use any of the aliases defined
in [`.cargo/config.toml`](.cargo/config.toml) (such
as `cargo uefi-dev`).

### Example Build Commands

```sh
task build:uefi
cargo uefi
task build
```

Or, manually:

```sh
cargo build --package uefi-loader --target x86_64-unknown-uefi
```

## Example output

```text
[INFO] uefi_loader: UEFI Loader reporting to QEMU
[INFO] uefi_loader: Attempting to load kernel.elf ...
[INFO] uefi_loader: Loading kernel segments into memory ...
[INFO] uefi_loader: kernel.elf loaded successfully: entry=0xFFFFFFFF80100A28, segments=2
[INFO] uefi_loader::framebuffer: Obtaining Graphics Output Protocol (GOP)
[INFO] uefi_loader: Kernel boot info: 0x000000000eb70ca0
[DEBUG] uefi_loader: Allocating trampoline stack for Kernel (0x000000000E36DB50, 65536 bytes)
[INFO] uefi_loader: Creating initial kernel page tables ...
[INFO] uefi_loader::vmem: Mapping kernel ELF PT_LOAD segments ...
[INFO] uefi_loader::vmem: Mapping first 1 GiB VA = HHDM_BASE to PA=0 ...
[INFO] uefi_loader::vmem: Identity map trampoline stack ...
[INFO] uefi_loader::vmem: Identity map trampoline code at 0x000000000E36DB50 ...
[INFO] uefi_loader::vmem: Identity map bootinfo pointer ...
[INFO] uefi_loader::uefi_mmap: Exiting boot services ...
[INFO] uefi_loader::uefi_mmap: Boot services exited, we're now flying by instruments.
[DEBUG] uefi::mem::memory_map::impl_: Boot services are exited. Memory map won't be freed using the UEFI boot services allocator.
[INFO] uefi_loader::tracing: Boot Info in UEFI Loader:
  Kernel   = VA(0xFFFFFFFF80100A28)
  Trampol. = VA(0x000000000E3C7FF8)
  BI ptr   = 0x000000000eb70ca0 (@235 MiB)
       VA  = VA(0x000000000EB70CA0)
  MMAP ptr = 0x000000000eb86020 (@235 MiB), len = 7200, desc size = 48, desc ver = 1, rsdp addr = 263708692
  FB ptr   = 0x0000000080000000 (@2048 MiB), size = 4096000, width = 1280, height = 800, stride = 1280, format = BGR
[INFO] uefi_loader: Enabling supervisor write protection ...
[INFO] uefi_loader: Setting EFER.NXE ...
[INFO] uefi_loader: Enabling global pages ...
[INFO] uefi_loader: UEFI is about to jump into Kernel land. Ciao Kakao ...
[INFO] kernel::init: Kernel reporting to QEMU! Initializing bootstrap processor now.
[INFO] kernel::init: Running on Intel
[INFO] kernel::tracing: Boot Info in Kernel:
  BI ptr   = 0x000000000eb70ca0
  MMAP ptr = 0x000000000eb86020, len = 7200, desc size = 48, desc ver = 1, rsdp addr = 263708692
  FB ptr   = 0x0000000080000000, size = 4096000, width = 1280, height = 800, stride = 1280, format = BGR
[INFO] kernel::init: Initializing Virtual Memory Manager ...
[INFO] kernel::init: Supporting 512 MiB of physical RAM
[INFO] kernel::init: Initializing Kernel stack ...
[INFO] kernel::init: Designated CPU-specific stack base at 0xFFFFFF0000000000/4K.
[INFO] kernel::init: Allocating bootstrap processor kernel stack ...
[INFO] kernel::init: Probing new kernel stack at 0xFFFFFF0000009000 ...
[INFO] kernel::init: Switching to boostrap processor kernel stack ...
[INFO] kernel::init: Trampolined onto the kernel stack. Observing kernel stack top at 0xFFFFFF0000009000.
[INFO] kernel::tracing: Boot Info in Kernel:
  BI ptr   = 0x000000000eb70ca0
  MMAP ptr = 0x000000000eb86020, len = 7200, desc size = 48, desc ver = 1, rsdp addr = 263708692
  FB ptr   = 0x0000000080000000, size = 4096000, width = 1280, height = 800, stride = 1280, format = BGR
[INFO] kernel::init: Allocating IST1 stack ..
[INFO] kernel::init: IST1 mapped: base=0xFFFFFF1000001000, top=0xFFFFFF1000005000
[INFO] kernel::init: Initializing GDT and TSS ...
[INFO] kernel::init: Syscall entry stubs at 0xFFFFFFFF80118F6C
[INFO] kernel::init: Remapping UEFI GOP framebuffer (4096000 bytes) ...
[INFO] kernel::init: Remapped frame buffer to 0xFFFF888040000000
[INFO] kernel::init: Initializing IDT ...
[INFO] kernel::init: Installing interrupt handlers ...
[INFO] kernel::init: Estimating TSC frequency ...
[INFO] kernel::init: TSC frequency = 4010804020 Hz (4.01 GHz)
[INFO] kernel::apic: Initializing LAPIC (x2APIC)…
[INFO] kernel::apic: x2APIC enabled; APIC ID = 0x0
[INFO] kernel::apic: Calibrating LAPIC timer via TSC ...
[INFO] kernel::init: Enabling interrupts ...
[INFO] kernel::init: Clearing UEFI pages ...
[INFO] kernel::init: Kernel early init is done, jumping into kernel main loop ...
[INFO] kernel: Kernel doing kernel things now ...
[INFO] kernel: Observed timer rate ≈ 1000 Hz
[INFO] kernel: Kernel cycle: 1 s
[INFO] kernel: Kernel cycle: 2 s
[INFO] kernel: Jumping into userland code - will not refresh screen anymore
[INFO] kernel::userland: Mapping user demo ...
[INFO] kernel::userland: About to enter user mode ...
[INFO] kernel::tracing: CR4=00000000000006e8 (SMEP=0 SMAP=0) EFER=0000000000000d01 (NXE=1)
[INFO] kernel::alloc::debug: L4[  0]=0000000000114027 P=true RW=true US=true NX=false
[INFO] kernel::alloc::debug: L3[  1]=0000000000115027 P=true RW=true US=true PS=false NX=false
[INFO] kernel::alloc::debug: L2[  0]=0000000000116027 P=true RW=true US=true PS=false NX=false
[INFO] kernel::alloc::debug: L1[  0]=0000000000113005 P=true RW=false US=true NX=false
[INFO] kernel::userland: About to flush TLB ...
[INFO] kernel::userland: Entering user mode ...
00000000D34DC0D3
00000000B007C4FE
```

The last two outputs are from the INT80h call, followed by the syscall.

On a bad day, we get:

```text
[INFO] kernel::userland: Entering user mode ...
[ERROR] kernel::interrupts::page_fault: page fault page fault page fault
       ⠀ ⠀⠀⠀⠀⠀⠀⠙⣿⣷⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⢺⣿⣿⡆⠀⠀⠀⠀⠀⠀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⡇⠀⠀⠀⠀⠀⠀⣾⢡⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢢⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠈⣿⣿⣷⡦⠀⠀⠀⠀⢰⣿⣿⣷⠀⠀⠀⠀⠀⠀⠀⠀⠃⣠⣾⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⢻⣿⣿⣿⣆⠀⠀⠀⣾⣿⣿⣿⣷⠄⠀⠰⠤⣀⠀⠀⣴⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠃⢺⣿⣿⣿⣿⡄⠀⠀⣿⣿⢿⣿⣿⣦⣦⣦⣶⣼⣭⣼⣿⣿⣿⠇⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⢿⣿⣿⣿⣷⡆⠂⣿⣿⣞⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⢙⣿⣿⣿⣿⣷⠸⣿⣿⣿⣿⣿⣿⠟⠻⣿⣿⣿⣿⡿⣿⣿⣷⠀⠀⠀P⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠄⢿⣿⣿⣿⣿⡄⣿⣿⣿⣿⣿⣿⡀⢀⣿⣿⣿⣿⠀⢸⣿⣿⠅⠀⠀A⠀F
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠸⣿⣿⣿⣿⣇⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠁⠀ G⠀A⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠠⢐⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠀⠀⠀E⠀U⠀⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣤⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠟⠁⠀⠀⠀⠀⠀L⠀⠀
        ⠀⠀⠀⠀⠀⠀⠀⢀⣴⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠀⠀⠀⠀⠀⠀⠀T⠀⠀⠀
        ⠀⠀⠀⠀⠀⡀⣠⣾⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡔⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠀⢁⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⠠⢸⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⣀⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡄⠀⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⠀⣻⣿⣿⣿⣿⣿⡟⠋⠙⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠙⢿⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⠀⠀⠀
        ⠀⠀⠀⣿⣿⣿⣿⣿⡿⠋⠀⠀⠀⢿⣿⣿⣿⣿⣿⣿⠿⢿⡿⠛⠋⠁⠀⠀⠈⠻⣿⣿⣿⣿⣿⣿⣅⠀⠀⠀⠀⠀
        ⠀⠀⠀⣿⣿⣿⣿⡟⠃⠀⠀⠀⠀⢸⣿⣿⣿⣿⣿⣿⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠙⢻⣿⣿⣿⣿⣿⣤⡀⠀⠀⠀
        ⠀⠜⢠⣿⣿⣿⣿⠀⠀⠀⠀⠀⠀⠀⢿⣿⣿⣿⣿⣿⣗⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢻⣿⣿⣿⣿⣿⣦⠄⣠⠀
        ⠠⢸⣿⣿⣿⣿⣿⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⣿⣿⣿⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠘⣿⣿⣿⣿⣿⣿⣿⣿
        ⠀⠛⣿⣿⣿⡿⠏⠀⠀⠀⠀⠀⠀⢳⣾⣿⣿⣿⣿⣿⣿⡶⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⣿⣿⣿⣿⣿⣿
        ⠀⢨⠀⠉⠉⠀⠀⠀⠀⠀⠀⠀⠀⠙⣿⣿⡿⡿⠿⠛⠙⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠹⠏⠉⠻⠿⠟⠁
PAGE FAULT: address/cr2=0x00000000002013E0 err=0x15
User-mode instruction fetch on protected page (likely NX or SMEP)
PageFaultError {
    present: true,
    write: false,
    user: true,
    reserved_bit: false,
    instruction_fetch: true,
    protection_key: false,
    shadow_stack: false,
}
[INFO] kernel::interrupts::page_fault: Control bits:
[INFO] kernel::tracing: CR4=00000000003006e8 (SMEP=1 SMAP=1) EFER=0000000000000d01 (NXE=1)
[INFO] kernel::interrupts::page_fault: Table walk at CR2:
[INFO] kernel::alloc::debug: L4[  0]=0000000000118027 P=true RW=true US=true NX=false
[INFO] kernel::alloc::debug: L3[  0]=0000000000119027 P=true RW=true US=true PS=false NX=false
[INFO] kernel::alloc::debug: L2[  1]=000000000011a027 P=true RW=true US=true PS=false NX=false
[INFO] kernel::alloc::debug: L1[  1]=800000000011b005 P=true RW=false US=true NX=true
```

## Related Projects

* The [ruefi](https://github.com/sunsided/ruefi) project is the spiritual parent of this repo.

## License

Licensed under the European Union Public Licence (EUPL), Version 1.2.
