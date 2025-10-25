#!/usr/bin/env bash
set -euo pipefail

# Detect workspace root by presence of [workspace] in Cargo.toml
workspace_root=$(grep -rl '^\[workspace\]' ./Cargo.toml || true)

find . -type f -name Cargo.toml | while read -r cargo_toml; do
  # Skip the workspace root
  if [[ -n "$workspace_root" && "$(realpath "$cargo_toml")" == "$(realpath "$workspace_root")" ]]; then
    echo "Skipping workspace root: $cargo_toml"
    continue
  fi

  dir=$(dirname "$cargo_toml")
  echo "Running clippy in $dir"
  (
    cd "$dir"
    cargo clippy --all-features "$@"
  )
done
