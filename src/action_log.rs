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
    /// Accept the currently offered contract.
    AcceptContract,
    /// Play a card from hand by its position index.
    PlayCard { hand_index: usize },
    /// Discard a card from hand for a small baseline production bonus.
    DiscardCard { hand_index: usize },
    /// Abandon the active contract and receive a new offer.
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
    next_seq: u64,
}

impl ActionLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 1,
        }
    }

    /// Append an action and return the created entry.
    pub fn append(&mut self, action: PlayerAction) -> ActionEntry {
        let entry = ActionEntry {
            seq: self.next_seq,
            action,
        };
        self.next_seq += 1;
        self.entries.push(entry.clone());
        entry
    }

    /// Return a snapshot of all entries.
    pub fn entries(&self) -> &[ActionEntry] {
        &self.entries
    }

    /// Clear the log (used on NewGame).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.next_seq = 1;
    }
}

impl Default for ActionLog {
    fn default() -> Self {
        Self::new()
    }
}
