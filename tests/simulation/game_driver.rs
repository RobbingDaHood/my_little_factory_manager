//! IMPORTANT — simulation-only fast path
//!
//! This module is the ONLY test module permitted to call `GameState` methods
//! directly (bypassing HTTP). Every other integration test MUST drive behaviour
//! through the HTTP API so that the actual endpoints are exercised.
//!
//! Direct calls are used here solely to remove the ~800 µs/call HTTP pipeline
//! overhead from the hot simulation loop. Game behaviour is identical to the
//! HTTP path because the same `GameState::dispatch`, `possible_actions`, and
//! `view` methods are called under the hood by the HTTP handlers.

use std::collections::{HashMap, HashSet};

use my_little_factory_manager::game_state::{
    ActionResult, ActionSuccess, GameState, PossibleAction, StrategyView,
};
use my_little_factory_manager::types::{ContractFailureReason, ContractResolution};
use serde::Serialize;

use crate::strategies::Strategy;

/// A point-in-time snapshot passed to the strategy for decision-making.
pub struct GameSnapshot<'a> {
    /// Lightweight borrowed state view from `GameState::view_for_scoring()`. `None` when `needs_state()` returns false.
    /// Uses borrowed references to avoid cloning/allocation overhead during strategy evaluation.
    pub state: Option<StrategyView<'a>>,
    /// Available actions with NewGame filtered out.
    pub possible_actions: Vec<PossibleAction>,
}

/// Total actions taken when the first contract at a specific milestone tier was completed.
#[derive(Debug, Clone, Serialize)]
pub struct MilestoneResult {
    pub tier: u32,
    pub actions_to_reach: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum ExitReason {
    Completed,
    ActionLimitExceeded,
    StallDetected,
}

/// Results from one complete simulated game.
#[derive(Debug, Clone, Serialize)]
pub struct GameResult {
    pub seed: u64,
    pub milestones: Vec<MilestoneResult>,
    pub max_tier_reached: Option<u32>,
    pub total_actions: u64,
    pub contracts_completed: u32,
    pub contracts_failed: u32,
    pub contracts_abandoned: u32,
    /// Count of each contract failure reason observed.
    pub failure_reasons: HashMap<String, u32>,
    pub completed_per_tier: HashMap<u32, u32>,
    pub failed_per_tier: HashMap<u32, u32>,
    pub abandoned_per_tier: HashMap<u32, u32>,
    /// True when max_actions was exhausted before all milestones were reached.
    pub hit_action_limit: bool,
    /// True when no non-NewGame actions were available (invariant broken).
    pub stuck: bool,
    pub exit_reason: ExitReason,
}

impl GameResult {
    fn new(seed: u64) -> Self {
        Self {
            seed,
            milestones: Vec::new(),
            max_tier_reached: None,
            total_actions: 0,
            contracts_completed: 0,
            contracts_failed: 0,
            contracts_abandoned: 0,
            failure_reasons: HashMap::new(),
            completed_per_tier: HashMap::new(),
            failed_per_tier: HashMap::new(),
            abandoned_per_tier: HashMap::new(),
            hit_action_limit: false,
            stuck: false,
            exit_reason: ExitReason::Completed,
        }
    }
}

/// Drives one game session from `NewGame` until all milestones are reached or
/// the action budget is exhausted.
pub struct GameDriver {
    pub max_actions: u64,
    pub milestone_tiers: Vec<u32>,
    pub max_contracts_without_tier_progress: u64,
}

impl GameDriver {
    pub fn new(max_actions: u64, milestone_tiers: Vec<u32>) -> Self {
        Self {
            max_actions,
            milestone_tiers,
            max_contracts_without_tier_progress: 1000,
        }
    }

    pub fn with_stall_threshold(mut self, threshold: u64) -> Self {
        self.max_contracts_without_tier_progress = threshold;
        self
    }

