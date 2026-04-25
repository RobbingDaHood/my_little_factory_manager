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
    /// Minimum turns played on the active contract before AbandonContract is allowed.
    pub min_turns_before_abandon: u32,
}

/// Formula parameters for contract and reward card generation.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ContractFormulasConfig {
    pub output_threshold: TierScalingFormula,
    pub harmful_token_limit: TierScalingFormula,
    #[serde(default)]
    pub turn_window: Option<TurnWindowFormulaConfig>,
    #[serde(default)]
    pub card_tag_constraint: Option<CardTagConstraintFormulaConfig>,
}

/// Formula config for TurnWindow requirement generation.
///
/// Three variants unlock progressively:
///   1. Only-Max (deadline): `max_turn` only — must complete before turn X. Unlocks at `unlock_tier_only_max`.
///   2. Only-Min (earliest-start): `min_turn` only — must not complete before turn X. Unlocks at `unlock_tier_only_min`.
///   3. Both (window): `min_turn` and `max_turn` — must complete between turns. Unlocks at `unlock_tier_both`.
///
/// `min_turn` rolls uniformly in `[0, min(base + tier×per_tier, max_min_turn)]` — 0 is always possible.
/// `window_size` rolls in `[window_size_min, window_size_min + extra]` where `extra` decreases with tier,
/// so higher tiers have tighter (harder) windows. `extra` is always ≥1 to ensure at least two possible values.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct TurnWindowFormulaConfig {
    /// Tier at which the Only-Max (deadline) variant unlocks.
    pub unlock_tier_only_max: u32,
    /// Tier at which the Only-Min (earliest-start) variant unlocks.
    pub unlock_tier_only_min: u32,
    /// Tier at which the Both (window) variant unlocks.
    pub unlock_tier_both: u32,
    pub min_turns_base: u32,
    pub min_turns_per_tier: u32,
    /// Maximum value min_turn can reach (caps the range to avoid boring wait turns).
    pub max_min_turn: u32,
    /// Minimum window size (turns) — the floor for all window rolls.
    pub window_size_min: u32,
    /// Extra window width added at the unlock tier; decreases by `window_size_extra_decrease_per_tier` each tier.
    pub window_size_extra_base: u32,
    pub window_size_extra_decrease_per_tier: u32,
}

/// Formula config for CardTagConstraint requirement generation.
///
/// Three variants unlock progressively:
///   1. Only-Max (upper limit/ban): `max` only — at most N cards of this tag. Unlocks at `unlock_tier_only_max`.
///   2. Only-Min (must-play): `min` only — must play at least N cards of this tag. Unlocks at `unlock_tier_only_min`.
///   3. Both (range): `min` and `max` — must play between N and M cards of this tag. Unlocks at `unlock_tier_both`.
///
/// Only-Max `max_count` rolls in `[0, max_count_at_tier]` where `max_count_at_tier` decreases with tier
/// (higher tier = tighter cap = harder). Rolling 0 is a full ban — the ban special-case is subsumed here.
/// Only-Min `min_count` rolls in `[0, min(min_count_per_tier × tier, min_count_cap)]` — increasing with tier.
/// Both uses `min_count` from the Only-Min formula plus a window (same decreasing pattern as TurnWindow).
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CardTagConstraintFormulaConfig {
    /// Tier at which the Only-Max (upper limit/ban) variant unlocks.
    pub unlock_tier_only_max: u32,
    /// Tier at which the Only-Min (must-play) variant unlocks.
    pub unlock_tier_only_min: u32,
    /// Tier at which the Both (range) variant unlocks.
    pub unlock_tier_both: u32,
    /// Maximum count at the unlock tier; decreases by `max_count_decrease_per_tier` each tier.
    /// Rolling 0 acts as a full ban.
    pub max_count_base: u32,
    pub max_count_decrease_per_tier: u32,
    /// Only-Min min_count scales as `min_count_per_tier × (tier - unlock_tier_only_min)`.
    pub min_count_per_tier: u32,
    /// Cap on min_count to prevent requirement from becoming impossibly high.
    pub min_count_cap: u32,
    /// Minimum window size for the Both variant (max_count = min_count + window).
    pub count_window_min: u32,
    /// Extra window width at the unlock tier; decreases by `count_window_extra_decrease_per_tier`.
    pub count_window_extra_base: u32,
    pub count_window_extra_decrease_per_tier: u32,
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

/// One token's definition: type, classification, and primary formula.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct TokenDefinitionConfig {
    pub token_type: TokenType,
    pub is_beneficial: bool,
    pub primary_formula: TierScalingFormula,
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
    /// Base input/output token signature for this effect type (no variation selected).
    pub tag: CardTag,
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
