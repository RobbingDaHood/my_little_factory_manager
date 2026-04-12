#!/usr/bin/env bash
set -euo pipefail

if command -v pre-commit >/dev/null 2>&1; then
  echo "pre-commit already installed"
else
  if command -v pip3 >/dev/null 2>&1; then
    echo "Installing pre-commit via pip3..."
    pip3 install --user pre-commit
    export PATH="$HOME/.local/bin:$PATH"
  else
    echo "pre-commit not found and pip3 not available. Install pre-commit manually (pip install pre-commit) and re-run." >&2
    exit 1
  fi
fi

pre-commit install
pre-commit install --hook-type pre-push || true

echo "pre-commit hooks installed (pre-commit should be available on PATH)."
