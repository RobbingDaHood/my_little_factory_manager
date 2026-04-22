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
    /// Top contract failure reasons sorted by frequency (descending).
    pub top_failure_reasons: Vec<(String, u64)>,
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
            eprintln!(
                "  max_tier={:?} completed={} failed={} actions={}{}",
                result.max_tier_reached,
                result.contracts_completed,
                result.contracts_failed,
                result.total_actions,
                if result.hit_action_limit {
                    " [LIMIT]"
                } else {
                    ""
                },
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
                let reached: Vec<u64> = results
                    .iter()
                    .filter_map(|r| {
                        r.milestones
                            .iter()
                            .find(|m| m.tier == tier)
                            .map(|m| m.actions_to_reach)
                    })
                    .collect();
                let reach_rate = reached.len() as f64 / games_run as f64;
                let mean_actions = if reached.is_empty() {
                    None
                } else {
                    Some(reached.iter().sum::<u64>() as f64 / reached.len() as f64)
                };
                MilestoneStats {
                    tier,
                    reach_rate,
                    mean_actions,
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

        let mut failure_totals: HashMap<String, u64> = HashMap::new();
        for result in &results {
            for (reason, &count) in &result.failure_reasons {
                *failure_totals.entry(reason.clone()).or_insert(0) += u64::from(count);
            }
        }
        let mut top_failure_reasons: Vec<(String, u64)> = failure_totals.into_iter().collect();
        top_failure_reasons.sort_by_key(|a| Reverse(a.1));

        let stuck_games = results.iter().filter(|r| r.stuck).count() as u32;
        let action_limit_games = results.iter().filter(|r| r.hit_action_limit).count() as u32;

        StrategyReport {
            strategy_name: name.to_string(),
            games_run,
            milestones,
            overall_max_tier,
            total_contracts_completed,
            total_contracts_failed,
            top_failure_reasons,
            stuck_games,
            action_limit_games,
        }
    }
}
