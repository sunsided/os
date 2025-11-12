# TODOs

_Auto-generated from code comments. Do not edit manually, recreate with `just todo`._

## [os/kernel/kernel-qemu/src/lib.rs](./os/kernel/kernel-qemu/src/lib.rs)

- Line [36](./os/kernel/kernel-qemu/src/lib.rs#L36): Model this as an actual sink for arbitrary port write
- Line [77](./os/kernel/kernel-qemu/src/lib.rs#L77): Model this as a regular trace macro optionally backed by the QWEMU sink

## [os/kernel/kernel-vmem/src/address_space.rs](./os/kernel/kernel-vmem/src/address_space.rs)

- Line [151](./os/kernel/kernel-vmem/src/address_space.rs#L151): Refactor to error type

## [os/kernel/kernel/src/alloc/debug.rs](./os/kernel/kernel/src/alloc/debug.rs)

- Line [7](./os/kernel/kernel/src/alloc/debug.rs#L7): Review whether the default type can be used

## [os/kernel/kernel/src/init.rs](./os/kernel/kernel/src/init.rs)

- Line [158](./os/kernel/kernel/src/init.rs#L158): 3. Allocate & map a per-CPU kernel stack (with a guard page), compute its 16-byte–aligned top.
- Line [304](./os/kernel/kernel/src/init.rs#L304): Use a different IST from PF

## [os/kernel/kernel/src/interrupts/page_fault.rs](./os/kernel/kernel/src/interrupts/page_fault.rs)

- Line [111](./os/kernel/kernel/src/interrupts/page_fault.rs#L111): Whenever returning, fix the swapgs in the asm handler above.

## [os/kernel/kernel/src/interrupts/syscall.rs](./os/kernel/kernel/src/interrupts/syscall.rs)

- Line [96](./os/kernel/kernel/src/interrupts/syscall.rs#L96): Validate the CPU frame is indeed in the correct location here.

## [os/kernel/kernel/src/userland.rs](./os/kernel/kernel/src/userland.rs)

- Line [50](./os/kernel/kernel/src/userland.rs#L50): Remove this later! Manually patches the specific L4 entry to be user-accessible.

## [os/uefi/uefi-loader/src/main.rs](./os/uefi/uefi-loader/src/main.rs)

- Line [33](./os/uefi/uefi-loader/src/main.rs#L33): Add proper documentation.
- Line [162](./os/uefi/uefi-loader/src/main.rs#L162): Document this properly
- Line [169](./os/uefi/uefi-loader/src/main.rs#L169): Handle properly

## [os/uefi/uefi-loader/src/memory.rs](./os/uefi/uefi-loader/src/memory.rs)

- Line [97](./os/uefi/uefi-loader/src/memory.rs#L97): Convert to actual pointer arithmetic ops.
