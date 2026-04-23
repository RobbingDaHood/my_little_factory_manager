use serde_json::{json, Value};

use crate::game_driver::GameSnapshot;
use crate::strategies::Strategy;

/// The simplest possible strategy:
/// - Accepts the highest available tier contract (last in valid_tiers list)
/// - Always plays the first valid card
/// - Discards only when no card can be played
/// - Abandons the contract as last resort (after min_turns_before_abandon) when
///   neither PlayCard nor DiscardCard is possible — this prevents permanent stalls
///   on uncompletable contracts
/// - Never deckbuilds (ignores ReplaceCard)
///
/// Picking the highest tier ensures the strategy actually attempts to advance
/// through tier milestones. It remains simple because it reads nothing from
/// game state and applies no card selection heuristics.  It intentionally
/// makes no attempt to manage harmful tokens, respect turn windows, or
/// optimise for contract requirements, which reveals the earliest tier where
/// the game mechanics become unforgiving for naive play.
pub struct SimpleFirstStrategy;

impl SimpleFirstStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Strategy for SimpleFirstStrategy {
    fn name(&self) -> &str {
        "simple_first"
    }

    fn choose_action(&self, possible_actions: &[Value], _snapshot: &GameSnapshot) -> Value {
        // 1. Play the first valid card if a contract is active
        if let Some(play) = possible_actions
            .iter()
            .find(|a| a["action_type"] == "PlayCard")
        {
            if let Some(indices) = play["valid_card_indices"].as_array() {
                if let Some(first) = indices.first() {
                    return json!({
                        "action_type": "PlayCard",
                        "card_index": first
                    });
                }
            }
        }

        // 2. Accept the highest available tier contract (furthest progression)
        if let Some(accept) = possible_actions
            .iter()
            .find(|a| a["action_type"] == "AcceptContract")
        {
            if let Some(tiers) = accept["valid_tiers"].as_array() {
                if let Some(highest_tier) = tiers.last() {
                    let tier_index = highest_tier["tier_index"].as_u64().unwrap_or(0);
                    let contract_index = highest_tier["valid_contract_index_range"]["min"]
                        .as_u64()
                        .unwrap_or(0);
                    return json!({
                        "action_type": "AcceptContract",
                        "tier_index": tier_index,
                        "contract_index": contract_index
                    });
                }
            }
        }

        // 3. Discard the first card when no card can be played
        if let Some(discard) = possible_actions
            .iter()
            .find(|a| a["action_type"] == "DiscardCard")
        {
            if let Some(indices) = discard["valid_card_indices"].as_array() {
                if let Some(first) = indices.first() {
                    return json!({
                        "action_type": "DiscardCard",
                        "card_index": first
                    });
                }
            }
        }

        // 3.5. Abandon the contract as a last resort when stuck — no PlayCard or
        //      DiscardCard available (e.g. all cards banned by a CardTagConstraint
        //      and hand is empty after exhausting the deck).
        if possible_actions
            .iter()
            .any(|a| a["action_type"] == "AbandonContract")
        {
            return json!({ "action_type": "AbandonContract" });
        }

        panic!(
            "SimpleFirstStrategy: no actionable option found in {:?}",
            possible_actions
        );
    }
}
