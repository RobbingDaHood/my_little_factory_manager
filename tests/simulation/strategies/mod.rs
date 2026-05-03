use my_little_factory_manager::action_log::PlayerAction;
use my_little_factory_manager::game_state::PossibleAction;

use crate::game_driver::GameSnapshot;

pub mod smart_strategy;
pub mod smart_strategy_v2;

/// A pluggable gameplay strategy for simulation.
///
/// Receives only the actions the game considers valid (NewGame is excluded).
/// Returns a single typed `PlayerAction` ready to dispatch.
pub trait Strategy {
    fn name(&self) -> &str;
    fn choose_action(
        &self,
        possible_actions: &[PossibleAction],
        snapshot: &GameSnapshot,
    ) -> PlayerAction;

    /// Return `true` if this strategy requires `snapshot.state` to be populated.
    /// Defaults to `false` — avoids an extra `GameState::view()` call per action for
    /// strategies that only inspect `possible_actions`.
    fn needs_state(&self) -> bool {
        false
    }
}
