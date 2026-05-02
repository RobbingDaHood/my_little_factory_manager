#!/bin/bash
# Quick test runner for tier 50 - uses diagnostic test for speed

echo "Testing tier 50 with diagnostic test (faster)..."
cargo test --features simulation --test simulation -- --include-ignored smart_strategy_diagnostic --nocapture 2>&1 | tee /tmp/test_output.txt | tail -50

# Check if it reached tier 50
if grep -q "overall_max_tier.*50" /tmp/test_output.txt || grep -q "Tier.*50" /tmp/test_output.txt; then
    echo ""
    echo "✓ TEST PASSED: Reached tier 50"
    exit 0
else
    echo ""
    echo "✗ TEST FAILED: Did not reach tier 50"
    # Try to extract the max tier reached
    grep -E "Tier|overall_max" /tmp/test_output.txt | tail -5
    exit 1
fi
