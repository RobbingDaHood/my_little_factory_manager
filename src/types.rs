//! Core game types: tokens, cards, effects, contracts, and locations.

use rocket::serde::{Deserialize, Serialize};
use schemars::JsonSchema;

// ---------------------------------------------------------------------------
// Token system
// ---------------------------------------------------------------------------

/// Resource and waste types used throughout the game.
///
/// Tokens are simple counters that persist between contracts. They are
/// produced and consumed by card effects and checked by contract requirements.
///
/// Introduction order (alternating beneficial/harmful):
/// 1. ProductionUnit (beneficial) — tier 0
/// 2. Heat (harmful) — tier 1
/// 3. Energy (beneficial) — tier 4
/// 4. Waste (harmful) — tier 9
/// 5. QualityPoint (beneficial) — tier 16
/// 6. Pollution (harmful) — tier 25
/// 7. Innovation (beneficial) — tier 36
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(crate = "rocket::serde")]
pub enum TokenType {
    // Beneficial tokens
    /// The main production output — mandatory in every contract.
    ProductionUnit,
    /// Energy resource — consumed by conversion effects, produced by some cards.
    Energy,
    /// Quality measure — produced by quality-focused operations.
    QualityPoint,
    /// Research and development output — produced by innovation-focused cards.
    Innovation,

    // Harmful tokens
    /// Thermal byproduct from production processes.
    Heat,
    /// Generic industrial waste.
    Waste,
    /// Environmental contamination.
    Pollution,

    // Progression tracking
    /// Number of contracts completed for a given tier (0-based, unbounded).
    ContractsTierCompleted(u32),
    /// Current active cycle size limit — cards in deck+hand+discard cannot exceed this.
    DeckSlots,
}

/// Classification tags for token types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub enum TokenTag {
    /// A positive resource (production units, energy, materials).
    Beneficial,
    /// A negative byproduct (heat, waste, pollution).
    Harmful,
    /// Tracks long-term progression (contracts completed per tier).
    Progression,
}

impl TokenType {
    /// Returns the classification tags for this token type (compile-time known).
    pub fn tags(&self) -> &'static [TokenTag] {
        match self {
            Self::ProductionUnit | Self::Energy | Self::QualityPoint | Self::Innovation => {
                &[TokenTag::Beneficial]
            }
            Self::Heat | Self::Waste | Self::Pollution => &[TokenTag::Harmful],
            Self::ContractsTierCompleted(_) | Self::DeckSlots => &[TokenTag::Progression],
        }
    }

    /// Whether this token type is beneficial to the player.
    pub fn is_beneficial(&self) -> bool {
        self.tags().contains(&TokenTag::Beneficial)
    }

    /// Whether this token type is harmful to the player.
    pub fn is_harmful(&self) -> bool {
        self.tags().contains(&TokenTag::Harmful)
    }
}

/// A specific quantity of a token type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TokenAmount {
    pub token_type: TokenType,
    pub amount: u32,
}

// ---------------------------------------------------------------------------
// Effect direction types
// ---------------------------------------------------------------------------

/// Whether a main card effect is a producer (outputs its token) or a
/// consumer/remover (inputs its token).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub enum MainEffectDirection {
    /// Primary formula → output amount (beneficial producers, harmful producers).
    Producer,
    /// Primary formula → input amount (beneficial consumers, harmful removers).
    Consumer,
}

/// Whether a variation's secondary token appears as an input or output.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub enum VariationDirection {
    Input,
    Output,
}

// ---------------------------------------------------------------------------
// Card system
// ---------------------------------------------------------------------------

/// Operational category tags for player action cards.
///
/// A card can have multiple tags indicating what kind of factory operation
/// it represents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub enum CardTag {
    /// Cards that generate output tokens.
    Production,
    /// Cards that convert one token type into another.
    Transformation,
    /// Cards that manage waste and harmful byproducts.
    QualityControl,
    /// Utility and meta-operational cards.
    SystemAdjustment,
}

