use std::cmp::Reverse;
use std::collections::HashMap;

use serde::Serialize;

use crate::game_driver::{GameDriver, GameResult};
use crate::strategies::Strategy;

/// Configuration for a simulation run.
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub games_per_strategy: u32,
    pub base_seed: u64,
    pub max_actions_per_game: u64,
    pub milestone_tiers: Vec<u32>,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            games_per_strategy: 3,
            base_seed: 42,
            max_actions_per_game: 200_000,
            milestone_tiers: vec![10, 20, 30, 40, 50],
        }
    }
}

/// Aggregated statistics for a single milestone tier across all simulated games.
#[derive(Debug, Clone, Serialize)]
pub struct MilestoneStats {
    pub tier: u32,
    /// Fraction of games that completed a contract at this tier (0.0–1.0).
    pub reach_rate: f64,
    /// Mean total actions to first completion at this tier, over games that reached it.
    /// None if no game reached this milestone.
    pub mean_actions: Option<f64>,
    /// Average per-tier completed count at the moment this milestone was first reached.
    /// `mean_completed_per_tier[3]` = average tier-3 contracts completed before
    /// reaching this milestone, across games that reached it.
    pub mean_completed_per_tier: HashMap<u32, f64>,
    /// Average per-tier failed count at the moment this milestone was first reached.
    pub mean_failed_per_tier: HashMap<u32, f64>,
    /// Average per-tier abandoned count at the moment this milestone was first reached.
    pub mean_abandoned_per_tier: HashMap<u32, f64>,
}

/// Aggregated simulation report for one strategy.
#[derive(Debug, Serialize)]
pub struct StrategyReport {
    pub strategy_name: String,
    pub games_run: u32,
    pub milestones: Vec<MilestoneStats>,
    pub overall_max_tier: Option<u32>,
    pub total_contracts_completed: u64,
    pub total_contracts_failed: u64,
    pub total_contracts_abandoned: u64,
    /// Top contract failure reasons sorted by frequency (descending).
    pub top_failure_reasons: Vec<(String, u64)>,
    pub completed_per_tier: HashMap<u32, u64>,
    pub failed_per_tier: HashMap<u32, u64>,
    pub abandoned_per_tier: HashMap<u32, u64>,
    pub stuck_games: u32,
    pub action_limit_games: u32,
}

/// Runs multiple seeds for a strategy and aggregates results into a `StrategyReport`.
pub struct SimulationRunner {
    pub config: SimulationConfig,
}

impl SimulationRunner {
    pub fn new(config: SimulationConfig) -> Self {
        Self { config }
    }

    pub fn run_strategy(&self, strategy: &dyn Strategy) -> StrategyReport {
        let driver = GameDriver::new(
            self.config.max_actions_per_game,
            self.config.milestone_tiers.clone(),
        );

        let mut all_results: Vec<GameResult> = Vec::new();

        for i in 0..self.config.games_per_strategy {
            let seed = self.config.base_seed + u64::from(i);
            eprintln!(
                "[{}] seed {} ({}/{})...",
                strategy.name(),
                seed,
                i + 1,
                self.config.games_per_strategy
            );
            let result = driver.play_game(seed, strategy);
            let exit_label = match result.exit_reason {
                crate::game_driver::ExitReason::Completed => "COMPLETED",
                crate::game_driver::ExitReason::ActionLimitExceeded => "ACTION_LIMIT",
            };
            eprintln!(
                "  max_tier={:?} completed={} failed={} abandoned={} actions={} [{}]",
                result.max_tier_reached,
                result.contracts_completed,
                result.contracts_failed,
                result.contracts_abandoned,
                result.total_actions,
                exit_label,
            );
            all_results.push(result);
        }

        self.aggregate(strategy.name(), all_results)
    }

