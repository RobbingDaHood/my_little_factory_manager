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

use game_driver::GameDriver;
use runner::{SimulationConfig, SimulationRunner};
use strategies::smart_strategy::SmartStrategy;
use strategies::smart_strategy_v2::SmartStrategyV2;

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

/// Runs SmartStrategy for a single seed configured via environment variables.
///
/// Designed for use by `scripts/test_smartstrategy_optimized.sh`, which launches
/// multiple instances of this test in parallel — each with a unique SEED_UUID —
/// eliminating the sequential batch bottleneck.
///
/// Environment variables:
///   SEED_UUID   — string identifier hashed to a u64 game seed (default: "default")
///   MAX_ACTIONS — action budget per game (default: 100_000)
///
/// Output (grep-parseable by the shell script):
///   max_tier=<N> completed=<N> failed=<N> abandoned=<N> actions=<N> [LIMIT]
#[ignore = "expensive; configure via SEED_UUID and MAX_ACTIONS env vars"]
#[test]
fn smart_strategy_seed() {
    let seed_str = std::env::var("SEED_UUID").unwrap_or_else(|_| "default".to_string());
    let max_actions: u64 = std::env::var("MAX_ACTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);

    let seed = fnv1a_hash(&seed_str);
    let strategy = SmartStrategy::new();
    let driver = GameDriver::new(max_actions, vec![10, 20, 30, 40, 50]);
    let result = driver.play_game(seed, &strategy);

    let max_tier = result.max_tier_reached.unwrap_or(0);
    eprintln!(
        "max_tier={} completed={} failed={} abandoned={} actions={}{}",
        max_tier,
        result.contracts_completed,
        result.contracts_failed,
        result.contracts_abandoned,
        result.total_actions,
        if result.hit_action_limit {
            " [LIMIT]"
        } else {
            ""
        },
    );
}

/// Single-seed run for SmartStrategyV2 — same harness as `smart_strategy_seed`,
/// configured via SEED_UUID and MAX_ACTIONS env vars. Lets the optimised parallel
/// shell script benchmark V2 alongside V1 on the same UUIDs.
#[ignore = "expensive; configure via SEED_UUID and MAX_ACTIONS env vars"]
#[test]
fn smart_strategy_v2_seed() {
    let seed_str = std::env::var("SEED_UUID").unwrap_or_else(|_| "default".to_string());
    let max_actions: u64 = std::env::var("MAX_ACTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);

    let seed = fnv1a_hash(&seed_str);
    let strategy = SmartStrategyV2::new();
    let driver = GameDriver::new(max_actions, vec![10, 20, 30, 40, 50]);
    let result = driver.play_game(seed, &strategy);

    let max_tier = result.max_tier_reached.unwrap_or(0);
    eprintln!(
        "max_tier={} completed={} failed={} abandoned={} actions={}{}",
        max_tier,
        result.contracts_completed,
        result.contracts_failed,
        result.contracts_abandoned,
        result.total_actions,
        if result.hit_action_limit {
            " [LIMIT]"
        } else {
            ""
        },
    );
}

fn fnv1a_hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3_u64)
    })
}
