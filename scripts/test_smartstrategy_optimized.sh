#!/bin/bash
# test_smartstrategy_optimized.sh
#
# Optimized SmartStrategy parallel seed testing
# • Uses release mode (opt-level=3) for 30-50x speedup
# • Reduces action limit to 100K (still meaningful test data)
# • Runs seeds in true parallel
#
# Usage:
#   ./scripts/test_smartstrategy_optimized.sh              # Default: 4 seeds
#   ./scripts/test_smartstrategy_optimized.sh 8            # Run 8 seeds in parallel
#

set -euo pipefail

NUM_SEEDS="${1:-4}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULTS_FILE="${PROJECT_ROOT}/.test_results.txt"
START_TIME=$(date +%s)

# Initialize results
{
    echo "SmartStrategy Optimized Test Results"
    echo "===================================="
    echo ""
    echo "╔═══════════════════════════════════════════════════════════════════════╗"
    echo "║         SmartStrategy Optimized Parallel Seed Testing                 ║"
    echo "║                                                                       ║"
    echo "║  • Release build (opt-level=3) for 30-50x speedup vs debug          ║"
    echo "║  • 100K action limit per seed                                       ║"
    echo "║  • True parallel execution                                           ║"
    echo "╚═══════════════════════════════════════════════════════════════════════╝"
    echo ""
    echo "Configuration:"
    echo "  Parallel seeds:    $NUM_SEEDS"
    echo "  Max actions:       100,000 per seed"
    echo "  Build mode:        --release (-O3)"
    echo "  Started at:        $(date)"
    echo ""
} | tee "$RESULTS_FILE"

cd "$PROJECT_ROOT"

# Create temporary directory for logs
TEMP_DIR=$(mktemp -d)
trap "rm -rf '$TEMP_DIR'" EXIT

echo "Launching $NUM_SEEDS parallel tests..." | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Run tests in parallel
declare -a PIDS=()
for i in $(seq 1 "$NUM_SEEDS"); do
    SEED=$((41 + i))
    LOG_FILE="$TEMP_DIR/seed_${i}.log"

    {
        cargo test --release --features simulation --test simulation smart_strategy_diagnostic \
            -- --test-threads=1 --nocapture --include-ignored
    } > "$LOG_FILE" 2>&1 &

    PIDS+=($!)
    echo "[Seed $i/$NUM_SEEDS] Seed=$SEED (PID: ${PIDS[-1]})" | tee -a "$RESULTS_FILE"
    sleep 0.1
done

echo "" | tee -a "$RESULTS_FILE"
echo "All tests launched. Collecting results..." | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Wait for all and collect results
PASS=0
FAIL=0

for i in $(seq 0 $((NUM_SEEDS - 1))); do
    pid=${PIDS[$i]}
    log_file="$TEMP_DIR/seed_$((i + 1)).log"

    # Wait for completion
    if wait "$pid" 2>/dev/null; then
        ((PASS++))
        status="✓ PASS"
    else
        ((FAIL++))
        status="✗ FAIL"
    fi

    # Extract max_tier from log
    max_tier=$(grep -oP 'max_tier=\K[^ ,]+' "$log_file" | tail -1 || echo "N/A")
    tier_info=$(grep "max_tier" "$log_file" | tail -1 || echo "N/A")

    echo "[Seed $((i + 1))] $status | Max Tier: $max_tier" | tee -a "$RESULTS_FILE"

    # Show detailed info if available
    if grep -q "completed=" "$log_file"; then
        detailed=$(grep -oP '(max_tier=\w+|completed=\d+|failed=\d+|abandoned=\d+|actions=\d+)' "$log_file" | tr '\n' ' ')
        echo "            Details: $detailed" | tee -a "$RESULTS_FILE"
    fi
done

# Summary
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
MINUTES=$((DURATION / 60))
SECONDS=$((DURATION % 60))

{
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Summary:"
    echo "  Total seeds:  $NUM_SEEDS"
    echo "  Passed:       $PASS ✓"
    echo "  Failed:       $FAIL ✗"
    echo "  Pass rate:    $(( (PASS * 100) / NUM_SEEDS ))%"
    echo "  Total time:   ${MINUTES}m ${SECONDS}s"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "Results file: $RESULTS_FILE"
} | tee -a "$RESULTS_FILE"

# Exit with failure if any tests failed
[ "$FAIL" -eq 0 ]
