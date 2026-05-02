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

/// Runs SmartStrategy across 100 different seeds and reports detailed results.
///
/// Outputs a JSON file at /tmp/smart_strategy_100_seeds_results.json with:
///   - Individual GameResult for each seed
///   - Aggregated StrategyReport with statistics
///
/// This is used to generate GitHub issues with sub-issues for each seed run.
#[ignore = "expensive 100-seed simulation; run with --include-ignored or by name"]
#[test]
fn smart_strategy_test_100_seeds() {
    use std::fs;
    use crate::game_driver::GameDriver;

    let strategy = SmartStrategy::new();
    let config = SimulationConfig {
        games_per_strategy: 100,
        base_seed: 1000,
        max_actions_per_game: 500_000,
        milestone_tiers: vec![10, 20, 30, 40, 50],
    };

    // Collect individual game results
    let driver = GameDriver::new(
        config.max_actions_per_game,
        config.milestone_tiers.clone(),
    );

    let mut individual_results = Vec::new();
    for i in 0..config.games_per_strategy {
        let seed = config.base_seed + u64::from(i);
        eprintln!("[{}] seed {} ({}/{})", strategy.name(), seed, i + 1, config.games_per_strategy);
        let result = driver.play_game(seed, &strategy);
        eprintln!(
            "  max_tier={:?} completed={} failed={} abandoned={} actions={}{}",
            result.max_tier_reached,
            result.contracts_completed,
            result.contracts_failed,
            result.contracts_abandoned,
            result.total_actions,
            if result.hit_action_limit { " [LIMIT]" } else { "" },
        );
        individual_results.push(result);
    }

    // Compute summary statistics
    let max_tier = individual_results.iter().filter_map(|r| r.max_tier_reached).max();
    let total_completed: u32 = individual_results.iter().map(|r| r.contracts_completed).sum();
    let total_failed: u32 = individual_results.iter().map(|r| r.contracts_failed).sum();
    let total_abandoned: u32 = individual_results.iter().map(|r| r.contracts_abandoned).sum();
    let stuck_count = individual_results.iter().filter(|r| r.stuck).count() as u32;
    let limit_count = individual_results.iter().filter(|r| r.hit_action_limit).count() as u32;

    let output = serde_json::json!({
        "summary": {
            "strategy": strategy.name(),
            "num_seeds": config.games_per_strategy,
            "base_seed": config.base_seed,
            "max_tier_reached": max_tier,
            "total_completed": total_completed,
            "total_failed": total_failed,
            "total_abandoned": total_abandoned,
            "stuck_games": stuck_count,
            "limit_games": limit_count,
        },
        "individual_results": individual_results,
    });

    let json_str = serde_json::to_string_pretty(&output).expect("serialisable");
    println!("{}", json_str);

    let output_path = "/tmp/smart_strategy_100_seeds_results.json";
    fs::write(output_path, json_str).expect("write results to file");
    eprintln!("Results written to: {}", output_path);
    eprintln!("Summary:");
    eprintln!("  Max tier: {:?}", max_tier);
    eprintln!("  Completed: {}", total_completed);
    eprintln!("  Failed: {}", total_failed);
    eprintln!("  Abandoned: {}", total_abandoned);
    eprintln!("  Stuck games: {}", stuck_count);
    eprintln!("  Hit action limit: {}", limit_count);
}
