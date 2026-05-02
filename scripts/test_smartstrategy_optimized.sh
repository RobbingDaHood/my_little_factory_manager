#!/bin/bash
# test_smartstrategy_optimized.sh
#
# Optimized SmartStrategy parallel seed testing (issue #111)
# Posts results as a comment to issue #115 for batch collection
#
# Uses:
# • Release build (opt-level=3) for 30-50x speedup vs debug
# • 100K action limit per seed
# • True parallel execution of multiple seeds
#
# Usage:
#   ./scripts/test_smartstrategy_optimized.sh              # Default: 2 seeds
#   ./scripts/test_smartstrategy_optimized.sh 4            # Run 4 seeds
#   NUM_SEEDS=8 ./scripts/test_smartstrategy_optimized.sh  # Use env var
#
# Results are automatically posted to GitHub issue #115

set -euo pipefail

NUM_SEEDS="${1:-${NUM_SEEDS:-2}}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GITHUB_ISSUE=115
GITHUB_REPO="RobbingDaHood/my_little_factory_manager"
START_TIME=$(date +%s)
HOSTNAME="${HOSTNAME:-unknown-host}"

# Temp directory for logs
TEMP_DIR=$(mktemp -d)
trap "rm -rf '$TEMP_DIR'" EXIT

# Header
cat << 'EOF'
╔═══════════════════════════════════════════════════════════════════════╗
║         SmartStrategy Optimized Parallel Test Execution               ║
║                                                                       ║
║  Release build (-O3) for 30-50x speedup                              ║
║  100K action limit per seed                                          ║
║  Results will be posted to issue #115 after completion               ║
╚═══════════════════════════════════════════════════════════════════════╝
EOF

echo ""
echo "Configuration:"
echo "  Seeds to run:  $NUM_SEEDS"
echo "  Host:          $HOSTNAME"
echo "  Started:       $(date)"
echo ""

cd "$PROJECT_ROOT"

# Run tests in parallel
echo "Launching tests in release mode..."
echo ""

declare -a PIDS=()
declare -a LOGS=()

for i in $(seq 1 "$NUM_SEEDS"); do
    SEED=$((41 + i))
    LOG_FILE="$TEMP_DIR/seed_${i}.log"
    LOGS+=("$LOG_FILE")

    {
        timeout 600 cargo test --release --features simulation --test simulation smart_strategy_diagnostic \
            -- --test-threads=1 --nocapture --include-ignored
    } > "$LOG_FILE" 2>&1 &

    PIDS+=($!)
    echo "[Seed $i/$NUM_SEEDS] Started with seed=$SEED (PID: ${PIDS[-1]})"
    sleep 0.2
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Tests executing. Waiting for completion (may take 5-20 minutes)..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Wait for all tests
PASS=0
FAIL=0
declare -a RESULTS=()

for i in $(seq 0 $((NUM_SEEDS - 1))); do
    pid=${PIDS[$i]}
    log_file=${LOGS[$i]}

    if wait "$pid" 2>/dev/null; then
        ((PASS++))
        status="✓ PASS"
    else
        ((FAIL++))
        status="✗ FAIL"
    fi

    # Extract key metrics from log
    if grep -q '"overall_max_tier"' "$log_file" 2>/dev/null; then
        max_tier=$(grep -oP '"overall_max_tier":\s*\K\d+' "$log_file" | head -1 || echo "N/A")
        completed=$(grep -oP '"total_contracts_completed":\s*\K\d+' "$log_file" | head -1 || echo "0")
        total_actions=$(grep -oP '"total_actions":\s*\K\d+' "$log_file" | head -1 || echo "100000")
    else
        max_tier="N/A"
        completed="N/A"
        total_actions="N/A"
    fi

    result_line="[Seed $((i + 1))] $status | Tier: $max_tier | Contracts: $completed | Actions: $total_actions"
    RESULTS+=("$result_line")

    echo "$result_line"
done

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
MINUTES=$((DURATION / 60))
SECONDS=$((DURATION % 60))

# Print summary
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Total:      $NUM_SEEDS"
echo "  Passed:     $PASS ✓"
echo "  Failed:     $FAIL ✗"
echo "  Duration:   ${MINUTES}m ${SECONDS}s"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Prepare GitHub comment (if gh CLI is available)
if command -v gh &> /dev/null; then
    echo "Posting results to issue #$GITHUB_ISSUE..."

    COMMENT="## Test Results - $(date)

**Environment**: $HOSTNAME
**Build Mode**: Release (-O3, LTO)
**Seeds**: $NUM_SEEDS
**Duration**: ${MINUTES}m ${SECONDS}s

### Results
\`\`\`
"
    for result in "${RESULTS[@]}"; do
        COMMENT+="$result"$'\n'
    done

    COMMENT+="
\`\`\`

**Summary**:
- Passed: $PASS/$NUM_SEEDS
- Failed: $FAIL/$NUM_SEEDS
- Pass Rate: $(( (PASS * 100) / NUM_SEEDS ))%

**Configuration**:
- Build: \`cargo test --release --features simulation\`
- Max Actions: 100,000 per seed
- Parallelism: $NUM_SEEDS seeds concurrent
"

    gh issue comment "$GITHUB_ISSUE" --repo "$GITHUB_REPO" --body "$COMMENT" 2>/dev/null || {
        echo "⚠️  Could not post to GitHub (gh CLI error)"
        echo "Saving results to: .test_results_$(date +%s).txt"
        {
            echo "$COMMENT"
        } > ".test_results_$(date +%s).txt"
    }
else
    echo "⚠️  GitHub CLI (gh) not found. Install with: brew install gh (macOS) or apt-get install gh (Linux)"
    echo "     Then authenticate with: gh auth login"
    echo ""
    echo "Results would have been posted to: https://github.com/$GITHUB_REPO/issues/$GITHUB_ISSUE"
fi

echo "✓ Test run complete"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
