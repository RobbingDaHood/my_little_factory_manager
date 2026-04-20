//! JSON configuration types for externalized game rules.
//!
//! These types are deserialized from JSON files under `configurations/` at
//! compile time and used to configure game behaviour.

use crate::types::{CardTag, MainEffectDirection, TokenType, VariationDirection};
use rocket::serde::Deserialize;

/// Top-level game rules loaded from `configurations/general/game_rules.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct GameRulesConfig {
    pub general: GeneralRules,
    pub contract_formulas: ContractFormulasConfig,
    pub adaptive_balance: AdaptiveBalanceConfig,
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
    pub harmful_token_limit: TierScalingFormula,
}

/// A linear tier-scaling formula: for a given tier, produces a range
/// `[base_min + tier × per_tier_min, base_max + tier × per_tier_max]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct TierScalingFormula {
    pub base_min: u32,
    pub base_max: u32,
    pub per_tier_min: u32,
    pub per_tier_max: u32,
}

/// A min/max range for a floating-point modifier.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ModifierRange {
    pub min: f64,
    pub max: f64,
}

// ---------------------------------------------------------------------------
// Token definitions configuration (replaces effect_types.json)
// ---------------------------------------------------------------------------

/// Top-level config loaded from `configurations/card_effects/token_definitions.json`.
///
/// Contains the design-intent parameters (~5 per token) from which
/// the combinatorial generator produces all CardEffectTypeConfig entries.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct TokenDefinitionsConfig {
    pub tokens: Vec<TokenDefinitionConfig>,
    pub variation_defaults: VariationDefaultsConfig,
}

/// One token's definition: type, classification, primary formula, and tags.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct TokenDefinitionConfig {
    pub token_type: TokenType,
    pub is_beneficial: bool,
    pub primary_formula: TierScalingFormula,
    pub producer_tags: Vec<CardTag>,
    pub consumer_tags: Vec<CardTag>,
}

/// Global defaults for variation generation.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct VariationDefaultsConfig {
    pub ratio_range: ModifierRange,
    pub efficiency_per_tier: f64,
    pub boost_factor: f64,
    pub items_per_tier: u32,
}

// ---------------------------------------------------------------------------
// Generated card effect types (in-memory, not serialized to JSON)
// ---------------------------------------------------------------------------

/// A generated card-effect type definition with its variations.
///
/// These are produced by the combinatorial generator from TokenDefinitionsConfig.
#[derive(Debug, Clone)]
pub struct CardEffectTypeConfig {
    pub name: String,
    pub tags: Vec<CardTag>,
    pub available_at_tier: u32,
    pub primary_token: TokenType,
    pub main_direction: MainEffectDirection,
    pub primary_formula: TierScalingFormula,
    pub variations: Vec<CardEffectVariation>,
}

/// A variation that modifies a main effect's primary output and adds a
/// secondary token exchange.
#[derive(Debug, Clone)]
pub struct CardEffectVariation {
    pub name: String,
    pub secondary_token: TokenType,
    pub direction: VariationDirection,
    pub is_self_consuming: bool,
    /// +1 = boosts primary (accepts harm / sacrifices beneficial input),
    /// -1 = costs primary (removes harm / gets extra beneficial output).
    pub direction_sign: i8,
    pub unlock_tier: u32,
}

// ---------------------------------------------------------------------------
// Adaptive balance configuration
// ---------------------------------------------------------------------------

/// Parameters for the adaptive balance system that adjusts contract difficulty
/// based on player behaviour patterns.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdaptiveBalanceConfig {
    /// Exponential moving average weight for new observations (0–1).
    pub alpha: f64,
    /// Per-contract decay multiplier for tokens not used (0–1).
    pub decay_rate: f64,
    /// Multiplier applied to ALL pressures on contract failure (0–1).
    pub failure_relaxation: f64,
    /// Maximum tightening percentage for HarmfulTokenLimit (0–1).
    pub max_tightening_pct: f64,
    /// Maximum increase percentage for OutputThreshold (0–1).
    pub max_increase_pct: f64,
    /// Normalization divisor to convert raw pressure into a 0–1 ratio.
    pub normalization_factor: f64,
}