    pub fn play_game(&self, seed: u64, strategy: &dyn Strategy) -> GameResult {
        // Direct GameState construction bypasses HTTP — see module-level doc comment.
        let mut state = GameState::new(Some(seed));

        let mut result = GameResult::new(seed);
        let milestone_set: HashSet<u32> = self.milestone_tiers.iter().cloned().collect();
        let mut reached_milestones: HashSet<u32> = HashSet::new();

        let mut current_tier: u32 = 0;
        let mut contracts_since_last_tier: u64 = 0;

        loop {
            let non_new_game: Vec<PossibleAction> = state
                .possible_actions()
                .into_iter()
                .filter(|a| !matches!(a, PossibleAction::NewGame))
                .collect();

            if non_new_game.is_empty() {
                result.stuck = true;
                result.exit_reason = ExitReason::Completed;
                break;
            }

            let state_view = if strategy.needs_state() {
                Some(state.view_for_scoring())
            } else {
                None
            };
            let snapshot = GameSnapshot {
                state: state_view,
                possible_actions: non_new_game.clone(),
            };

            let player_action = strategy.choose_action(&non_new_game, &snapshot);
            result.total_actions += 1;

            let response = state.dispatch(player_action);

            if let ActionResult::Success(ref success) = response {
                let resolution_opt: Option<&ContractResolution> = match success {
                    ActionSuccess::CardPlayed {
                        contract_resolution: Some(res),
                    } => Some(res),
                    ActionSuccess::CardDiscarded {
                        contract_resolution: Some(res),
                    } => Some(res),
                    ActionSuccess::ContractAbandoned {
                        contract_resolution,
                    } => Some(contract_resolution),
                    _ => None,
                };

                if let Some(resolution) = resolution_opt {
                    match resolution {
                        ContractResolution::Completed { contract } => {
                            let tier = contract.tier.0;
                            result.contracts_completed += 1;
                            *result.completed_per_tier.entry(tier).or_insert(0) += 1;
                            result.max_tier_reached =
                                Some(result.max_tier_reached.map_or(tier, |t: u32| t.max(tier)));

                            if tier > current_tier {
                                current_tier = tier;
                                contracts_since_last_tier = 0;
                            } else {
                                contracts_since_last_tier += 1;
                            }

                            if milestone_set.contains(&tier) && !reached_milestones.contains(&tier)
                            {
                                reached_milestones.insert(tier);
                                result.milestones.push(MilestoneResult {
                                    tier,
                                    actions_to_reach: result.total_actions,
                                });
                            }
                        }
                        ContractResolution::Failed { contract, reason } => {
                            let tier = contract.tier.0;
                            result.contracts_failed += 1;
                            *result.failed_per_tier.entry(tier).or_insert(0) += 1;
                            contracts_since_last_tier += 1;
                            let failure_type = match reason {
                                ContractFailureReason::HarmfulTokenLimitExceeded { .. } => {
                                    "HarmfulTokenLimitExceeded"
                                }
                                ContractFailureReason::TurnWindowExceeded { .. } => {
                                    "TurnWindowExceeded"
                                }
                                ContractFailureReason::Abandoned { .. } => {
                                    result.contracts_abandoned += 1;
                                    *result.abandoned_per_tier.entry(tier).or_insert(0) += 1;
                                    "Abandoned"
                                }
                            }
                            .to_string();
                            *result.failure_reasons.entry(failure_type).or_insert(0) += 1;
                        }
                    }
                }
            }

            if reached_milestones.len() == self.milestone_tiers.len() {
                result.exit_reason = ExitReason::Completed;
                break;
            }

            if contracts_since_last_tier >= self.max_contracts_without_tier_progress {
                result.exit_reason = ExitReason::StallDetected;
                break;
            }

            if result.total_actions >= self.max_actions {
                result.hit_action_limit = true;
                result.exit_reason = ExitReason::ActionLimitExceeded;
                break;
            }
        }

        result
    }
}
