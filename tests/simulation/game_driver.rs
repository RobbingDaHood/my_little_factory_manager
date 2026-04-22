use std::collections::{HashMap, HashSet};

use my_little_factory_manager::rocket_initialize;
use rocket::http::ContentType;
use rocket::local::blocking::Client;
use serde::Serialize;
use serde_json::{json, Value};

use crate::strategies::Strategy;

/// A point-in-time snapshot passed to the strategy for decision-making.
pub struct GameSnapshot {
    /// Full game state from `GET /state`.
    pub state: Value,
    /// Available actions with NewGame filtered out, from `GET /actions/possible`.
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
        let client = Client::tracked(rocket_initialize()).expect("valid rocket");

        let new_game_action = json!({"action_type": "NewGame", "seed": seed});
        post_action(&client, &new_game_action);

        let mut result = GameResult::new(seed);
        let milestone_set: HashSet<u32> = self.milestone_tiers.iter().cloned().collect();
        let mut reached_milestones: HashSet<u32> = HashSet::new();

        loop {
            let all_possible = get_possible_actions(&client);

            // NewGame is always listed but must not be chosen during an active session.
            let non_new_game: Vec<Value> = all_possible
                .into_iter()
                .filter(|a| a["action_type"] != "NewGame")
                .collect();

            if non_new_game.is_empty() {
                result.stuck = true;
                break;
            }

            let state = if strategy.needs_state() {
                get_state(&client)
            } else {
                Value::Null
            };
            let snapshot = GameSnapshot {
                state,
                possible_actions: non_new_game.clone(),
            };

            let action = strategy.choose_action(&non_new_game, &snapshot);
            result.total_actions += 1;

            let response = post_action(&client, &action);

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

fn post_action(client: &Client, action: &Value) -> Value {
    let response = client
        .post("/action")
        .header(ContentType::JSON)
        .body(action.to_string())
        .dispatch();
    let body = response.into_string().expect("response body");
    serde_json::from_str(&body).expect("valid json response from /action")
}

fn get_state(client: &Client) -> Value {
    let response = client.get("/state").dispatch();
    let body = response.into_string().expect("response body");
    serde_json::from_str(&body).expect("valid json from /state")
}

fn get_possible_actions(client: &Client) -> Vec<Value> {
    let response = client.get("/actions/possible").dispatch();
    let body = response.into_string().expect("response body");
    serde_json::from_str::<Vec<Value>>(&body).expect("valid json array from /actions/possible")
}
