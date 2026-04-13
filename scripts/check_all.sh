#!/usr/bin/env bash
set -uo pipefail

# Unified check script: runs all validations and reports ALL failures at the end.
# Used by `make check` and as the agent's single validation command.

FAILURES=()

# --- 1. Format (auto-fix + stage) ---
echo "=== cargo fmt (auto-fix) ==="
cargo fmt
FMT_EXIT=$?
if [ $FMT_EXIT -ne 0 ]; then
  FAILURES+=("cargo fmt")
else
  git diff --name-only --diff-filter=M 2>/dev/null | while read -r file; do
    if [[ "$file" == *.rs ]]; then
      git add "$file"
    fi
  done
  echo "  ✓ fmt passed"
fi

# --- 2. Clippy ---
echo ""
echo "=== cargo clippy ==="
cargo clippy --all-targets --all-features -- -D warnings 2>&1
CLIPPY_EXIT=$?
if [ $CLIPPY_EXIT -ne 0 ]; then
  FAILURES+=("cargo clippy")
else
  echo "  ✓ clippy passed"
fi

# --- 3. Build ---
echo ""
echo "=== cargo build ==="
cargo build 2>&1
BUILD_EXIT=$?
if [ $BUILD_EXIT -ne 0 ]; then
  FAILURES+=("cargo build")
else
  echo "  ✓ build passed"
fi

# --- 4. Tests ---
echo ""
echo "=== cargo test ==="
cargo test 2>&1
TEST_EXIT=$?
if [ $TEST_EXIT -ne 0 ]; then
  FAILURES+=("cargo test")
else
  echo "  ✓ tests passed"
fi

# --- 5. Check for unwraps in production code ---
echo ""
echo "=== unwrap check ==="
UNWRAP_COUNT=$(grep -rn "\.unwrap()" src/ --include="*.rs" 2>/dev/null | grep -v "// ok:" | grep -v "#\[cfg(test)\]" | wc -l)
if [ "$UNWRAP_COUNT" -gt 0 ]; then
  echo "  ⚠ Found $UNWRAP_COUNT unwrap() calls in src/ (review these):"
  grep -rn "\.unwrap()" src/ --include="*.rs" | grep -v "// ok:" | grep -v "#\[cfg(test)\]"
fi

# --- 6. Coverage (matches CI threshold) ---
echo ""
echo "=== coverage check (80% threshold) ==="
if command -v cargo-llvm-cov &>/dev/null; then
  cargo llvm-cov --workspace --fail-under-lines 80 2>&1
  COV_EXIT=$?
  if [ $COV_EXIT -ne 0 ]; then
    FAILURES+=("coverage (below 80%)")
  else
    echo "  ✓ coverage passed"
  fi
else
  echo "  ⚠ cargo-llvm-cov not installed, skipping coverage check"
  echo "    Install: cargo install --locked cargo-llvm-cov && rustup component add llvm-tools-preview"
fi

# --- Summary ---
echo ""
echo "==============================="
if [ ${#FAILURES[@]} -eq 0 ]; then
  echo "✅ All checks passed"
  exit 0
else
  echo "❌ ${#FAILURES[@]} check(s) failed:"
  for f in "${FAILURES[@]}"; do
    echo "  - $f"
  done
  exit 1
fi