/// A card effect with token inputs consumed and token outputs produced.
///
/// At least one of `inputs` or `outputs` must be non-empty. Deserialization
/// enforces this constraint; use `CardEffect::new()` for programmatic construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct CardEffect {
    #[serde(default)]
    pub inputs: Vec<TokenAmount>,
    #[serde(default)]
    pub outputs: Vec<TokenAmount>,
}

impl CardEffect {
    /// Creates a new `CardEffect`, returning an error if both inputs and outputs are empty.
    pub fn new(inputs: Vec<TokenAmount>, outputs: Vec<TokenAmount>) -> Result<Self, String> {
        if inputs.is_empty() && outputs.is_empty() {
            return Err("CardEffect must have at least one input or output".into());
        }
        Ok(Self { inputs, outputs })
    }
}

/// Raw deserialization helper that validates non-empty inputs/outputs.
#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct CardEffectRaw {
    #[serde(default)]
    inputs: Vec<TokenAmount>,
    #[serde(default)]
    outputs: Vec<TokenAmount>,
}

impl<'de> Deserialize<'de> for CardEffect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: rocket::serde::Deserializer<'de>,
    {
        let raw = CardEffectRaw::deserialize(deserializer)?;
        CardEffect::new(raw.inputs, raw.outputs).map_err(rocket::serde::de::Error::custom)
    }
}

/// Where card copies reside during gameplay.
///
/// Cards move: Shelved → Deck → Hand → Discard → (shuffle back to Deck).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub enum CardLocation {
    /// The complete catalogue of available actions — owned but not in the active cycle.
    Shelved,
    /// The player's current operational toolset (shuffled into hand).
    Deck,
    /// Actions available for the current turn.
    Hand,
    /// Used actions awaiting recycling back into the deck.
    Discard,
}

/// Per-location copy counts for a single card type.
///
/// Each field independently tracks copies at that location:
/// - `shelved` — copies on the shelf (owned but not in the active cycle).
/// - `deck` + `hand` + `discard` — copies in the active cycle.
///
/// Total owned = `shelved + deck + hand + discard`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct CardCounts {
    pub shelved: u32,
    pub deck: u32,
    pub hand: u32,
    pub discard: u32,
}

impl CardCounts {
    pub fn has_shelved(&self) -> bool {
        self.shelved > 0
    }

    pub fn has_non_shelved(&self) -> bool {
        self.deck + self.hand + self.discard > 0
    }

    pub fn non_shelved(&self) -> u32 {
        self.deck + self.hand + self.discard
    }

    pub fn total(&self) -> u32 {
        self.shelved + self.deck + self.hand + self.discard
    }
}

/// A card type with its per-location copy counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct CardEntry {
    pub card: PlayerActionCard,
    pub counts: CardCounts,
}

/// Add a copy of `card` to `entries`, placing it at the given `location`.
///
/// If an identical card already exists, its counts are incremented.
/// Otherwise a new `CardEntry` is appended.
/// Only the specified location's count is incremented.
pub fn add_card_to_entries(
    entries: &mut Vec<CardEntry>,
    card: &PlayerActionCard,
    location: CardLocation,
) {
    if let Some(entry) = entries.iter_mut().find(|e| e.card == *card) {
        increment_location_count(&mut entry.counts, &location);
    } else {
        let mut counts = CardCounts {
            shelved: 0,
            deck: 0,
            hand: 0,
            discard: 0,
        };
        increment_location_count(&mut counts, &location);
        entries.push(CardEntry {
            card: card.clone(),
            counts,
        });
    }
}

fn increment_location_count(counts: &mut CardCounts, location: &CardLocation) {
    match location {
        CardLocation::Shelved => counts.shelved += 1,
        CardLocation::Deck => counts.deck += 1,
        CardLocation::Hand => counts.hand += 1,
        CardLocation::Discard => counts.discard += 1,
    }
}

/// A concrete player action card with tags and effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct PlayerActionCard {
    pub tags: Vec<CardTag>,
    pub effects: Vec<CardEffect>,
}

// ---------------------------------------------------------------------------
// Contract system
// ---------------------------------------------------------------------------

