#!/bin/bash
#
# QUICK_TEST.sh — Fast SmartStrategy test (3 seeds, reduced actions)
#
# This is a quick demonstration of the distributed testing framework.
# Uses reduced action limits for faster execution.
#

set -e

echo "========================================"
echo "SmartStrategy Quick Test (Fast Demo)"
echo "========================================"
echo ""

cd /home/user/my_little_factory_manager

# Clean up old results
rm -rf batch_results_fast_* 2>/dev/null || true

# Generate batch ID
BATCH_ID=$(date +%s | tail -c 9)

echo "Batch ID: $BATCH_ID"
echo "Seeds: 3 (for quick demo)"
echo "Action limit: 100,000 per game (reduced for speed)"
echo ""

# Run the fast batch test
RESULTS_DIR="/home/user/my_little_factory_manager/batch_results_fast_${BATCH_ID}"

export BATCH_SEEDS_FAST="12345678,23456789,34567890"
export BATCH_OUTPUT_DIR_FAST="$RESULTS_DIR"

echo "Running batch simulation tests..."
echo "Results will be saved to: $RESULTS_DIR"
echo ""

timeout 600 cargo test --features simulation --test simulation smart_strategy_batch_runner_fast \
    -- --nocapture --include-ignored 2>&1

echo ""
echo "========================================"
echo "Checking results..."
echo "========================================"
echo ""

if [[ -d "$RESULTS_DIR" && -n "$(find "$RESULTS_DIR" -name '*.json' -type f 2>/dev/null)" ]]; then
    echo "✓ Results generated successfully!"
    echo ""

    # Display summary
    for seed_file in "$RESULTS_DIR"/seed_*.json; do
        if [[ -f "$seed_file" ]]; then
            SEED=$(basename "$seed_file" | sed 's/seed_//;s/.json//')
            MAX_TIER=$(jq -r '.max_tier_reached // "?" ' "$seed_file")
            COMPLETED=$(jq -r '.contracts_completed // 0' "$seed_file")
            ACTIONS=$(jq -r '.total_actions // 0' "$seed_file")

            echo "Seed $SEED: Max Tier=$MAX_TIER Completed=$COMPLETED Actions=$ACTIONS"
        fi
    done

    echo ""
    echo "Full results in: $RESULTS_DIR"
else
    echo "✗ No results found"
    exit 1
fi

echo ""
echo "========================================"
echo "Test completed successfully!"
echo "========================================"
