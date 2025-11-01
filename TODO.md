# TODOs

_Auto-generated from code comments. Do not edit manually, recreate with `just todo`._

## [os/kernel/kernel-qemu/src/lib.rs](./os/kernel/kernel-qemu/src/lib.rs)

- Line [36](./os/kernel/kernel-qemu/src/lib.rs#L36): Model this as an actual sink for arbitrary port write
- Line [77](./os/kernel/kernel-qemu/src/lib.rs#L77): Model this as a regular trace macro optionally backed by the QWEMU sink

## [os/kernel/kernel-syscall/src/syscall.rs](./os/kernel/kernel-syscall/src/syscall.rs)

- Line [110](./os/kernel/kernel-syscall/src/syscall.rs#L110): Code duplication with kernel-qemu/src/lib.rs

## [os/kernel/kernel-vmem/src/address_space.rs](./os/kernel/kernel-vmem/src/address_space.rs)

- Line [151](./os/kernel/kernel-vmem/src/address_space.rs#L151): Refactor to error type

## [os/kernel/kernel/src/userland.rs](./os/kernel/kernel/src/userland.rs)

- Line [19](./os/kernel/kernel/src/userland.rs#L19): !("alloc & copy USER_CODE into a phys page(s)");
- Line [20](./os/kernel/kernel/src/userland.rs#L20): !("alloc N pages for user stack");

## [os/uefi/uefi-loader/src/main.rs](./os/uefi/uefi-loader/src/main.rs)

- Line [33](./os/uefi/uefi-loader/src/main.rs#L33): Add proper documentation.
- Line [162](./os/uefi/uefi-loader/src/main.rs#L162): Document this properly
- Line [169](./os/uefi/uefi-loader/src/main.rs#L169): Handle properly

## [os/uefi/uefi-loader/src/memory.rs](./os/uefi/uefi-loader/src/memory.rs)

- Line [93](./os/uefi/uefi-loader/src/memory.rs#L93): Convert to actual pointer arithmetic ops.
