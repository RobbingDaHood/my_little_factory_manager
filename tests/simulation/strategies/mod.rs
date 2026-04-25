use serde_json::Value;

use crate::game_driver::GameSnapshot;

pub mod smart_strategy;

/// A pluggable gameplay strategy for simulation.
///
/// Receives only the actions the game considers valid (NewGame is excluded).
/// Returns a single action value ready to POST to `/action`.
pub trait Strategy {
    fn name(&self) -> &str;
    fn choose_action(&self, possible_actions: &[Value], snapshot: &GameSnapshot) -> Value;

    /// Return `true` if this strategy requires `snapshot.state` to be populated.
    /// Defaults to `false` — avoids an extra `GET /state` per action for simple
    /// strategies that only inspect `possible_actions`.
    fn needs_state(&self) -> bool {
        false
    }
}