    fn aggregate(&self, name: &str, results: Vec<GameResult>) -> StrategyReport {
        let games_run = results.len() as u32;

        let milestones: Vec<MilestoneStats> = self
            .config
            .milestone_tiers
            .iter()
            .map(|&tier| {
                // Only games that reached this milestone contribute to the
                // progression averages — averaging in unreached games would
                // bias the per-tier counts toward zero.
                let reached_milestones: Vec<&crate::game_driver::MilestoneResult> = results
                    .iter()
                    .filter_map(|r| r.milestones.iter().find(|m| m.tier == tier))
                    .collect();
                let reach_count = reached_milestones.len();
                let reach_rate = reach_count as f64 / games_run as f64;
                let mean_actions = if reached_milestones.is_empty() {
                    None
                } else {
                    Some(
                        reached_milestones
                            .iter()
                            .map(|m| m.actions_to_reach)
                            .sum::<u64>() as f64
                            / reach_count as f64,
                    )
                };
                let mut mean_completed_per_tier: HashMap<u32, f64> = HashMap::new();
                let mut mean_failed_per_tier: HashMap<u32, f64> = HashMap::new();
                let mut mean_abandoned_per_tier: HashMap<u32, f64> = HashMap::new();
                if reach_count > 0 {
                    let denom = reach_count as f64;
                    for m in &reached_milestones {
                        for (&t, &c) in &m.progression.completed_per_tier {
                            *mean_completed_per_tier.entry(t).or_insert(0.0) += f64::from(c);
                        }
                        for (&t, &c) in &m.progression.failed_per_tier {
                            *mean_failed_per_tier.entry(t).or_insert(0.0) += f64::from(c);
                        }
                        for (&t, &c) in &m.progression.abandoned_per_tier {
                            *mean_abandoned_per_tier.entry(t).or_insert(0.0) += f64::from(c);
                        }
                    }
                    for v in mean_completed_per_tier.values_mut() {
                        *v /= denom;
                    }
                    for v in mean_failed_per_tier.values_mut() {
                        *v /= denom;
                    }
                    for v in mean_abandoned_per_tier.values_mut() {
                        *v /= denom;
                    }
                }
                MilestoneStats {
                    tier,
                    reach_rate,
                    mean_actions,
                    mean_completed_per_tier,
                    mean_failed_per_tier,
                    mean_abandoned_per_tier,
                }
            })
            .collect();

        let overall_max_tier = results.iter().filter_map(|r| r.max_tier_reached).max();

        let total_contracts_completed: u64 = results
            .iter()
            .map(|r| u64::from(r.contracts_completed))
            .sum();
        let total_contracts_failed: u64 =
            results.iter().map(|r| u64::from(r.contracts_failed)).sum();
        let total_contracts_abandoned: u64 = results
            .iter()
            .map(|r| u64::from(r.contracts_abandoned))
            .sum();

        let mut failure_totals: HashMap<String, u64> = HashMap::new();
        for result in &results {
            for (reason, &count) in &result.failure_reasons {
                *failure_totals.entry(reason.clone()).or_insert(0) += u64::from(count);
            }
        }
        let mut top_failure_reasons: Vec<(String, u64)> = failure_totals.into_iter().collect();
        top_failure_reasons.sort_by_key(|a| Reverse(a.1));

        let mut completed_per_tier: HashMap<u32, u64> = HashMap::new();
        let mut failed_per_tier: HashMap<u32, u64> = HashMap::new();
        let mut abandoned_per_tier: HashMap<u32, u64> = HashMap::new();
        for result in &results {
            for (&tier, &count) in &result.completed_per_tier {
                *completed_per_tier.entry(tier).or_insert(0) += u64::from(count);
            }
            for (&tier, &count) in &result.failed_per_tier {
                *failed_per_tier.entry(tier).or_insert(0) += u64::from(count);
            }
            for (&tier, &count) in &result.abandoned_per_tier {
                *abandoned_per_tier.entry(tier).or_insert(0) += u64::from(count);
            }
        }

        let stuck_games = results.iter().filter(|r| r.stuck).count() as u32;
        let action_limit_games = results.iter().filter(|r| r.hit_action_limit).count() as u32;

        StrategyReport {
            strategy_name: name.to_string(),
            games_run,
            milestones,
            overall_max_tier,
            total_contracts_completed,
            total_contracts_failed,
            total_contracts_abandoned,
            top_failure_reasons,
            completed_per_tier,
            failed_per_tier,
            abandoned_per_tier,
            stuck_games,
            action_limit_games,
        }
    }
}
