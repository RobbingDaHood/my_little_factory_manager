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
/// Only active for tiers ≥ `min_tier` (defaults to 1 when omitted).
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct TierScalingFormula {
    #[serde(default = "default_min_tier")]
    pub min_tier: u32,
    pub base_min: u32,
    pub base_max: u32,
    pub per_tier_min: u32,
    pub per_tier_max: u32,
}

fn default_min_tier() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Card effect type configuration
// ---------------------------------------------------------------------------

/// A single card-effect type definition loaded from
/// `configurations/card_effects/effect_types.json`.
///
/// Represents a root effect type (e.g., pure production) with optional
/// variations that modify the root's primary output and add extra token
/// exchanges.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CardEffectTypeConfig {
    /// Minimum contract tier where this effect type appears as a reward.
    pub unlocked_at_tier: u32,
    /// Tags assigned to cards generated with this effect type.
    pub tags: Vec<String>,
    /// Token inputs consumed when the card is played (root effect).
    pub inputs: Vec<EffectFormula>,
    /// Token outputs produced when the card is played (root effect).
    pub outputs: Vec<EffectFormula>,
    /// Optional variations that build on the root effect with a modifier
    /// and additional token exchanges.
    #[serde(default)]
    pub variations: Vec<CardEffectVariation>,
}

/// A variation of a root card effect type. When selected, the root's
/// primary output is rolled first, then multiplied by a modifier
/// rolled from `modifier_range`, and the variation's extra inputs/outputs
/// are appended.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CardEffectVariation {
    /// Tier at which this variation becomes available.
    pub unlocked_at_tier: u32,
    /// Multiplier range applied to the root's primary output amount.
    pub modifier_range: ModifierRange,
    /// Extra token inputs added by this variation.
    #[serde(default)]
    pub extra_inputs: Vec<EffectFormula>,
    /// Extra token outputs added by this variation.
    #[serde(default)]
    pub extra_outputs: Vec<EffectFormula>,
}

/// A min/max range for a floating-point modifier.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ModifierRange {
    pub min: f64,
    pub max: f64,
}

/// A formula pairing a token type name with a tier-scaling formula.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct EffectFormula {
    pub token_type: String,
    pub formula: TierScalingFormula,
}
