# TODOs

_Auto-generated from code comments. Do not edit manually, recreate with `just todo`._

## [os/kernel/kernel-qemu/src/lib.rs](./os/kernel/kernel-qemu/src/lib.rs)

- Line [39](./os/kernel/kernel-qemu/src/lib.rs#L39): Model this as an actual sink for arbitrary port write
- Line [80](./os/kernel/kernel-qemu/src/lib.rs#L80): Model this as a regular trace macro optionally backed by the QWEMU sink

## [os/kernel/kernel-vmem/src/address_space.rs](./os/kernel/kernel-vmem/src/address_space.rs)

- Line [172](./os/kernel/kernel-vmem/src/address_space.rs#L172): Refactor to error type

## [os/kernel/kernel/src/alloc/debug.rs](./os/kernel/kernel/src/alloc/debug.rs)

- Line [9](./os/kernel/kernel/src/alloc/debug.rs#L9): Review whether the default type can be used

## [os/kernel/kernel/src/init.rs](./os/kernel/kernel/src/init.rs)

- Line [179](./os/kernel/kernel/src/init.rs#L179): Restrict allocator to actual available RAM size.
- Line [319](./os/kernel/kernel/src/init.rs#L319): Use a different IST from PF

## [os/kernel/kernel/src/interrupts/page_fault.rs](./os/kernel/kernel/src/interrupts/page_fault.rs)

- Line [109](./os/kernel/kernel/src/interrupts/page_fault.rs#L109): Whenever returning, fix the swapgs in the asm handler above.

## [os/kernel/kernel/src/interrupts/syscall.rs](./os/kernel/kernel/src/interrupts/syscall.rs)

- Line [96](./os/kernel/kernel/src/interrupts/syscall.rs#L96): Validate the CPU frame is indeed in the correct location here.

## [os/kernel/kernel/src/userland.rs](./os/kernel/kernel/src/userland.rs)

- Line [49](./os/kernel/kernel/src/userland.rs#L49): Remove this later! Manually patches the specific L4 entry to be user-accessible.

## [os/uefi/uefi-loader/src/main.rs](./os/uefi/uefi-loader/src/main.rs)

- Line [34](./os/uefi/uefi-loader/src/main.rs#L34): Add proper documentation.
- Line [166](./os/uefi/uefi-loader/src/main.rs#L166): Document this properly
- Line [173](./os/uefi/uefi-loader/src/main.rs#L173): Handle properly

## [os/uefi/uefi-loader/src/memory.rs](./os/uefi/uefi-loader/src/memory.rs)

- Line [97](./os/uefi/uefi-loader/src/memory.rs#L97): Convert to actual pointer arithmetic ops.
