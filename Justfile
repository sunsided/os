[private]
help:
    @just --list --unsorted

# Format all code in the workspace
fmt:
    cargo fmt --all

# Build all packages with default settings
build: uefi

# Build all packages in development mode
build-dev: uefi-dev

# Build all packages in release mode
build-release: uefi-release

# Build the UEFI loader with default settings
uefi *ARGS:
    cargo uefi {{ ARGS }}

# Build the UEFI loader in development mode
uefi-dev *ARGS:
    cargo uefi-dev {{ ARGS }}

# Build the UEFI loader in release mode
uefi-release *ARGS:
    cargo uefi-release {{ ARGS }}