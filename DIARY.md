# Developer Diary

## 2025-10-26

I have started wrapping my head around the Virtual Memory Manager.

I also learned about the `-monitor stdio` QEMU option as well as the `-debugcon file:debug.log`
variant.

I now added `-s` option (i.e., `-gdb tcp::1234`) to the QEMU command line, allowing for remote debugging
in RustRover. The `add-symbol-file qemu/esp/EFI/Boot/kernel.elf` command needs to be executed when
in Kernel mode (i.e., left UEFI); this can be done in the remote debug run configuration. I'm curious
to see how this works when running this in a higher-half kernel.

Since I need the information about the kernel's virtual memory address, as well as its physical location
in both the UEFI (for the ELF loader) and in the linker script, I decided to pull it into the `kernel-info`
crate which I now reuse in the kernel's `build.rs`. The linker script now uses `DEFINED(KERNEL_BASE)`
and `DEFINED(PHYS_LOAD)` to allow external configuration.

Tracing with QEMU got massively easier now with the `qemu_trace!` macro. I didn't think implementing
that would be so easy.

## 2025-10-25

Adding a serial output to the QEMU emulator turned out to be extremely helpful in finding out
what was going on when my UEFI loader would simply spin indefinitely and then get killed by QEMU
after some 60 seconds. It turned out that the calling convention was problematic: having my
Kernel `_start_kernel` as `extern "C"` (assuming the first argument, the `KernelBootInfo` struct
would be passed in `rdi`) was a bad idea: The UEFI code is PE/COFF and used `win64` calling conventions,
setting up the argument in `rcx` instead. Changing calling conventions a bit and setting up a naked
jump pad in `_start_kernel` fixed the problem.

I then added a bit of RSDP/XSDP, i.e., ACPI 1.0 and 2.0 parsing, only to later on learn that QEMU
currently does not even support ACPI 2.0.
