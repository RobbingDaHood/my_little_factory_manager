# SmartStrategy Distributed Seed Testing

**Main Issue:** https://github.com/RobbingDaHood/my_little_factory_manager/issues/83

## Quick Start

Clone the repo and run the agent script:

```bash
git clone https://github.com/RobbingDaHood/my_little_factory_manager.git
cd my_little_factory_manager
git checkout claude/test-smart-strategy-seeds-EqR4p
bash AGENT_SCRIPT.sh
```

That's it! The script will:
1. Generate a unique UUID for your batch
2. Run 12 seeds with unique randomized values (4 at a time × 3 batches)
3. Automatically create 12 GitHub sub-issues linked to the main issue
4. Takes 15-45 minutes total

## What Happens

- **Unique seeds:** Each agent gets completely unique seeds derived from their UUID
- **Parallel execution:** 4 seeds run in parallel on your 4 CPUs
- **Auto-linking:** All 12 sub-issues are created with detailed results
- **No coordination needed:** Just run the script, each agent works independently

## Requirements

- Git
- Rust/Cargo (with nightly toolchain)
- Python 3
- GitHub CLI (`gh`) with authentication token

## Result

12 new sub-issues on https://github.com/RobbingDaHood/my_little_factory_manager/issues/83, each with:
- Seed number and max tier reached
- Contracts completed/failed/abandoned
- Total actions executed
- Failure reasons
