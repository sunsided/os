# Rusty OS

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
