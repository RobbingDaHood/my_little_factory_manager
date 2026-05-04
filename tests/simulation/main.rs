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

use game_driver::{GameDriver, GameResult};
use runner::{SimulationConfig, SimulationRunner};
use strategies::smart_aggressive::SmartAggressive;
use strategies::smart_careful::SmartCareful;
use strategies::Strategy;

/// Fast diagnostic run with finer-grained milestones — used for tuning iterations.
/// Action budget is intentionally small so iteration is quick; raise it locally
/// if you need to confirm a tuning change reaches a higher tier in absolute terms.
#[ignore = "diagnostic-only fast simulation; run with --include-ignored or by name"]
#[test]
fn smart_aggressive_diagnostic() {
    let strategy = SmartAggressive::new();
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

/// Aspirational target: SmartAggressive should be able to reach tier 50.
/// Currently failing — left in place as the marker for "we're done" once
/// strategy progression is good enough to clear the full tier ladder.
#[ignore = "expensive strategy simulation; run with --include-ignored or by name"]
#[test]
fn smart_aggressive_reaches_tier_50() {
    let strategy = SmartAggressive::new();
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
        "SmartAggressive must reach tier 50 to confirm a winning path exists.\n\
         Max tier reached: {:?}\n\
         Top failure reasons: {:?}\n\
         Milestones: {:?}",
        report.overall_max_tier,
        report.top_failure_reasons,
        report.milestones,
    );
}

/// Single-seed run for SmartAggressive, configured via SEED_UUID and MAX_ACTIONS.
/// Designed for parallel invocation by `scripts/test_smartstrategy_optimized.sh`,
/// which launches multiple instances on distinct UUIDs.
///
/// Output is structured: a SUMMARY line (single-line, grep-parseable),
/// a TIER_BREAKDOWN block (per-tier completed/failed/abandoned), and one
/// MILESTONE block per milestone tier showing the per-tier breakdown at the
/// moment the milestone was first reached. The shell script aggregates these
/// across seeds to produce a per-strategy progression report.
#[ignore = "expensive; configure via SEED_UUID and MAX_ACTIONS env vars"]
#[test]
fn smart_aggressive_seed() {
    run_seed_test(&SmartAggressive::new());
}

/// Single-seed run for SmartCareful — same harness as `smart_aggressive_seed`.
#[ignore = "expensive; configure via SEED_UUID and MAX_ACTIONS env vars"]
#[test]
fn smart_careful_seed() {
    run_seed_test(&SmartCareful::new());
}

fn run_seed_test(strategy: &dyn Strategy) {
    let seed_str = std::env::var("SEED_UUID").unwrap_or_else(|_| "default".to_string());
    let max_actions: u64 = std::env::var("MAX_ACTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);

    let seed = fnv1a_hash(&seed_str);
    let driver = GameDriver::new(max_actions, vec![5, 10, 15, 20, 25, 30, 35, 40, 45, 50]);
    let result = driver.play_game(seed, strategy);
    print_seed_report(strategy.name(), &seed_str, &result);
}

fn print_seed_report(strategy_name: &str, seed_uuid: &str, r: &GameResult) {
    let max_tier = r.max_tier_reached.unwrap_or(0);
    eprintln!("=== STRATEGY: {} ===", strategy_name);
    eprintln!("=== UUID: {} ===", seed_uuid);
    eprintln!(
        "SUMMARY max_tier={} completed={} failed={} abandoned={} actions={} limit={} stuck={}",
        max_tier,
        r.contracts_completed,
        r.contracts_failed,
        r.contracts_abandoned,
        r.total_actions,
        r.hit_action_limit,
        r.stuck,
    );

    eprintln!("TIER_BREAKDOWN");
    let highest_tier_seen = highest_tier(r);
    for tier in 0..=highest_tier_seen {
        let c = r.completed_per_tier.get(&tier).copied().unwrap_or(0);
        let f = r.failed_per_tier.get(&tier).copied().unwrap_or(0);
        let a = r.abandoned_per_tier.get(&tier).copied().unwrap_or(0);
        if c == 0 && f == 0 && a == 0 {
            continue;
        }
        eprintln!(
            "  tier={} completed={} failed={} abandoned={}",
            tier, c, f, a
        );
    }

    for milestone_tier in [5u32, 10, 15, 20, 25, 30, 35, 40, 45, 50] {
        match r.milestones.iter().find(|m| m.tier == milestone_tier) {
            Some(m) => {
                eprintln!(
                    "MILESTONE tier={} actions={}",
                    milestone_tier, m.actions_to_reach
                );
                eprintln!(
                    "  completed_per_tier: {}",
                    format_per_tier_map(&m.progression.completed_per_tier)
                );
                eprintln!(
                    "  failed_per_tier:    {}",
                    format_per_tier_map(&m.progression.failed_per_tier)
                );
                eprintln!(
                    "  abandoned_per_tier: {}",
                    format_per_tier_map(&m.progression.abandoned_per_tier)
                );
            }
            None => {
                eprintln!(
                    "MILESTONE tier={} actions=N/A (not reached)",
                    milestone_tier
                );
            }
        }
    }
    eprintln!("=== END ===");
}

fn highest_tier(r: &GameResult) -> u32 {
    let max_completed = r.completed_per_tier.keys().copied().max().unwrap_or(0);
    let max_failed = r.failed_per_tier.keys().copied().max().unwrap_or(0);
    let max_abandoned = r.abandoned_per_tier.keys().copied().max().unwrap_or(0);
    max_completed.max(max_failed).max(max_abandoned)
}

fn format_per_tier_map(m: &std::collections::HashMap<u32, u32>) -> String {
    let mut entries: Vec<(u32, u32)> = m.iter().map(|(&t, &c)| (t, c)).collect();
    entries.sort_by_key(|(t, _)| *t);
    entries
        .iter()
        .map(|(t, c)| format!("{}:{}", t, c))
        .collect::<Vec<_>>()
        .join(" ")
}

fn fnv1a_hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3_u64)
    })
}
