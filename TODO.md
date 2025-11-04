# TODOs

_Auto-generated from code comments. Do not edit manually, recreate with `just todo`._

## [os/kernel/kernel-qemu/src/lib.rs](./os/kernel/kernel-qemu/src/lib.rs)

- Line [36](./os/kernel/kernel-qemu/src/lib.rs#L36): Model this as an actual sink for arbitrary port write
- Line [77](./os/kernel/kernel-qemu/src/lib.rs#L77): Model this as a regular trace macro optionally backed by the QWEMU sink

## [os/kernel/kernel-vmem/src/address_space.rs](./os/kernel/kernel-vmem/src/address_space.rs)

- Line [151](./os/kernel/kernel-vmem/src/address_space.rs#L151): Refactor to error type

## [os/kernel/kernel/src/init.rs](./os/kernel/kernel/src/init.rs)

- Line [79](./os/kernel/kernel/src/init.rs#L79): 1. Start on the boot stack (your _start_kernel does this).
- Line [80](./os/kernel/kernel/src/init.rs#L80): 2. Bring up the minimal MM you need to map memory.
- Line [81](./os/kernel/kernel/src/init.rs#L81): 3. Allocate & map a per-CPU kernel stack (with a guard page), compute its 16-byte–aligned top.
- Line [82](./os/kernel/kernel/src/init.rs#L82): 4. Switch rsp to that new top.
- Line [83](./os/kernel/kernel/src/init.rs#L83): 5. Now call gdt::init_gdt_and_tss(new_top, ist) and then IDT setup.
- Line [87](./os/kernel/kernel/src/init.rs#L87): Bad idea, should allocate proper kernel stack here
- Line [90](./os/kernel/kernel/src/init.rs#L90): feeds boot stack into TSS.rsp0

## [os/kernel/kernel/src/syscall.rs](./os/kernel/kernel/src/syscall.rs)

- Line [74](./os/kernel/kernel/src/syscall.rs#L74): Validate the CPU frame is indeed in the correct location here.
- Line [180](./os/kernel/kernel/src/syscall.rs#L180): Code duplication with kernel-qemu/src/lib.rs

## [os/kernel/kernel/src/userland.rs](./os/kernel/kernel/src/userland.rs)

- Line [73](./os/kernel/kernel/src/userland.rs#L73): !("alloc & copy USER_CODE into a phys page(s)");
- Line [74](./os/kernel/kernel/src/userland.rs#L74): !("alloc N pages for user stack");

## [os/uefi/uefi-loader/src/main.rs](./os/uefi/uefi-loader/src/main.rs)

- Line [33](./os/uefi/uefi-loader/src/main.rs#L33): Add proper documentation.
- Line [162](./os/uefi/uefi-loader/src/main.rs#L162): Document this properly
- Line [169](./os/uefi/uefi-loader/src/main.rs#L169): Handle properly

## [os/uefi/uefi-loader/src/memory.rs](./os/uefi/uefi-loader/src/memory.rs)

- Line [97](./os/uefi/uefi-loader/src/memory.rs#L97): Convert to actual pointer arithmetic ops.
