# TODOs

_Auto-generated from code comments. Do not edit manually, recreate with `just todo`._

## [os/kernel/kernel-vmem/src/lib.rs](./os/kernel/kernel-vmem/src/lib.rs)

- Line [139](./os/kernel/kernel-vmem/src/lib.rs#L139): Rework using bitfield_struct
- Line [267](./os/kernel/kernel-vmem/src/lib.rs#L267): Have the mapper return a Result if the address cannot be mapped.

## [os/kernel/kernel/src/bootstrap_alloc.rs](./os/kernel/kernel/src/bootstrap_alloc.rs)

- Line [50](./os/kernel/kernel/src/bootstrap_alloc.rs#L50): !()
- Line [72](./os/kernel/kernel/src/bootstrap_alloc.rs#L72): !()

## [os/kernel/kernel/src/vmem.rs](./os/kernel/kernel/src/vmem.rs)

- Line [89](./os/kernel/kernel/src/vmem.rs#L89): Implement mapping logic with new allocator/page management design.
- Line [94](./os/kernel/kernel/src/vmem.rs#L94): Map framebuffer pages here using new allocation/mapping logic.

## [os/uefi/uefi-loader/src/elf/parser.rs](./os/uefi/uefi-loader/src/elf/parser.rs)

- Line [56](./os/uefi/uefi-loader/src/elf/parser.rs#L56): Rework flags using bitflags or bitfield_struct

## [os/uefi/uefi-loader/src/main.rs](./os/uefi/uefi-loader/src/main.rs)

- Line [32](./os/uefi/uefi-loader/src/main.rs#L32): Add proper documentation.
- Line [109](./os/uefi/uefi-loader/src/main.rs#L109): Assert tramp_stack_base_phys == tramp_stack_top_va
- Line [165](./os/uefi/uefi-loader/src/main.rs#L165): Document this properly
- Line [172](./os/uefi/uefi-loader/src/main.rs#L172): Handle properly

## [os/uefi/uefi-loader/src/memory.rs](./os/uefi/uefi-loader/src/memory.rs)

- Line [93](./os/uefi/uefi-loader/src/memory.rs#L93): Convert to actual pointer arithmetic ops.
