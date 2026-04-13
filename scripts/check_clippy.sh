#!/usr/bin/env bash
set -euo pipefail

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup not found. Please install Rust toolchain." >&2
  exit 1
fi

if ! rustup component list --installed | grep -q '^clippy'; then
  echo "Installing clippy component..."
  rustup component add clippy || true
fi

echo "Running cargo clippy..."
cargo clippy --all-targets --all-features -- -D warnings
