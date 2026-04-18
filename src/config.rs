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
    pub contract_formulas: ContractFormulasConfig,
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
    /// Probability (0.0–1.0) that completing a contract awards +1 DeckSlots.
    pub deck_slot_reward_chance: f64,
}

/// Formula parameters for contract and reward card generation.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ContractFormulasConfig {
    pub output_threshold: TierScalingFormula,
    pub reward_production: TierScalingFormula,
}

/// A linear tier-scaling formula: for a given tier, produces a range
/// `[base_min + tier × per_tier_min, base_max + tier × per_tier_max]`.
/// Only active for tiers ≥ `min_tier`.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct TierScalingFormula {
    pub min_tier: u32,
    pub base_min: u32,
    pub base_max: u32,
    pub per_tier_min: u32,
    pub per_tier_max: u32,
}

// ---------------------------------------------------------------------------
// Card effect type configuration
// ---------------------------------------------------------------------------

/// A single card-effect type definition loaded from
/// `configurations/card_effects/effect_types.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CardEffectTypeConfig {
    /// Minimum contract tier where this effect type appears as a reward.
    pub min_tier: u32,
    /// Tags assigned to cards generated with this effect type.
    pub tags: Vec<String>,
    /// Token inputs consumed when the card is played.
    pub inputs: Vec<EffectFormula>,
    /// Token outputs produced when the card is played.
    pub outputs: Vec<EffectFormula>,
}

/// A formula pairing a token type name with a tier-scaling formula.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct EffectFormula {
    pub token_type: String,
    pub formula: TierScalingFormula,
}
