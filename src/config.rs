//! JSON configuration types for externalized game rules.
//!
//! These types are deserialized from JSON files under `configurations/` at
//! compile time and used to configure game behaviour.

use rocket::serde::Deserialize;

/// Top-level game rules loaded from `configurations/general/game_rules.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct GameRulesConfig {
    pub general: GeneralRules,
}

/// General (non-tier-specific) game constants.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct GeneralRules {
    /// Number of cards in the player's starting hand.
    pub starting_hand_size: u32,
    /// Number of cards in the player's starting deck.
    pub starting_deck_size: u32,
    /// Contracts completed in a tier before the next tier unlocks.
    pub contracts_per_tier_to_advance: u32,
    /// Number of contracts offered in the market per unlocked tier.
    pub contract_market_size_per_tier: u32,
    /// Production units gained when discarding a card for baseline benefit.
    pub discard_production_unit_bonus: u32,
}
