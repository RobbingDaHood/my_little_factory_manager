//! Ordered action log for deterministic game replay.
//!
//! The action log records every player action with a sequence number.
//! Combined with the game seed, it serves as the save/load mechanism:
//! replaying the same actions on a fresh game with the same seed
//! produces an identical game state.

use rocket::serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// A player action that can be dispatched to the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action_type", crate = "rocket::serde")]
pub enum PlayerAction {
    /// Start a new game. If seed is None, a random seed is generated.
    NewGame { seed: Option<u64> },
    /// Accept a contract from the offered list by tier and contract position.
    AcceptContract {
        tier_index: usize,
        contract_index: usize,
    },
    /// Play a card from hand by its index in the cards Vec (must have hand > 0).
    PlayCard { card_index: usize },
    /// Discard a card from hand for a small baseline production bonus.
    DiscardCard { card_index: usize },
    /// Replace a card in the active cycle (deck or discard, auto-selected)
    /// with a shelved card. The replacement always enters the deck.
    /// A third card is permanently destroyed as the cost.
    /// Only available between contracts.
    ReplaceCard {
        target_card_index: usize,
        replacement_card_index: usize,
        sacrifice_card_index: usize,
    },
    /// Abandon the active contract before it resolves naturally.
    ///
    /// Only allowed after at least `min_turns_before_abandon` turns have been
    /// played on the contract. Abandoning counts as a contract failure in all
    /// failure metrics and also increments `total_contracts_abandoned`.
    /// Abandonment is an emergency escape — no well-balanced strategy should
    /// rely on it routinely.
    AbandonContract,
}

/// A single entry in the action log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct ActionEntry {
    pub seq: u64,
    pub action: PlayerAction,
}

/// Ordered log of all player actions in the current game.
#[derive(Debug, Clone)]
pub struct ActionLog {
    entries: Vec<ActionEntry>,
}

impl ActionLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Append an action and return the created entry.
    pub fn append(&mut self, action: PlayerAction) -> ActionEntry {
        let entry = ActionEntry {
            seq: self.entries.len() as u64,
            action,
        };
        self.entries.push(entry.clone());
        entry
    }

    /// Return a snapshot of all entries.
    pub fn entries(&self) -> &[ActionEntry] {
        &self.entries
    }
}

impl Default for ActionLog {
    fn default() -> Self {
        Self::new()
    }
}
