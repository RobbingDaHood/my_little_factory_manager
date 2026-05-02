#!/bin/bash
# SmartStrategy Distributed Seed Testing - Complete Agent Script
# Copy and paste this entire script to run a complete batch of 12 seeds

set -e

echo "================================================================================"
echo "SmartStrategy Distributed Seed Testing - Complete Agent Script"
echo "================================================================================"
echo ""
echo "Main Issue: https://github.com/RobbingDaHood/my_little_factory_manager/issues/83"
echo "Branch: claude/test-smart-strategy-seeds-EqR4p"
echo ""

# ================================================================================
# STEP 1: Clone and Setup
# ================================================================================

echo "STEP 1: Cloning repository..."
WORK_DIR="/tmp/smart_strategy_agent_$$"
mkdir -p "$WORK_DIR"
cd "$WORK_DIR"

git clone https://github.com/RobbingDaHood/my_little_factory_manager.git
cd my_little_factory_manager
git checkout claude/test-smart-strategy-seeds-EqR4p

echo "✓ Repository cloned and branch checked out"
echo ""

# ================================================================================
# STEP 2: Generate UUID and Run Tests
# ================================================================================

echo "STEP 2: Generating unique UUID and running tests..."
AGENT_UUID=$(python3 -c "import uuid; print(uuid.uuid4())")
echo "Using UUID: $AGENT_UUID"
echo ""

export SMART_STRATEGY_UUID="$AGENT_UUID"
export SMART_STRATEGY_OUTPUT="/tmp/results_${AGENT_UUID}.jsonl"

echo "Running: cargo test --features simulation --test simulation smart_strategy_test_parallel_seeds"
echo "This will take 15-45 minutes..."
echo ""

cargo test --features simulation --test simulation smart_strategy_test_parallel_seeds -- --include-ignored --nocapture

if [ ! -f "$SMART_STRATEGY_OUTPUT" ]; then
    echo "ERROR: Results file not created at $SMART_STRATEGY_OUTPUT"
    exit 1
fi

echo ""
echo "✓ Test completed. Results saved to: $SMART_STRATEGY_OUTPUT"
echo ""

# ================================================================================
# STEP 3: Create Sub-Issues
# ================================================================================

echo "STEP 3: Creating sub-issues on GitHub..."
echo ""

python3 << 'PYTHON_EOF'
import json
import subprocess
import os
import sys

AGENT_UUID = os.environ.get("SMART_STRATEGY_UUID", "unknown")
results_file = os.environ.get("SMART_STRATEGY_OUTPUT", f"/tmp/results_{AGENT_UUID}.jsonl")

if not os.path.exists(results_file):
    print(f"ERROR: Results file not found: {results_file}")
    sys.exit(1)

print(f"Creating sub-issues from {results_file}...")
print()

issue_count = 0
with open(results_file) as f:
    for i, line in enumerate(f, 1):
        result = json.loads(line)
        seed = result["seed"]
        tier = result.get("max_tier_reached", "?")
        completed = result.get("contracts_completed", 0)
        failed = result.get("contracts_failed", 0)
        actions = result.get("total_actions", 0)

        title = f"Seed {seed}: Tier {tier}"
        body = f"""**Seed:** {seed}
**Max Tier:** {tier}
**Contracts Completed:** {completed}
**Contracts Failed:** {failed}
**Total Actions:** {actions:,}
**Stuck:** {result.get('stuck', False)}
**Hit Action Limit:** {result.get('hit_action_limit', False)}

**Failure Reasons:** {dict(result.get('failure_reasons', {}))}
"""

        result_obj = subprocess.run([
            "gh", "issue", "create",
            "--title", title,
            "--body", body,
            "--repo", "robbingdahood/my_little_factory_manager"
        ], capture_output=True, text=True)

        if result_obj.returncode == 0:
            issue_url = result_obj.stdout.strip()
            issue_num = int(issue_url.split('/')[-1])
            print(f"[{i:2d}/12] ✓ Created sub-issue #{issue_num} for seed {seed}")
            issue_count += 1
        else:
            print(f"[{i:2d}/12] ✗ ERROR creating sub-issue for seed {seed}")
            print(f"         {result_obj.stderr}")

print()
print(f"✓ Done! Created {issue_count} sub-issues.")
PYTHON_EOF

echo ""
echo "================================================================================"
echo "✓ COMPLETE! All 12 seeds tested and sub-issues created."
echo "================================================================================"
echo ""
echo "Summary:"
echo "  UUID: $AGENT_UUID"
echo "  Results file: $SMART_STRATEGY_OUTPUT"
echo "  Work directory: $WORK_DIR"
echo ""
echo "Main issue: https://github.com/RobbingDaHood/my_little_factory_manager/issues/83"
echo ""
