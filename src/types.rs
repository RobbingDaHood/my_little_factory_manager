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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
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
            Self::ContractsTierCompleted(_) => &[TokenTag::Progression],
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

/// Concrete card effect variants.
///
/// Each effect has inputs (tokens consumed) and/or outputs (tokens produced).
/// At least one of inputs or outputs is non-empty. The variant name
/// communicates the intent/pattern of the effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde", tag = "effect_type")]
pub enum CardEffect {
    /// No input required; produces beneficial tokens in moderate amounts.
    PureProduction { outputs: Vec<TokenAmount> },
    /// Consumes beneficial tokens to produce beneficial tokens in larger amounts.
    Conversion {
        inputs: Vec<TokenAmount>,
        outputs: Vec<TokenAmount>,
    },
    /// Consumes harmful tokens with no output — removing waste is its own reward.
    WasteRemoval { inputs: Vec<TokenAmount> },
}

/// Where card copies reside during gameplay.
///
/// Cards move: Library → Deck → Hand → Discard → (shuffle back to Deck).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub enum CardLocation {
    /// The complete catalogue of available actions.
    Library,
    /// The player's current operational toolset (shuffled into hand).
    Deck,
    /// Actions available for the current turn.
    Hand,
    /// Used actions awaiting recycling back into the deck.
    Discard,
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
