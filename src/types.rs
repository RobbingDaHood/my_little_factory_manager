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
    /// Basic material input for transformations.
    RawMaterial,

    // Harmful tokens
    /// Thermal byproduct from production processes.
    Heat,
    /// Carbon emissions from factory operations.
    CO2,
    /// Generic industrial waste.
    Waste,
    /// Environmental contamination.
    Pollution,

    // Progression tracking
    /// Number of contracts completed for a given tier (1-based, unbounded).
    ContractsTierCompleted(u32),
    /// Current deck size limit — cards in deck+hand+discard cannot exceed this.
    DeckSlots,
}

/// Classification tags for token types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub enum TokenTag {
    /// A positive resource (production units, energy, materials).
    Beneficial,
    /// A negative byproduct (heat, CO2, waste, pollution).
    Harmful,
    /// Tracks long-term progression (contracts completed per tier).
    Progression,
}

impl TokenType {
    /// Returns the classification tags for this token type (compile-time known).
    pub fn tags(&self) -> &'static [TokenTag] {
        match self {
            Self::ProductionUnit | Self::Energy | Self::RawMaterial => &[TokenTag::Beneficial],
            Self::Heat | Self::CO2 | Self::Waste | Self::Pollution => &[TokenTag::Harmful],
            Self::ContractsTierCompleted(_) | Self::DeckSlots => &[TokenTag::Progression],
        }
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
    /// Produce at least `min_amount` of `token_type` (mandatory on every contract).
    OutputThreshold {
        token_type: TokenType,
        min_amount: u32,
    },
    /// Complete without exceeding `max_amount` of a harmful `token_type`.
    HarmfulTokenLimit {
        token_type: TokenType,
        max_amount: u32,
    },
    /// Certain card tags are unavailable during this contract.
    CardTagRestriction { restricted_tag: CardTag },
    /// Contract must be completed between turn `min_turn` and `max_turn` (inclusive).
    TurnWindow { min_turn: u32, max_turn: u32 },
}

/// A concrete contract with requirements and a visible reward card.
///
/// The reward card is generated when the contract is generated — the player
/// can see exactly what card they would earn before accepting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct Contract {
    pub tier: ContractTier,
    pub requirements: Vec<ContractRequirementKind>,
    pub reward_card: PlayerActionCard,
}

/// A group of contract offers for a single tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TierContracts {
    pub tier: ContractTier,
    pub contracts: Vec<Contract>,
}
