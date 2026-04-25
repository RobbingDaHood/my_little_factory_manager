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

use my_little_factory_manager::action_log::PlayerAction;
use my_little_factory_manager::game_state::GameState;
use serde::Serialize;
use serde_json::Value;

use crate::strategies::Strategy;

/// A point-in-time snapshot passed to the strategy for decision-making.
pub struct GameSnapshot {
    /// Full game state from `GameState::view()` (serialised to JSON for strategy compatibility).
    pub state: Value,
    /// Available actions with NewGame filtered out.
    pub possible_actions: Vec<Value>,
}

/// Total actions taken when the first contract at a specific milestone tier was completed.
#[derive(Debug, Clone, Serialize)]
pub struct MilestoneResult {
    pub tier: u32,
    pub actions_to_reach: u64,
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
    /// True when max_actions was exhausted before all milestones were reached.
    pub hit_action_limit: bool,
    /// True when no non-NewGame actions were available (invariant broken).
    pub stuck: bool,
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
            hit_action_limit: false,
            stuck: false,
        }
    }
}

/// Drives one game session from `NewGame` until all milestones are reached or
/// the action budget is exhausted.
pub struct GameDriver {
    pub max_actions: u64,
    pub milestone_tiers: Vec<u32>,
}

impl GameDriver {
    pub fn new(max_actions: u64, milestone_tiers: Vec<u32>) -> Self {
        Self {
            max_actions,
            milestone_tiers,
        }
    }

    pub fn play_game(&self, seed: u64, strategy: &dyn Strategy) -> GameResult {
        // Direct GameState construction bypasses HTTP — see module-level doc comment.
        let mut state = GameState::new(Some(seed));

        let mut result = GameResult::new(seed);
        let milestone_set: HashSet<u32> = self.milestone_tiers.iter().cloned().collect();
        let mut reached_milestones: HashSet<u32> = HashSet::new();

        loop {
            let all_possible: Vec<Value> = serde_json::to_value(state.possible_actions())
                .expect("PossibleAction serializes")
                .as_array()
                .cloned()
                .unwrap_or_default();

            // NewGame is always listed but must not be chosen during an active session.
            let non_new_game: Vec<Value> = all_possible
                .into_iter()
                .filter(|a| a["action_type"] != "NewGame")
                .collect();

            if non_new_game.is_empty() {
                result.stuck = true;
                break;
            }

            let state_value = if strategy.needs_state() {
                serde_json::to_value(state.view()).expect("GameStateView serializes")
            } else {
                Value::Null
            };
            let snapshot = GameSnapshot {
                state: state_value,
                possible_actions: non_new_game.clone(),
            };

            let action = strategy.choose_action(&non_new_game, &snapshot);
            result.total_actions += 1;

            let player_action: PlayerAction =
                serde_json::from_value(action).expect("strategy returned valid PlayerAction JSON");
            let response = serde_json::to_value(state.dispatch(player_action))
                .expect("ActionResult serializes");

            if response["outcome"] == "Success" {
                let resolution = &response["detail"]["contract_resolution"];
                match resolution["resolution_type"].as_str() {
                    Some("Completed") => {
                        let tier = resolution["contract"]["tier"].as_u64().unwrap_or(0) as u32;
                        result.contracts_completed += 1;
                        result.max_tier_reached =
                            Some(result.max_tier_reached.map_or(tier, |t: u32| t.max(tier)));

                        if milestone_set.contains(&tier) && !reached_milestones.contains(&tier) {
                            reached_milestones.insert(tier);
                            result.milestones.push(MilestoneResult {
                                tier,
                                actions_to_reach: result.total_actions,
                            });
                        }
                    }
                    Some("Failed") => {
                        result.contracts_failed += 1;
                        let failure_type = resolution["reason"]["failure_type"]
                            .as_str()
                            .unwrap_or("Unknown")
                            .to_string();
                        if failure_type == "Abandoned" {
                            result.contracts_abandoned += 1;
                        }
                        *result.failure_reasons.entry(failure_type).or_insert(0) += 1;
                    }
                    _ => {}
                }
            }

            if reached_milestones.len() == self.milestone_tiers.len() {
                break;
            }
            if result.total_actions >= self.max_actions {
                result.hit_action_limit = true;
                break;
            }
        }

        result
    }
}