/// Contract difficulty tier (1-based) representing increasing structural complexity.
///
/// Higher tiers introduce new requirement types, more complex combinations,
/// and access to stronger player action cards. Tiers are unbounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent, crate = "rocket::serde")]
pub struct ContractTier(pub u32);

/// Kinds of requirements a contract can impose.
///
/// A contract has a list of requirements; all must be satisfied simultaneously
/// for the contract to complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde", tag = "requirement_type")]
pub enum ContractRequirementKind {
    /// Token requirement with optional lower and upper bounds.
    ///
    /// - `min: Some(n)` — contract requires accumulating at least n of this token.
    /// - `max: Some(m)` — exceeding m tokens is an immediate failure; completion
    ///   also requires the balance to be ≤ m at the time of resolution.
    /// - Both bounds may be set simultaneously for dual-constraint requirements.
    ///
    /// Generation bias: beneficial tokens start with only `min` set; harmful
    /// tokens start with only `max` set. Higher tiers may add the second bound.
    TokenRequirement {
        token_type: TokenType,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<u32>,
    },
    /// Card-tag play count requirement with optional lower and upper bounds.
    ///
    /// - `min: Some(n)` — must play at least n cards with this tag before completion.
    /// - `max: Some(0)` — this tag is banned: no cards with this tag may be played.
    /// - `max: Some(m)` where m > 0 — at most m cards with this tag may be played.
    /// - Both bounds may be set to require playing between n and m cards of this tag.
    ///
    /// Only tags that have at least one card available at the contract's tier are
    /// ever generated.
    CardTagConstraint {
        tag: CardTag,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<u32>,
    },
    /// Contract turn-window constraint with optional lower and upper bounds.
    ///
    /// - `min_turn: Some(n)` — contract cannot complete before turn n (must wait).
    /// - `max_turn: Some(m)` — exceeding turn m is an immediate contract failure.
    /// - Both may be set for a strict window; either may be omitted for a one-sided constraint.
    ///
    /// Generation: three tier-gated variants unlock progressively.
    /// Only-Max (deadline) unlocks first; Only-Min (earliest-start) later; Both (window) last.
    TurnWindow {
        #[serde(skip_serializing_if = "Option::is_none")]
        min_turn: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_turn: Option<u32>,
    },
}

/// A concrete contract with requirements and a visible reward card.
///
/// The reward card is generated when the contract is generated — the player
/// can see exactly what card they would earn before accepting.
/// `adaptive_adjustments` shows how the adaptive balance system modified
/// each requirement from its base-rolled value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct Contract {
    pub tier: ContractTier,
    pub requirements: Vec<ContractRequirementKind>,
    pub reward_card: PlayerActionCard,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adaptive_adjustments: Vec<AdaptiveAdjustment>,
}

/// Records how the adaptive balance system modified a single contract requirement.
///
/// Stored on the contract so the player can see exactly what was adjusted and by
/// how much compared to the base-rolled value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct AdaptiveAdjustment {
    pub requirement_index: usize,
    pub original_value: u32,
    pub adjusted_value: u32,
    /// Negative = tightened (harder), positive = eased (easier).
    pub adjustment_percent: i32,
}

/// Outcome of a contract reaching resolution (success or failure).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "resolution_type", crate = "rocket::serde")]
pub enum ContractResolution {
    Completed {
        contract: Contract,
    },
    Failed {
        contract: Contract,
        reason: ContractFailureReason,
    },
}

/// Why a contract was failed.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "failure_type", crate = "rocket::serde")]
pub enum ContractFailureReason {
    /// A harmful token exceeded its limit during gameplay.
    HarmfulTokenLimitExceeded {
        token_type: TokenType,
        max_amount: u32,
        current_amount: u32,
    },
    /// The contract's turn window expired.
    TurnWindowExceeded { max_turn: u32, current_turn: u32 },
    /// The player voluntarily abandoned the contract.
    ///
    /// Only allowed after `min_turns_before_abandon` turns have been played.
    /// Counts as a failure in all failure metrics.
    Abandoned { turns_played: u32 },
}

/// A group of contract offers for a single tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TierContracts {
    pub tier: ContractTier,
    pub contracts: Vec<Contract>,
}
