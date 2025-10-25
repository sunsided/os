# Rusty OS

## Table of Contents

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->

- [Pitfalls for Compiling](#pitfalls-for-compiling)
- [Example Build Command](#example-build-command)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

## Pitfalls for Compiling

The workspace targets require different target architectures, for example `x86_64-unknown-uefi` for
the UEFI loader package. At this moment, `cargo build`
cannot be configured for per-package targets, so
using `cargo build` from the workspace root is bound to
fail.

For the easiest build path, use `just build` instead
of `cargo build`, or use any of the aliases defined
in [`.cargo/config.toml`](.cargo/config.toml) (such
as `cargo uefi-dev`).

The individual packages use compile-time guards to
ensure that the correct target triple is selected.

## Example Build Command

```sh
just uefi
cargo uefi
just build
```

Or, manually:

```sh
cargo build --package uefi-loader --target x86_64-unknown-uefi
```
