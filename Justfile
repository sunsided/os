# Defaults (override via env or on the CLI)

set shell := ["bash", "-cu"]

# These locations and file names vary per distribution.
# You can try to find them using `just find-ovmf`.

ovmf-dir := "/usr/share/OVMF"
ovmf-code-file := "OVMF_CODE_4M.fd"
ovmf-vars-file := "OVMF_VARS_4M.fd"

# Where to build and how to name artifacts.

build-local-dir := "qemu"
uefi-image-file := "uefi.img"

# Assembled paths for the OVMF UEFI code and variable templates.

_ofmv-code-path := ovmf-dir / ovmf-code-file
_ofmv-vars-path := ovmf-dir / ovmf-vars-file

# Where to store the local copy of the UEFI vars.

_ofmv-local-vars-path := build-local-dir / "uefi-vars.fd"

# Where to package the local development files for QEMU runs.

_esp-local-dir := build-local-dir / "esp"
_uefi-local-dir := _esp-local-dir / "EFI/Boot"

# How to rename the example EFI binary for easier access.

_uefi-local-file := "BootX64.efi"
_uefi-local-path := _uefi-local-dir / _uefi-local-file

# How to rename the example EFI binary for easier access.

_kernel-local-file := "kernel.elf"
_kernel-local-path := _uefi-local-dir / _kernel-local-file

# Where to store the image

_uefi-image-path := build-local-dir / "uefi.img"

# Target triples

_uefi_target_triple := "x86_64-unknown-uefi"
_none_target_triple := "x86_64-unknown-none"

[private]
help:
    @just --list --unsorted

# Format all code in the workspace
fmt:
    @cargo sort --workspace
    @cargo fmt --all
    @just todo

# Lint the code
lint *ARGS:
    @scripts/clippy-lint.sh {{ ARGS }}

# Lint the code and apply fixes.
fix *ARGS:
    @scripts/clippy-fix.sh {{ ARGS }}
    @just fmt

# Updates the TODO.md file
todo:
    @scripts/update-todos.sh

# Clean the targets
clean:
    @rm -r {{ build-local-dir }} || true
    @rm debug.log || true
    @cargo clean

# Run test in all projects
test: test-docs test-libs

# Run documentation test in all projects
test-docs:
    @cargo test --doc

# Run library test in all projects
test-libs *ARGS:
    @cargo test --all-features --lib {{ ARGS }}

# Build and open the docs
docs:
    @just build-docs --open

# Build the docs
build-docs *ARGS:
    @cargo doc --no-deps --all-features {{ ARGS }}

# Build all packages with default settings
build: uefi kernel

# Build all packages in debug mode
build-debug: uefi-debug kernel-debug

# Build all packages in release mode
build-release: uefi-release kernel-release

# Build the UEFI loader (default build)
uefi *ARGS:
    @cd os/uefi/uefi-loader && cargo build

# Build the UEFI loader (debug build)
uefi-debug *ARGS:
    @cd os/uefi/uefi-loader && cargo build

# Build the UEFI loader (release build)
uefi-release *ARGS:
    @cd os/uefi/uefi-loader && cargo build --release

# Build the Kernel (default build)
kernel *ARGS:
    @cd os/kernel/kernel && cargo build
    @readelf -l target/x86_64-unknown-none/debug/kernel

# Build the Kernel (debug build)
kernel-debug *ARGS:
    @cd os/kernel/kernel && cargo build
    @readelf -l target/x86_64-unknown-none/debug/kernel

# Build the Kernel (release build)
kernel-release *ARGS:
    @cd os/kernel/kernel && cargo build --release
    @readelf -l target/x86_64-unknown-none/release/kernel

# Ensures the target directory exists.
[private]
_make-target-dir:
    @mkdir -p {{ _uefi-local-dir }}

# Copy the OFMF UEFI vars to the local directory
reset-uefi-vars: _make-target-dir
    @rm {{ build-local-dir / "*.fd" }} || true
    @cp "{{ _ofmv-vars-path }}" "{{ _ofmv-local-vars-path }}"
    @echo "Updated {{ _ofmv-local-vars-path }}"

# Package the build artifacts into the target dir (debug build)
package: package-debug

# Package the build artifacts into the target dir
package-debug: reset-uefi-vars build-debug
    @rm {{ _uefi-local-dir / "*.efi" }} || true
    @cp "target/{{ _uefi_target_triple }}/debug/uefi-loader.efi" "{{ _uefi-local-path }}"
    @cp "target/{{ _none_target_triple }}/debug/kernel" "{{ _kernel-local-path }}"
    @echo "Updated {{ _uefi-local-path }} and {{ _kernel-local-path }}"

# Package the build artifacts into the target dir
package-release: reset-uefi-vars build-release
    @rm {{ _uefi-local-dir / "*.efi" }} || true
    @cp "target/{{ _uefi_target_triple }}/release/uefi-loader.efi" "{{ _uefi-local-path }}"
    @cp "target/{{ _none_target_triple }}/release/kernel" "{{ _kernel-local-path }}"
    @echo "Updated {{ _uefi-local-path }} and {{ _kernel-local-path }}"

# Run the firmware in QEMU using OVMF (pass arguments like -nographic)
run-qemu *ARGS: package
    qemu-system-x86_64 \
      -machine q35 \
      -m 256 \
      -drive "if=pflash,format=raw,readonly=on,file={{ _ofmv-code-path }}" \
      -drive "if=pflash,format=raw,file={{ _ofmv-local-vars-path }}" \
      -drive "format=raw,file=fat:rw:{{ _esp-local-dir }}" \
      -net none \
      -s \
      -debugcon file:debug.log -global isa-debugcon.iobase=0x402 \
      -monitor stdio \
      -no-reboot -no-shutdown -d cpu_reset \
      {{ ARGS }}

# Run the firmware in QEMU using OVMF (no graphic, no debug serial)
run-qemu-nographic *ARGS: package
    qemu-system-x86_64 \
      -machine q35 \
      -m 256 \
      -drive "if=pflash,format=raw,readonly=on,file={{ _ofmv-code-path }}" \
      -drive "if=pflash,format=raw,file={{ _ofmv-local-vars-path }}" \
      -drive "format=raw,file=fat:rw:{{ _esp-local-dir }}" \
      -net none \
      -s \
      -debugcon file:debug.log -global isa-debugcon.iobase=0x402 \
      -nographic \
      {{ ARGS }}
