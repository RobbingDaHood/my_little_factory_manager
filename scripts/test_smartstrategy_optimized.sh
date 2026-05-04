#!/bin/bash
# test_smartstrategy_optimized.sh
#
# Parallel benchmark of SmartAggressive and SmartCareful strategies.
#
# By default runs 6 unique seeds against EACH strategy (12 total games),
# all in parallel. Each seed UUID is paired across strategies, so the two
# strategies are evaluated on the same RNG seeds for direct comparison.
#
# Wall-clock time ≈ the slowest single game.
#
# Usage:
#   ./scripts/test_smartstrategy_optimized.sh                       # default: 6 seeds per strategy
#   ./scripts/test_smartstrategy_optimized.sh 4                     # 4 seeds per strategy (8 games)
#   NUM_SEEDS=12 MAX_ACTIONS=200000 ./scripts/test_smartstrategy_optimized.sh

set -euo pipefail

# ===== Configuration =====
NUM_SEEDS="${1:-${NUM_SEEDS:-6}}"
MAX_ACTIONS_PER_SEED="${MAX_ACTIONS:-100000}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
START_TIME=$(date +%s)
HOSTNAME="${HOSTNAME:-unknown-host}"
STRATEGIES=(smart_aggressive smart_careful)

TEMP_DIR=$(mktemp -d)
trap "rm -rf '$TEMP_DIR'" EXIT

# ===== Header =====
cat << 'EOF'
╔═══════════════════════════════════════════════════════════════════════╗
║  Strategy Benchmark — SmartAggressive vs SmartCareful                 ║
║                                                                       ║
║  • Each seed UUID is run against BOTH strategies for paired compare   ║
║  • All games launched in parallel; total time ≈ slowest single game   ║
║  • Reports per-strategy progression: actions-to-reach tier 5/10/.../50║
╚═══════════════════════════════════════════════════════════════════════╝
EOF
echo ""
echo "Configuration:"
echo "  Seeds per strategy:   $NUM_SEEDS"
echo "  Strategies:           ${STRATEGIES[*]}"
echo "  Total games:          $((NUM_SEEDS * ${#STRATEGIES[@]}))"
echo "  Max actions per game: $MAX_ACTIONS_PER_SEED"
echo "  Build mode:           --release"
echo "  Host:                 $HOSTNAME"
echo "  Started:              $(date)"
echo ""

cd "$PROJECT_ROOT"

# ===== Generate one UUID per seed slot, shared across strategies =====
declare -a SEED_UUIDS=()
for seed_num in $(seq 1 "$NUM_SEEDS"); do
    SEED_UUIDS+=("$(uuidgen 2>/dev/null || echo "seed-${seed_num}-${RANDOM}-${RANDOM}")")
done

# ===== Pre-build to avoid concurrent target-dir contention =====
echo "Building simulation binary (release)..."
cargo test --release --features simulation --test simulation --no-run 2>&1 | tail -2
echo ""

# ===== Launch all (strategy × seed) combinations in parallel =====
declare -a PIDS=()
declare -a LABELS=()
declare -a LOG_PATHS=()

for strategy in "${STRATEGIES[@]}"; do
    test_name="${strategy}_seed"
    for seed_num in $(seq 1 "$NUM_SEEDS"); do
        idx=$((seed_num - 1))
        seed_uuid="${SEED_UUIDS[$idx]}"
        log_file="$TEMP_DIR/${strategy}_seed${seed_num}.log"
        label="${strategy} #${seed_num} [${seed_uuid:0:8}]"

        printf "Launching %s\n" "$label"

        (
            export SEED_UUID="$seed_uuid"
            export MAX_ACTIONS="$MAX_ACTIONS_PER_SEED"
            cd "$PROJECT_ROOT"
            timeout 1200 cargo test \
                --release \
                --features simulation \
                --test simulation \
                "$test_name" \
                -- --test-threads=1 --nocapture --include-ignored \
                2>&1
        ) > "$log_file" 2>&1 &

        PIDS+=($!)
        LABELS+=("$label")
        LOG_PATHS+=("$log_file")

        # Tiny stagger avoids cargo target-dir lock contention.
        sleep 0.1
    done
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Tests executing. Waiting for completion..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ===== Wait for all =====
declare -a STATUSES=()
for i in "${!PIDS[@]}"; do
    if wait "${PIDS[$i]}" 2>/dev/null; then
        STATUSES+=("PASS")
    else
        STATUSES+=("FAIL")
    fi
done

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
MINUTES=$((DURATION / 60))
SECS=$((DURATION % 60))

# ===== Helpers to parse a log file =====
parse_summary_field() {
    # parse_summary_field <log_file> <field>
    # Empty stdout if field not present (so callers can handle missing values).
    { grep -m1 -oE "$2=[A-Za-z0-9.]+" "$1" 2>/dev/null || true; } \
        | head -1 | sed -E "s/^$2=//" || true
}

parse_milestone_actions() {
    # parse_milestone_actions <log_file> <tier>
    # Empty stdout when the milestone wasn't reached (line shows actions=N/A).
    { grep -E "^MILESTONE tier=$2 " "$1" 2>/dev/null || true; } | head -1 \
        | { grep -oE "actions=[0-9]+" || true; } \
        | sed 's/^actions=//' || true
}

