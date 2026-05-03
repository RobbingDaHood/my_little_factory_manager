//! Optimized game driver that works directly with GameState (no HTTP overhead).
//!
//! This driver uses direct Rust method calls to GameState and StrategyView,
//! avoiding HTTP serialization/deserialization overhead. It's suitable for
//! strategy simulation and performance testing.

use my_little_factory_manager::game_state::GameState;
use serde::Serialize;

use crate::internal_smart_strategy::InternalSmartStrategy;

/// A game result with performance metrics.
#[derive(Debug, Clone, Serialize)]
pub struct InternalGameResult {
    pub seed: u64,
    pub max_tier_reached: Option<u32>,
    pub total_actions: u64,
    pub contracts_completed: u32,
    pub contracts_failed: u32,
    pub contracts_abandoned: u32,
}

impl InternalGameResult {
    fn new(seed: u64) -> Self {
        Self {
            seed,
            max_tier_reached: None,
            total_actions: 0,
            contracts_completed: 0,
            contracts_failed: 0,
            contracts_abandoned: 0,
        }
    }
}

/// Optimized game driver that works directly with GameState.
pub struct InternalGameDriver {
    pub max_actions: u64,
}

impl InternalGameDriver {
    pub fn new(max_actions: u64) -> Self {
        Self { max_actions }
    }

    /// Play a single game using direct GameState calls and InternalSmartStrategy.
    pub fn play_game(&self, seed: u64, strategy: &InternalSmartStrategy) -> InternalGameResult {
        let state = GameState::new(Some(seed));
        let result = InternalGameResult::new(seed);

        loop {
            // Get possible actions
            let possible_actions = state.possible_actions();

            // Filter out NewGame action
            let non_new_game: Vec<_> = possible_actions
                .iter()
                .enumerate()
                .filter(|(_, a)| !matches!(a, my_little_factory_manager::game_state::PossibleAction::NewGame))
                .collect();

            if non_new_game.is_empty() {
                break;
            }

            // Get the state view for strategy evaluation
            let view = state.view_for_scoring();

            // Convert possible actions to string representations for the strategy
            let valid_action_strings: Vec<String> = non_new_game
                .iter()
                .map(|(_, a)| format!("{:?}", a)) // Simple debug format for now
                .collect();

            // Choose the next action (simplified for now)
            let _action_str = strategy.choose_action_from_view(&view, &valid_action_strings);

            // For now, just break to avoid infinite loop
            // TODO: Complete action parsing and execution
            break;
        }

        result
    }
}
