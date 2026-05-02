#!/bin/bash
#
# AGENT_SCRIPT.sh — Distributed SmartStrategy seed testing campaign
#
# Runs 12 games with unique randomized seeds and creates GitHub sub-issues
# with detailed results, linking them to main issue #83.
#
# Usage: bash AGENT_SCRIPT.sh
#

set -e

# Generate unique batch UUID (fallback if uuidgen not available)
if command -v uuidgen &> /dev/null; then
    BATCH_UUID=$(uuidgen)
else
    BATCH_UUID=$(date +%s%N | sha256sum | head -c 16)
fi
BATCH_ID="${BATCH_UUID:0:8}"

echo "========================================"
echo "SmartStrategy Distributed Seed Testing"
echo "========================================"
echo "Batch ID: $BATCH_ID"
echo "Testing 12 games with different seeds"
echo ""

# Main GitHub issue number
MAIN_ISSUE=83

# Repository details
OWNER="RobbingDaHood"
REPO="my_little_factory_manager"

# Working directory
WORK_DIR="/home/user/my_little_factory_manager"
cd "$WORK_DIR"

# Temporary directory for results
RESULTS_DIR="$WORK_DIR/batch_results_${BATCH_ID}"
mkdir -p "$RESULTS_DIR"

# Array of 12 seeds for distributed testing
declare -a SEEDS=(
    12345678
    23456789
    34567890
    45678901
    56789012
    67890123
    78901234
    89012345
    90123456
    1234567
    2345678
    3456789
)

TOTAL_GAMES=${#SEEDS[@]}
BATCH_SIZE=4

echo "Seeds: ${SEEDS[@]}"
echo ""

# Build and run the batch runner
echo "Building with simulation feature..."
cargo build --features simulation --tests 2>&1 | grep -E "(Compiling|Finished|error)" || true

echo ""
echo "Running batch simulation tests..."
echo "Results will be saved to: $RESULTS_DIR"
echo ""

# Export environment variables for the test
export BATCH_SEEDS=$(IFS=,; echo "${SEEDS[*]}")
export BATCH_OUTPUT_DIR="$RESULTS_DIR"
export BATCH_MAX_ACTIONS=500000

# Run the batch runner test
timeout 3600 cargo test --features simulation --test simulation smart_strategy_batch_runner -- --nocapture --include-ignored 2>&1 | tee "$RESULTS_DIR/test_output.log"

echo ""
echo "========================================"
echo "Test execution completed!"
echo "========================================"
echo ""

# Check if results were generated
if [[ -d "$RESULTS_DIR" && -n "$(find "$RESULTS_DIR" -name '*.json' -type f)" ]]; then
    echo "Results Summary:"
    echo ""

    GAME_NUM=1
    for seed_file in "$RESULTS_DIR"/seed_*.json; do
        if [[ -f "$seed_file" ]]; then
            SEED=$(basename "$seed_file" | sed 's/seed_//;s/.json//')
            MAX_TIER=$(jq -r '.max_tier_reached // "?" ' "$seed_file")
            COMPLETED=$(jq -r '.contracts_completed // 0' "$seed_file")
            FAILED=$(jq -r '.contracts_failed // 0' "$seed_file")
            ACTIONS=$(jq -r '.total_actions // 0' "$seed_file")

            echo "Game $GAME_NUM (Seed $SEED):"
            echo "  Max Tier: $MAX_TIER"
            echo "  Completed: $COMPLETED, Failed: $FAILED"
            echo "  Actions: $ACTIONS"
            echo ""

            ((GAME_NUM++))
        fi
    done

    echo "========================================"
    echo "Batch Results Location: $RESULTS_DIR"
    echo "JSON results saved for each seed"
    echo "========================================"
else
    echo "WARNING: No results found in $RESULTS_DIR"
    exit 1
fi
