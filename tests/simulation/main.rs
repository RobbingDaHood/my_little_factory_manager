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
use strategies::simple_first::SimpleFirstStrategy;

/// Runs SimpleFirstStrategy across multiple seeds and reports tier-milestone
/// performance.  The strategy never deckbuilds and makes no attempt to manage
/// harmful tokens — it exists purely to reveal how far naive play can reach and
/// what the earliest blockers are.
///
/// Hard assertions (test failure):
///   - At least one game reaches tier 2 (proves the basic game loop works end-to-end)
///
/// Soft observations (printed, not asserted):
///   - Milestone reach rates and mean action counts for tiers 10/20/30/40/50
///   - Dominant failure reasons at the tier where progress stalls
#[ignore = "expensive strategy simulation; run with --include-ignored or by name"]
#[test]
fn simple_first_strategy_progression() {
    let strategy = SimpleFirstStrategy::new();
    let config = SimulationConfig {
        games_per_strategy: 3,
        base_seed: 42,
        max_actions_per_game: 2_000_000,
        milestone_tiers: vec![10, 20, 30, 40, 50],
    };

    let runner = SimulationRunner::new(config);
    let report = runner.run_strategy(&strategy);

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report serialisable")
    );

    // Hard assertion: game loop is functional
    assert!(
        report.overall_max_tier.is_some_and(|t| t >= 2),
        "SimpleFirstStrategy should reach at least tier 2; max tier reached: {:?}. \
         This indicates a broken game loop, not just a weak strategy.",
        report.overall_max_tier,
    );

    // Inform developer when the strategy stalls before tier 50
    if report.overall_max_tier.is_none_or(|t| t < 50) {
        eprintln!(
            "\nSimpleFirstStrategy did not reach tier 50.\n\
             Max tier: {:?}\n\
             Top failure reasons: {:?}\n\
             Suggestion: a simple strategy is expected to stall due to harmful \
             token accumulation (no waste-removal cards in starter deck) and the \
             lack of deckbuilding. This is informational — see Phase 10.2+ for \
             strategies that handle these mechanics.",
            report.overall_max_tier, report.top_failure_reasons,
        );
    }

    // Log unreached milestones
    for milestone in &report.milestones {
        if milestone.reach_rate < 1.0 {
            eprintln!(
                "Milestone tier {}: reach rate {:.0}% (mean actions: {:?})",
                milestone.tier,
                milestone.reach_rate * 100.0,
                milestone.mean_actions,
            );
        }
    }
}
