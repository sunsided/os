# Rusty OS

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

  - [Building the Project](#building-the-project)
    - [Pitfalls for Compiling](#pitfalls-for-compiling)
    - [Example Build Commands](#example-build-commands)
- [License](#license)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

## Building the Project

### Pitfalls for Compiling

The workspace targets require different target architectures, for example `x86_64-unknown-uefi` for
the UEFI loader package. At this moment, `cargo build`
cannot be configured for per-package targets, so
using `cargo build` from the workspace root is bound to
fail.

For the easiest build path, use `just build` instead
of `cargo build`, or use any of the aliases defined
in [`.cargo/config.toml`](.cargo/config.toml) (such
as `cargo uefi-dev`).

### Example Build Commands

```sh
just uefi
cargo uefi
just build
```

Or, manually:

```sh
cargo build --package uefi-loader --target x86_64-unknown-uefi
```

# License

Licensed under the European Union Public Licence (EUPL), Version 1.2.
