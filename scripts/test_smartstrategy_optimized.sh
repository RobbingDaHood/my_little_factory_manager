#!/bin/bash
# test_smartstrategy_optimized.sh
#
# Optimized SmartStrategy parallel seed testing (implements issue #112).
#
# Eliminates the sequential batch bottleneck by launching every seed immediately
# into its own process with a unique UUID-derived game seed.  All N seeds run
# concurrently; total wall-clock time ≈ the slowest single game.
#
# Results are optionally posted to GitHub issue #115 when gh CLI is available.
#
# Usage:
#   ./scripts/test_smartstrategy_optimized.sh              # default: 4 seeds
#   ./scripts/test_smartstrategy_optimized.sh 8            # 8 seeds
#   NUM_SEEDS=12 MAX_ACTIONS=100000 ./scripts/test_smartstrategy_optimized.sh

set -euo pipefail

# ===== Configuration =====
NUM_SEEDS="${1:-${NUM_SEEDS:-4}}"
MAX_ACTIONS_PER_SEED="${MAX_ACTIONS:-100000}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GITHUB_ISSUE=115
GITHUB_REPO="RobbingDaHood/my_little_factory_manager"
START_TIME=$(date +%s)
HOSTNAME="${HOSTNAME:-unknown-host}"

TEMP_DIR=$(mktemp -d)
trap "rm -rf '$TEMP_DIR'" EXIT

# ===== Header =====
cat << 'EOF'
╔═══════════════════════════════════════════════════════════════════════╗
║         SmartStrategy Optimised Parallel Seed Testing                 ║
║                                                                       ║
║  • Release build (opt-level=3) for 30-50x speedup vs debug           ║
║  • Configurable action limit per seed                                 ║
║  • True parallel execution — all seeds start immediately              ║
║  • Unique UUID-derived seed per run (not the same game repeated)      ║
╚═══════════════════════════════════════════════════════════════════════╝
EOF
echo ""
echo "Configuration:"
echo "  Parallel seeds:       $NUM_SEEDS"
echo "  Max actions per seed: $MAX_ACTIONS_PER_SEED"
echo "  Build mode:           --release"
echo "  Host:                 $HOSTNAME"
echo "  Started:              $(date)"
echo ""

cd "$PROJECT_ROOT"

# ===== Launch all seeds in parallel =====
declare -a PIDS=()
declare -a SEEDS=()
declare -a LOGS=()

for seed_num in $(seq 1 "$NUM_SEEDS"); do
    # Generate a unique string identifier for this seed.
    # Each parallel invocation gets a distinct UUID → distinct u64 game seed.
    SEED_UUID=$(uuidgen 2>/dev/null || echo "seed-${seed_num}-${RANDOM}-${RANDOM}")

    LOG_FILE="$TEMP_DIR/seed_${seed_num}.log"
    SEEDS+=("$SEED_UUID")
    LOGS+=("$LOG_FILE")

    printf "[Seed %d/%d] Starting UUID: %s\n" "$seed_num" "$NUM_SEEDS" "$SEED_UUID"

    # Each subshell exports its own SEED_UUID and MAX_ACTIONS so the Rust test
    # (smart_strategy_seed) picks them up via std::env::var.
    (
        export SEED_UUID="$SEED_UUID"
        export MAX_ACTIONS="$MAX_ACTIONS_PER_SEED"
        cd "$PROJECT_ROOT"
        timeout 600 cargo test \
            --release \
            --features simulation \
            --test simulation \
            smart_strategy_seed \
            -- --test-threads=1 --nocapture --include-ignored \
            2>&1
    ) > "$LOG_FILE" 2>&1 &

    PIDS+=($!)

    # Tiny stagger avoids thundering-herd on cargo's target directory lock.
    sleep 0.2
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Tests executing. Waiting for completion..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# ===== Collect results as each seed finishes =====
PASS=0
FAIL=0
declare -a RESULTS=()

for seed_num in $(seq 0 $((NUM_SEEDS - 1))); do
    pid="${PIDS[$seed_num]}"
    seed_uuid="${SEEDS[$seed_num]}"
    log_file="${LOGS[$seed_num]}"

    if wait "$pid" 2>/dev/null; then
        PASS=$((PASS + 1))
        status="PASS"
    else
        FAIL=$((FAIL + 1))
        status="FAIL"
    fi

    # Parse metrics from the eprintln! output of smart_strategy_seed.
    tier=$(grep -oP '(?<=max_tier=)\d+' "$log_file" | tail -1 || echo "N/A")
    completed=$(grep -oP '(?<=completed=)\d+' "$log_file" | tail -1 || echo "N/A")
    actions=$(grep -oP '(?<=actions=)\d+' "$log_file" | tail -1 || echo "N/A")

    result_line="[Seed $((seed_num + 1))] $status | UUID: $seed_uuid | Tier: $tier | Contracts: $completed | Actions: $actions"
    RESULTS+=("$result_line")
    echo "$result_line"
done

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
MINUTES=$((DURATION / 60))
SECS=$((DURATION % 60))

# ===== Summary =====
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Total:      $NUM_SEEDS"
echo "  Passed:     $PASS"
echo "  Failed:     $FAIL"
echo "  Duration:   ${MINUTES}m ${SECS}s"
if [ "$NUM_SEEDS" -gt 0 ]; then
    echo "  Pass rate:  $(( (PASS * 100) / NUM_SEEDS ))%"
fi
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# ===== Optional GitHub result posting =====
if command -v gh &> /dev/null; then
    echo "Posting results to issue #$GITHUB_ISSUE..."

    COMMENT="## Test Results - $(date)

**Environment**: $HOSTNAME
**Build Mode**: Release (-O3)
**Seeds**: $NUM_SEEDS (unique UUID per run)
**Duration**: ${MINUTES}m ${SECS}s

### Results
\`\`\`
"
    for result in "${RESULTS[@]}"; do
        COMMENT+="$result"$'\n'
    done

    COMMENT+="
\`\`\`

**Summary**: Passed $PASS/$NUM_SEEDS ($(( (PASS * 100) / NUM_SEEDS ))%)

**Configuration**:
- Build: \`cargo test --release --features simulation\`
- Max Actions: $MAX_ACTIONS_PER_SEED per seed
- Parallelism: $NUM_SEEDS seeds concurrent
"

    gh issue comment "$GITHUB_ISSUE" --repo "$GITHUB_REPO" --body "$COMMENT" 2>/dev/null || {
        echo "Could not post to GitHub (gh CLI error)"
    }
else
    echo "GitHub CLI (gh) not found — results not posted to issue #$GITHUB_ISSUE"
fi

echo "Test run complete"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
