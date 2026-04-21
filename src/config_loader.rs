//! Config loader: reads JSON config files embedded at compile time.
//!
//! JSON files under `configurations/` are embedded via `include_str!()` and
//! parsed at initialization.

use super::config::{GameRulesConfig, TokenDefinitionsConfig};

static GAME_RULES_JSON: &str = include_str!("../configurations/general/game_rules.json");
static TOKEN_DEFINITIONS_JSON: &str =
    include_str!("../configurations/card_effects/token_definitions.json");

/// FNV-1a 64-bit hash of a byte string.
pub fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Deterministic fingerprint of all embedded config files (FNV-1a XOR, hex).
pub fn config_hash() -> String {
    let h = fnv1a_hash(GAME_RULES_JSON.as_bytes()) ^ fnv1a_hash(TOKEN_DEFINITIONS_JSON.as_bytes());
    format!("{h:016x}")
}

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

/// Load token definitions from the embedded configuration.
///
/// # Errors
///
/// Returns a `serde_json::Error` if the embedded JSON is malformed.
pub fn load_token_definitions() -> Result<TokenDefinitionsConfig, serde_json::Error> {
    serde_json::from_str(TOKEN_DEFINITIONS_JSON)
}
