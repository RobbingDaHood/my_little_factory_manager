//! Optimized game driver that works directly with GameState (no HTTP overhead).
//!
//! This driver uses direct Rust method calls to GameState and StrategyView,
//! avoiding HTTP serialization/deserialization overhead. It's suitable for
//! strategy simulation and performance testing.

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
    pub fn play_game(&self, _seed: u64, _strategy: &InternalSmartStrategy) -> InternalGameResult {
        // TODO: Implement complete game loop with action execution
        // This is a placeholder skeleton for the optimized game driver.
        // Real implementation will:
        // 1. Create GameState with seed
        // 2. Loop until game ends
        // 3. Get possible actions from GameState::possible_actions()
        // 4. Use strategy.choose_action_from_view() with view_for_scoring()
        // 5. Execute actions via GameState::dispatch()
        // 6. Track results (tier reached, contracts completed, etc.)

        InternalGameResult::new(_seed)
    }
}