# ===== Per-strategy per-seed table =====
echo ""
for strategy in "${STRATEGIES[@]}"; do
    echo ""
    echo "┌─────────────────────────────────────────────────────────────────────────┐"
    printf "│  %-71s │\n" "Strategy: $strategy"
    echo "├─────────────────────────────────────────────────────────────────────────┤"
    printf "│ %-2s %-10s %-7s %-9s %-7s %-9s %-7s │\n" \
        "##" "uuid" "max_t" "completed" "failed" "abandoned" "actions"
    echo "├─────────────────────────────────────────────────────────────────────────┤"

    for seed_num in $(seq 1 "$NUM_SEEDS"); do
        log="$TEMP_DIR/${strategy}_seed${seed_num}.log"
        seed_uuid="${SEED_UUIDS[$((seed_num - 1))]}"
        max_t=$(parse_summary_field "$log" max_tier)
        completed=$(parse_summary_field "$log" completed)
        failed=$(parse_summary_field "$log" failed)
        abandoned=$(parse_summary_field "$log" abandoned)
        actions=$(parse_summary_field "$log" actions)
        printf "│ %-2s %-10s %-7s %-9s %-7s %-9s %-7s │\n" \
            "$seed_num" "${seed_uuid:0:8}" "${max_t:-?}" "${completed:-?}" "${failed:-?}" "${abandoned:-?}" "${actions:-?}"
    done
    echo "└─────────────────────────────────────────────────────────────────────────┘"
done

# ===== Per-strategy progression summary =====
echo ""
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Progression: mean actions to first reach each tier (across $NUM_SEEDS seeds)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

MILESTONE_TIERS=(5 10 15 20 25 30 35 40 45 50)
printf "  %-12s" "Tier"
for strategy in "${STRATEGIES[@]}"; do
    printf "  %-30s" "$strategy"
done
echo ""

for tier in "${MILESTONE_TIERS[@]}"; do
    printf "  %-12s" "$tier"
    for strategy in "${STRATEGIES[@]}"; do
        sum=0
        count=0
        for seed_num in $(seq 1 "$NUM_SEEDS"); do
            log="$TEMP_DIR/${strategy}_seed${seed_num}.log"
            actions=$(parse_milestone_actions "$log" "$tier")
            if [ -n "$actions" ]; then
                sum=$((sum + actions))
                count=$((count + 1))
            fi
        done
        if [ "$count" -gt 0 ]; then
            mean=$((sum / count))
            printf "  %-30s" "$(printf '%d actions (%d/%d games)' "$mean" "$count" "$NUM_SEEDS")"
        else
            printf "  %-30s" "N/A (0/$NUM_SEEDS games)"
        fi
    done
    echo ""
done

# ===== Per-strategy aggregate stats =====
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Aggregate (sum / mean across $NUM_SEEDS seeds)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

for strategy in "${STRATEGIES[@]}"; do
    total_completed=0
    total_failed=0
    total_abandoned=0
    sum_max_tier=0
    max_tier_overall=0
    for seed_num in $(seq 1 "$NUM_SEEDS"); do
        log="$TEMP_DIR/${strategy}_seed${seed_num}.log"
        c=$(parse_summary_field "$log" completed)
        f=$(parse_summary_field "$log" failed)
        a=$(parse_summary_field "$log" abandoned)
        m=$(parse_summary_field "$log" max_tier)
        total_completed=$((total_completed + ${c:-0}))
        total_failed=$((total_failed + ${f:-0}))
        total_abandoned=$((total_abandoned + ${a:-0}))
        sum_max_tier=$((sum_max_tier + ${m:-0}))
        if [ "${m:-0}" -gt "$max_tier_overall" ]; then
            max_tier_overall="${m:-0}"
        fi
    done
    mean_max_tier=$((sum_max_tier / NUM_SEEDS))

    echo ""
    echo "  $strategy:"
    echo "    mean max_tier across seeds: $mean_max_tier"
    echo "    best max_tier across seeds: $max_tier_overall"
    echo "    sum completed: $total_completed"
    echo "    sum failed:    $total_failed"
    echo "    sum abandoned: $total_abandoned"
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Run summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
PASS_COUNT=$(printf '%s\n' "${STATUSES[@]}" | grep -c "^PASS$" || true)
FAIL_COUNT=$(printf '%s\n' "${STATUSES[@]}" | grep -c "^FAIL$" || true)
echo "  Total games:  ${#PIDS[@]}"
echo "  Passed:       $PASS_COUNT"
echo "  Failed:       $FAIL_COUNT"
echo "  Duration:     ${MINUTES}m ${SECS}s"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Detail output for failed runs (helpful for debugging).
if [ "$FAIL_COUNT" -gt 0 ]; then
    echo "Failed runs:"
    for i in "${!PIDS[@]}"; do
        if [ "${STATUSES[$i]}" = "FAIL" ]; then
            echo "  ${LABELS[$i]} — log: ${LOG_PATHS[$i]}"
            tail -20 "${LOG_PATHS[$i]}" | sed 's/^/    /'
        fi
    done
fi

if [ "$FAIL_COUNT" -eq 0 ]; then
    exit 0
else
    exit 1
fi
