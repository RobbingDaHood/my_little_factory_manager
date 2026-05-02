# SmartStrategy Distributed Seed Testing Framework

This document describes the distributed testing framework for running SmartStrategy simulations across multiple randomized seeds.

## Overview

The framework enables:
- Running 12 (or more) simulation games with unique randomized seeds
- Executing games in parallel batches (4 at a time by default)
- Capturing detailed performance metrics in JSON format
- Generating GitHub sub-issues with results linked to main issue #83

## Quick Start

### Fast Demo (Recommended for Testing)

Run a quick demonstration with 3 seeds and reduced action limits:

```bash
bash QUICK_TEST.sh
```

Execution time: ~5-15 minutes depending on system performance
- Runs 3 games sequentially
- 100,000 actions per game (reduced from 500,000)
- Produces JSON results in `batch_results_fast_<batch_id>/`

### Full Distributed Test

Run the complete 12-seed distributed testing campaign:

```bash
bash AGENT_SCRIPT.sh
```

Execution time: ~1-3 hours (highly variable based on system performance)
- Runs 12 games with unique seeds
- 500,000 actions per game
- Executes 4 games in parallel (3 batches)
- Produces JSON results in `batch_results_<batch_id>/`

## Scripts and Tools

### AGENT_SCRIPT.sh
Main distributed testing orchestrator.

**Features:**
- Generates unique batch ID for tracking
- Compiles the project with simulation feature
- Exports environment variables for the batch runner test
- Runs all 12 games via `cargo test --features simulation`
- Parses results using `jq` and displays summary

**Environment Variables:**
- `BATCH_SEEDS`: Comma-separated seed values (default: 12 seeds)
- `BATCH_OUTPUT_DIR`: Output directory for results (default: ./batch_results_<batch_id>)
- `BATCH_MAX_ACTIONS`: Max actions per game (default: 500000)

### QUICK_TEST.sh
Fast demonstration script for validation and iteration.

**Features:**
- Uses reduced action limits (100,000) for faster execution
- Tests with 3 seeds for quick validation
- Shows timing and results summary
- Good for testing framework changes

### create_github_issues.sh
Utility script to parse batch results and prepare GitHub sub-issue creation.

**Usage:**
```bash
bash create_github_issues.sh <batch_id> <results_directory>
```

**Output:**
Formatted issue bodies with:
- Performance metrics table
- Full JSON results
- Links to main issue #83

## Test Infrastructure

### Simulation Tests

#### smart_strategy_batch_runner
Runs 12 games with full 500,000 action limit per game.

```bash
cargo test --features simulation --test simulation smart_strategy_batch_runner \
    -- --nocapture --include-ignored
```

Control via environment variables:
- `BATCH_SEEDS`: Seed list (required)
- `BATCH_OUTPUT_DIR`: Output directory (required)
- `BATCH_MAX_ACTIONS`: Actions per game (optional, default: 500000)

#### smart_strategy_batch_runner_fast
Runs games with 100,000 action limit for faster testing.

```bash
cargo test --features simulation --test simulation smart_strategy_batch_runner_fast \
    -- --nocapture --include-ignored
```

Control via environment variables:
- `BATCH_SEEDS_FAST`: Seed list (optional, default: 3 seeds)
- `BATCH_OUTPUT_DIR_FAST`: Output directory (optional, default: ./batch_results_fast)

### Batch Runner Modules

#### batch_runner.rs
Core module for running full-scale simulations.

Key exports:
- `run_single_game(seed, max_actions) -> GameResult`
- `run_batch(seeds, output_dir, max_actions) -> Result<()>`

#### batch_runner_fast.rs
Lightweight version with reduced action limits.

Key exports:
- `run_single_game_fast(seed) -> GameResult`
- `run_batch_fast(seeds, output_dir) -> Result<()>`

## Results Format

Each completed game produces a JSON file: `seed_<seed_value>.json`

### Example JSON Result Structure

```json
{
  "seed": 12345678,
  "milestones": [
    {
      "tier": 10,
      "actions_to_reach": 1234
    },
    {
      "tier": 20,
      "actions_to_reach": 5678
    }
  ],
  "max_tier_reached": 35,
  "total_actions": 123456,
  "contracts_completed": 42,
  "contracts_failed": 3,
  "contracts_abandoned": 1,
  "failure_reasons": {
    "HarmfulTokenLimitExceeded": 2,
    "TurnWindowExceeded": 1
  },
  "completed_per_tier": {
    "10": 5,
    "20": 4,
    "30": 3,
    "35": 1
  },
  "failed_per_tier": {
    "40": 1,
    "41": 2
  },
  "abandoned_per_tier": {},
  "hit_action_limit": false,
  "stuck": false
}
```

