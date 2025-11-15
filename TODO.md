# TODOs

_Auto-generated from code comments. Do not edit manually, recreate with `just todo`._

## [os/kernel/kernel-qemu/src/lib.rs](./os/kernel/kernel-qemu/src/lib.rs)

- Line [216](./os/kernel/kernel-qemu/src/lib.rs#L216): Model this as an actual sink for arbitrary port write
- Line [257](./os/kernel/kernel-qemu/src/lib.rs#L257): Model this as a regular trace macro optionally backed by the QWEMU sink

## [os/kernel/kernel-vmem/src/address_space.rs](./os/kernel/kernel-vmem/src/address_space.rs)

- Line [177](./os/kernel/kernel-vmem/src/address_space.rs#L177): Refactor to error type

## [os/kernel/kernel/src/alloc/debug.rs](./os/kernel/kernel/src/alloc/debug.rs)

- Line [8](./os/kernel/kernel/src/alloc/debug.rs#L8): Review whether the default type can be used

## [os/kernel/kernel/src/init.rs](./os/kernel/kernel/src/init.rs)

- Line [266](./os/kernel/kernel/src/init.rs#L266): Restrict allocator to actual available RAM size.
- Line [414](./os/kernel/kernel/src/init.rs#L414): Use a different IST from PF

## [os/kernel/kernel/src/interrupts/page_fault.rs](./os/kernel/kernel/src/interrupts/page_fault.rs)

- Line [109](./os/kernel/kernel/src/interrupts/page_fault.rs#L109): Whenever returning, fix the swapgs in the asm handler above.

## [os/kernel/kernel/src/interrupts/syscall.rs](./os/kernel/kernel/src/interrupts/syscall.rs)

- Line [89](./os/kernel/kernel/src/interrupts/syscall.rs#L89): Validate the CPU frame is indeed in the correct location here.

## [os/kernel/kernel/src/userland.rs](./os/kernel/kernel/src/userland.rs)

- Line [50](./os/kernel/kernel/src/userland.rs#L50): Remove later

## [os/uefi/uefi-loader/src/main.rs](./os/uefi/uefi-loader/src/main.rs)

- Line [246](./os/uefi/uefi-loader/src/main.rs#L246): Add proper documentation.

## [os/uefi/uefi-loader/src/memory.rs](./os/uefi/uefi-loader/src/memory.rs)

- Line [97](./os/uefi/uefi-loader/src/memory.rs#L97): Convert to actual pointer arithmetic ops.
