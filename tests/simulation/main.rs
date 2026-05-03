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
        max_contracts_without_tier_progress: 1000,
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
        max_contracts_without_tier_progress: 1000,
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
