//! Config loader: reads JSON config files embedded at compile time.
//!
//! JSON files under `configurations/` are embedded via `include_str!()` and
//! parsed at initialization.

use super::config::{CardEffectTypeConfig, GameRulesConfig};

static GAME_RULES_JSON: &str = include_str!("../configurations/general/game_rules.json");
static EFFECT_TYPES_JSON: &str = include_str!("../configurations/card_effects/effect_types.json");

/// Load game rules from the embedded configuration.
///
/// # Errors
///
/// Returns a `serde_json::Error` if the embedded JSON is malformed.
pub fn load_game_rules() -> Result<GameRulesConfig, serde_json::Error> {
    load_game_rules_from_json(GAME_RULES_JSON)
}

/// Load game rules from a custom JSON string (useful for testing).
///
/// # Errors
///
/// Returns a `serde_json::Error` if the JSON is malformed.
pub fn load_game_rules_from_json(json: &str) -> Result<GameRulesConfig, serde_json::Error> {
    serde_json::from_str(json)
}

/// Load card effect type definitions from the embedded configuration.
///
/// # Errors
///
/// Returns a `serde_json::Error` if the embedded JSON is malformed.
pub fn load_effect_types() -> Result<Vec<CardEffectTypeConfig>, serde_json::Error> {
    serde_json::from_str(EFFECT_TYPES_JSON)
}
