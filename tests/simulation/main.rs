//! Simulation test suite — strategy-driven automated gameplay.
//!
//! Compile and run with:
//!   cargo test --features simulation --test simulation -- --nocapture
//!
//! These tests are intentionally opt-in and not part of the standard `make check`
//! run because each game can require tens of thousands of actions.

#![cfg(feature = "simulation")]
#![allow(dead_code)]

mod game_driver;
mod runner;
mod strategies;

use runner::{SimulationConfig, SimulationRunner};
use strategies::smart_strategy::SmartStrategy;
use strategies::Strategy;

/// Fast diagnostic run with finer-grained milestones — used for tuning iterations.
/// Action budget is intentionally small so iteration is quick; raise it locally
/// if you need to confirm a tuning change reaches a higher tier in absolute terms.
#[ignore = "diagnostic-only fast simulation; run with --include-ignored or by name"]
#[test]
fn smart_strategy_diagnostic() {
    let strategy = SmartStrategy::new();
    let config = SimulationConfig {
        games_per_strategy: 1,
        base_seed: 42,
        max_actions_per_game: 100_000,
        milestone_tiers: vec![10, 15, 20, 25, 30, 35, 40, 45, 50],
    };

    let runner = SimulationRunner::new(config);
    let report = runner.run_strategy(&strategy);

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report serialisable")
    );
    eprintln!("=== Milestones ===");
    for milestone in &report.milestones {
        eprintln!(
            "Tier {:2}: reach {:.0}% (mean actions: {:?})",
            milestone.tier,
            milestone.reach_rate * 100.0,
            milestone.mean_actions,
        );
    }
    eprintln!(
        "Max tier: {:?}, completed: {}, failed: {}, abandoned: {}",
        report.overall_max_tier,
        report.total_contracts_completed,
        report.total_contracts_failed,
        report.total_contracts_abandoned,
    );
    eprintln!("Top failures: {:?}", report.top_failure_reasons);
}

/// Runs SmartStrategy across multiple seeds and asserts that tier 50 is reached.
///
/// SmartStrategy uses full game state to make informed decisions:
///   - Deckbuilds between contracts (replaces weak cards with reward cards)
///   - Scores playable cards against active contract requirements
///   - Avoids cards that would push harmful tokens past their limits
///   - Selects the contract with the best reward card and most feasible requirements
///
/// Hard assertions (test failure):
///   - At least one game reaches tier 50 (proves a winning path exists)
///
/// Soft observations (printed, not asserted):
///   - Milestone reach rates and mean action counts for tiers 10/20/30/40/50
///   - Dominant failure reasons at the tier where progress stalls
#[ignore = "expensive strategy simulation; run with --include-ignored or by name"]
#[test]
fn smart_strategy_reaches_tier_50() {
    let strategy = SmartStrategy::new();
    let config = SimulationConfig {
        games_per_strategy: 1,
        base_seed: 42,
        max_actions_per_game: 500_000,
        milestone_tiers: vec![10, 20, 30, 40, 50],
    };

    let runner = SimulationRunner::new(config);
    let report = runner.run_strategy(&strategy);

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report serialisable")
    );

    for milestone in &report.milestones {
        eprintln!(
            "Milestone tier {}: reach rate {:.0}% (mean actions: {:?})",
            milestone.tier,
            milestone.reach_rate * 100.0,
            milestone.mean_actions,
        );
    }

    assert!(
        report.overall_max_tier.is_some_and(|t| t >= 50),
        "SmartStrategy must reach tier 50 to confirm a winning path exists.\n\
         Max tier reached: {:?}\n\
         Top failure reasons: {:?}\n\
         Milestones: {:?}",
        report.overall_max_tier,
        report.top_failure_reasons,
        report.milestones,
    );
}

/// Runs SmartStrategy across N parallel seeds (where N = 3 * num_cpus).
/// Each agent runs this independently with a different BASE_SEED to test different seed batches.
///
/// Environment variables:
///   - SMART_STRATEGY_BASE_SEED: Starting seed (default 1000)
///   - SMART_STRATEGY_NUM_SEEDS: Number of seeds to test (default 3*nproc)
///   - SMART_STRATEGY_OUTPUT: Output JSON file path (default /tmp/smart_strategy_seeds_results.json)
///   - GITHUB_ISSUE_ID: GitHub main issue ID to link sub-issues to
///
/// Outputs JSON with individual GameResult for each seed, one per line (JSONL format for easy processing).
#[ignore = "expensive parallel seed simulation; run with --include-ignored or by name"]
#[test]
fn smart_strategy_test_parallel_seeds() {
    use std::fs::File;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use crate::game_driver::GameDriver;

    // Configuration from environment or defaults
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let num_seeds = std::env::var("SMART_STRATEGY_NUM_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3 * num_cpus);
    let base_seed = std::env::var("SMART_STRATEGY_BASE_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000u64);
    let output_path = std::env::var("SMART_STRATEGY_OUTPUT")
        .unwrap_or_else(|_| "/tmp/smart_strategy_seeds_results.jsonl".to_string());

    eprintln!("SmartStrategy parallel test:");
    eprintln!("  CPUs: {}", num_cpus);
    eprintln!("  Seeds to run: {}", num_seeds);
    eprintln!("  Base seed: {}", base_seed);
    eprintln!("  Output: {}", output_path);
    eprintln!();

    let strategy = SmartStrategy::new();
    let driver = Arc::new(GameDriver::new(500_000, vec![10, 20, 30, 40, 50]));
    let results = Arc::new(Mutex::new(Vec::new()));

    // Process seeds in batches of num_cpus (4), running each batch in parallel
    for batch in 0..(num_seeds + num_cpus - 1) / num_cpus {
        let batch_start = batch * num_cpus;
        let batch_end = (batch_start + num_cpus).min(num_seeds);
        let mut handles = vec![];

        eprintln!("Batch {}: seeds {}-{}", batch + 1, batch_start + 1, batch_end);

        for offset in 0..(batch_end - batch_start) {
            let driver = Arc::clone(&driver);
            let results = Arc::clone(&results);
            let strategy = SmartStrategy::new();

            let handle = thread::spawn(move || {
                let seed_idx = batch_start + offset;
                let seed = base_seed + u64::try_from(seed_idx).unwrap();
                eprintln!("[{}] seed {} ({}/{})", strategy.name(), seed, seed_idx + 1, num_seeds);
                let result = driver.play_game(seed, &strategy);
                eprintln!(
                    "  max_tier={:?} completed={} actions={}",
                    result.max_tier_reached, result.contracts_completed, result.total_actions,
                );
                results.lock().unwrap().push(result);
            });

            handles.push(handle);
        }

        // Wait for this batch to complete before starting the next one
        for handle in handles {
            handle.join().unwrap();
        }
    }

    let individual_results = Arc::try_unwrap(results)
        .unwrap()
        .into_inner()
        .unwrap();

    // Write results as JSONL (one JSON object per line for easy streaming)
    let mut file = File::create(&output_path).expect("create output file");
    for result in &individual_results {
        let json_str = serde_json::to_string(result).expect("serialize result");
        writeln!(file, "{}", json_str).expect("write result");
    }

    eprintln!();
    eprintln!("✓ Results written to: {}", output_path);
    eprintln!("  Total seeds: {}", individual_results.len());
}