### Key Metrics

- **max_tier_reached**: Highest contract tier completed
- **total_actions**: Total game actions taken
- **contracts_completed**: Number of contracts successfully completed
- **contracts_failed**: Number of contracts failed (including abandoned)
- **hit_action_limit**: Whether the game hit the action budget
- **stuck**: Whether the game became stuck (no valid actions available)

## Workflow

### 1. Prepare
```bash
# Verify the code compiles
cargo build --features simulation --tests
```

### 2. Run Tests
```bash
# Option A: Quick validation
bash QUICK_TEST.sh

# Option B: Full distributed campaign
bash AGENT_SCRIPT.sh
```

### 3. Analyze Results
```bash
# List all results
ls batch_results_*/seed_*.json

# View a specific result
cat batch_results_e1ce88ed/seed_12345678.json | jq .

# Extract max tiers
jq -r '.max_tier_reached' batch_results_*/seed_*.json
```

### 4. Create Sub-Issues
```bash
# Prepare issue data (not yet integrated with GitHub)
bash create_github_issues.sh <batch_id> <results_directory>

# Results are formatted and ready for manual GitHub issue creation
# or integration with GitHub MCP tools
```

## Performance Characteristics

### Simulation Game Duration

Times vary significantly based on system performance:

- **Tier 10**: 100-500 actions, ~0.1-0.5 seconds
- **Tier 20**: 1,000-5,000 actions, ~1-5 seconds
- **Tier 30**: 5,000-20,000 actions, ~5-20 seconds
- **Tier 40**: 20,000-100,000 actions, ~20-100 seconds
- **Tier 50**: 100,000-500,000 actions, ~100-500 seconds

### Memory Usage

- Cargo compilation: ~300-500 MB
- Simulation execution: ~50-100 MB per game

### Total Execution Time

- **QUICK_TEST.sh** (3 seeds, 100k actions): 5-15 minutes
- **AGENT_SCRIPT.sh** (12 seeds, 500k actions, parallel): 30 minutes - 2 hours

## Troubleshooting

### Test Hangs or Gets Stuck

If a test appears to hang:
1. Check if the cargo process is still running: `ps aux | grep cargo`
2. The "test running for over 60 seconds" message is a warning, not an error
3. Let the test continue unless CPU usage is 0% for extended periods
4. Kill the process if necessary: `pkill -f cargo`

### No Results Generated

Check the log output:
```bash
tail -50 batch_results_*/test_output.log
```

Common issues:
- Compilation errors: check `cargo build --features simulation`
- Timeout: increase the timeout value in the script
- Disk space: ensure sufficient space for output

### JSON Parsing Errors

Ensure `jq` is installed:
```bash
apt-get install jq
# or
brew install jq
```

## Integration with GitHub

The results JSON can be integrated with GitHub sub-issue creation:

1. Parse results with `create_github_issues.sh`
2. Use GitHub MCP tools or `gh` CLI to create issues
3. Link results to main issue #83

Example (with gh CLI):
```bash
gh issue create --title "SmartStrategy Seed 12345678 - Batch e1ce88ed" \
                --body "$(cat batch_results.md)"
```

## Extending the Framework

### Adding More Seeds

Modify the seed list in AGENT_SCRIPT.sh or export `BATCH_SEEDS`:

```bash
export BATCH_SEEDS="1,2,3,4,5,6,7,8,9,10,11,12,13,14,15"
bash AGENT_SCRIPT.sh
```

### Changing Action Limits

```bash
export BATCH_MAX_ACTIONS=1000000
bash AGENT_SCRIPT.sh
```

### Running Custom Seed Ranges

```bash
export BATCH_SEEDS="100001,100002,100003"
export BATCH_OUTPUT_DIR="custom_results"
cargo test --features simulation --test simulation smart_strategy_batch_runner \
    -- --nocapture --include-ignored
```

## Architecture Notes

The framework leverages existing game simulation infrastructure:
- `GameDriver`: Executes individual games with a given seed
- `SimulationRunner`: Aggregates results across multiple games
- `GameResult`: Serializable structure containing all performance metrics

Each game runs to completion or until hitting the action limit, with all results captured in JSON format for analysis and reporting.
