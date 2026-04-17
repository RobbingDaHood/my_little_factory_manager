//! Designer guide: contract, card, token, and effect authoring reference.

use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket_okapi::{openapi, JsonSchema};

/// A reference entry for a single game concept.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct ReferenceEntry {
    pub name: String,
    pub description: String,
}

/// A section in the designer guide.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct DesignerSection {
    pub title: String,
    pub description: String,
    pub entries: Vec<ReferenceEntry>,
}

/// Complete designer guide for understanding and authoring game content.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct DesignerGuide {
    pub title: String,
    pub introduction: String,
    pub sections: Vec<DesignerSection>,
}

fn build_designer_guide() -> DesignerGuide {
    DesignerGuide {
        title: "My Little Factory Manager — Designer Guide".to_string(),
        introduction: "This guide describes how contracts, cards, tokens, and effects \
            are structured. Use it to understand the building blocks of the game and the \
            formula-based balance system. The game follows a single unified production \
            mechanic — contracts vary by their requirement combinations, not by different \
            resolution systems."
            .to_string(),
        sections: vec![
            build_token_types(),
            build_card_effects(),
            build_contract_requirements(),
            build_tier_system(),
            build_card_locations(),
            build_deckbuilding(),
            build_configuration(),
            build_determinism(),
        ],
    }
}

fn build_token_types() -> DesignerSection {
    DesignerSection {
        title: "Token Types".to_string(),
        description: "Tokens are simple counters that persist between contracts. \
            Each token type belongs to one category: beneficial, harmful, or progression. \
            Token types are enum-based with no lifecycle — they are added and removed \
            by card effects and checked by contract requirements."
            .to_string(),
        entries: vec![
            ReferenceEntry {
                name: "ProductionUnit (Beneficial)".to_string(),
                description: "The main production output — mandatory in every contract's \
                    OutputThreshold requirement. Produced by production cards and consumed \
                    when contracts complete."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Energy (Beneficial)".to_string(),
                description: "Energy resource consumed by conversion effects and produced \
                    by some cards. Used as input for advanced card effects in higher tiers."
                    .to_string(),
            },
            ReferenceEntry {
                name: "RawMaterial (Beneficial)".to_string(),
                description: "Basic material input for transformation effects. Converted \
                    into other beneficial tokens at favorable ratios."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Heat (Harmful)".to_string(),
                description: "Thermal byproduct from production processes. Accumulates \
                    during card plays and may be constrained by contract requirements."
                    .to_string(),
            },
            ReferenceEntry {
                name: "CO2 (Harmful)".to_string(),
                description: "Carbon emissions from factory operations. Higher-output \
                    production cards may produce CO2 as a tradeoff."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Waste (Harmful)".to_string(),
                description: "Generic industrial waste. Can be removed by QualityControl \
                    cards with WasteRemoval effects."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Pollution (Harmful)".to_string(),
                description: "Environmental contamination. The most severe harmful token — \
                    contracts in higher tiers may impose strict Pollution limits."
                    .to_string(),
            },
            ReferenceEntry {
                name: "ContractsTierCompleted(N) (Progression)".to_string(),
                description: "Tracks the number of contracts completed at tier N. \
                    When this reaches the advancement threshold (default: 10), the next \
                    tier unlocks."
                    .to_string(),
            },
            ReferenceEntry {
                name: "DeckSlots (Progression)".to_string(),
                description: "Controls the maximum number of cards in the active cycle \
                    (deck + hand + discard). Initialized to starting_deck_size (default: 10). \
                    Each contract completion has a 25% chance to award +1 DeckSlots. When \
                    the active cycle is at the limit, reward cards go to the library shelf \
                    instead of entering the deck."
                    .to_string(),
            },
        ],
    }
}

fn build_card_effects() -> DesignerSection {
    DesignerSection {
        title: "Card Effects".to_string(),
        description: "Each card has a list of effects. Each effect has inputs (tokens \
            consumed) and outputs (tokens produced). At least one must be non-empty. \
            Effect variants represent different operational categories."
            .to_string(),
        entries: vec![
            ReferenceEntry {
                name: "PureProduction".to_string(),
                description: "No inputs required. Produces beneficial tokens (typically \
                    ProductionUnit). The simplest effect type — all starter deck cards \
                    use this. Higher-tier variants may produce larger amounts but also \
                    produce harmful byproducts."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Conversion".to_string(),
                description: "Consumes one beneficial token type and produces a different \
                    beneficial token type in a larger amount. The conversion ratio improves \
                    with tier — higher tiers offer better exchange rates."
                    .to_string(),
            },
            ReferenceEntry {
                name: "WasteRemoval".to_string(),
                description: "Consumes harmful tokens (Heat, CO2, Waste, Pollution) and \
                    produces nothing or a small amount of beneficial output. Removing \
                    harmful tokens is its own reward — these effects have intentionally \
                    lower beneficial output."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Mixed Variants".to_string(),
                description: "Combinations of the above with varying input/output ratios. \
                    More powerful beneficial output comes with tradeoffs — either consuming \
                    valuable inputs or producing harmful byproducts."
                    .to_string(),
            },
        ],
    }
}

fn build_contract_requirements() -> DesignerSection {
    DesignerSection {
        title: "Contract Requirements".to_string(),
        description: "Each contract has a list of requirements, all of which must be \
            satisfied simultaneously for the contract to auto-complete. Requirement \
            types are introduced progressively as tiers unlock."
            .to_string(),
        entries: vec![
            ReferenceEntry {
                name: "OutputThreshold".to_string(),
                description: "Produce at least min_amount of a token_type (always \
                    ProductionUnit in current tiers). Mandatory on every contract. The \
                    threshold scales with tier via the output_threshold formula."
                    .to_string(),
            },
            ReferenceEntry {
                name: "HarmfulTokenLimit".to_string(),
                description: "Complete the contract without exceeding max_amount of a \
                    specific harmful token type. Forces players to manage waste and \
                    pollution alongside production. Introduced in higher tiers."
                    .to_string(),
            },
            ReferenceEntry {
                name: "CardTagRestriction".to_string(),
                description: "Certain card tags are unavailable during this contract. \
                    Forces players to work with a restricted subset of their deck. \
                    Introduced in higher tiers."
                    .to_string(),
            },
            ReferenceEntry {
                name: "TurnWindow".to_string(),
                description: "The contract must be completed between min_turn and \
                    max_turn (inclusive). Creates time pressure and rewards efficient \
                    play. Introduced in higher tiers."
                    .to_string(),
            },
        ],
    }
}

fn build_tier_system() -> DesignerSection {
    DesignerSection {
        title: "Contract Tier System".to_string(),
        description: "Contracts are organized into tiers of increasing structural \
            complexity. Each tier introduces new requirement types and stronger reward \
            cards. The system uses formula-based generation — one definition per \
            effect/requirement type that scales with tier, not one per tier."
            .to_string(),
        entries: vec![
            ReferenceEntry {
                name: "Tier Structure".to_string(),
                description: "A tier X contract has max(X−1, 1) to X+1 requirements \
                    (minimum 1). Each requirement is of tier X−1 to X+1 difficulty. \
                    Higher tiers mean more requirements with higher thresholds."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Tier Progression".to_string(),
                description: "Completing 10 contracts in tier N unlocks tier N+1. \
                    Progress is tracked via ContractsTierCompleted(N) tokens. Tier 1 \
                    is always unlocked."
                    .to_string(),
            },
            ReferenceEntry {
                name: "TierScalingFormula".to_string(),
                description: "Each effect/requirement type uses a linear scaling formula: \
                    range = [base_min + tier × per_tier_min, base_max + tier × per_tier_max]. \
                    Concrete values are rolled deterministically within this range. The \
                    formula has a min_tier gate — types only appear at or above their min_tier."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Reward Cards".to_string(),
                description: "Each contract generates a reward card at creation time \
                    (visible before accepting). The reward has the same number of effects \
                    as the contract has requirements. Each effect matches the tier of a \
                    corresponding requirement. Concrete values are rolled from tier formulas."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Contract Market".to_string(),
                description: "3 contracts are offered per unlocked tier. The market refills \
                    (not regenerates) after each completion — remaining contracts stay. \
                    New contracts are generated using the seeded RNG."
                    .to_string(),
            },
        ],
    }
}

fn build_card_locations() -> DesignerSection {
    DesignerSection {
        title: "Card Locations".to_string(),
        description: "Cards move between distinct locations during gameplay. The system \
            uses count-based tracking — each card entry has a count per location rather \
            than individual card objects."
            .to_string(),
        entries: vec![
            ReferenceEntry {
                name: "Library".to_string(),
                description: "The total number of copies of this card the player owns. \
                    Grows when reward cards are earned. library >= deck + hand + discard. \
                    The difference (library - active) represents shelved copies — owned but \
                    not in the active deck cycle."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Deck".to_string(),
                description: "Copies available to be drawn. Cards are drawn randomly \
                    proportional to deck counts. When deck is empty, discard counts \
                    are moved back to deck."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Hand".to_string(),
                description: "Copies currently in the player's hand, available to play \
                    or discard. Hand persists between contracts."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Discard".to_string(),
                description: "Copies that have been played or discarded. Recycled back \
                    to deck when the deck runs empty."
                    .to_string(),
            },
        ],
    }
}

fn build_deckbuilding() -> DesignerSection {
    DesignerSection {
        title: "Deckbuilding".to_string(),
        description: "Between contracts, players can reshape their active deck using the \
            ReplaceCard action. The deck size is limited by the DeckSlots token. Cards \
            that exceed the limit are shelved in the library."
            .to_string(),
        entries: vec![
            ReferenceEntry {
                name: "DeckSlots".to_string(),
                description: "Limits active deck size (deck + hand + discard). Initialized \
                    to starting_deck_size. Each contract completion has a configurable chance \
                    (deck_slot_reward_chance, default 25%) to award +1 DeckSlots."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Shelved Cards".to_string(),
                description: "Cards with library count > (deck + hand + discard) have \
                    shelved copies. These are owned but not in the active cycle. Shelved \
                    cards can be moved into the deck via ReplaceCard."
                    .to_string(),
            },
            ReferenceEntry {
                name: "ReplaceCard Action".to_string(),
                description: "The sole deckbuilding action. Swaps a target card (in Deck \
                    or Discard) with a shelved replacement card. Permanently destroys a \
                    third sacrifice card (library count decremented). Only available \
                    between contracts."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Reward Card Placement".to_string(),
                description: "When a contract is completed, the reward card enters the \
                    library. If the active cycle is under the DeckSlots limit, it also \
                    enters the deck. Otherwise it is shelved."
                    .to_string(),
            },
        ],
    }
}

fn build_configuration() -> DesignerSection {
    DesignerSection {
        title: "Game Configuration".to_string(),
        description: "All game constants are externalized to JSON files under \
            configurations/ and embedded at compile time. No runtime file I/O needed."
            .to_string(),
        entries: vec![
            ReferenceEntry {
                name: "configurations/general/game_rules.json".to_string(),
                description: "Contains general rules (starting_hand_size, \
                    starting_deck_size, contracts_per_tier_to_advance, \
                    contract_market_size_per_tier, discard_production_unit_bonus, \
                    deck_slot_reward_chance) \
                    and contract formula parameters (output_threshold, \
                    reward_production scaling formulas)."
                    .to_string(),
            },
            ReferenceEntry {
                name: "configurations/card_effects/effect_types.json".to_string(),
                description: "Defines card effect types with per-tier availability. Each \
                    type specifies: name, min_tier, tags, input formulas, and output \
                    formulas. Tier 1 has pure_production only. Higher tiers add \
                    boosted_production and energy_production."
                    .to_string(),
            },
            ReferenceEntry {
                name: "TierScalingFormula fields".to_string(),
                description: "Each formula has: min_tier (when it activates), \
                    base_min, base_max (constant component), per_tier_min, \
                    per_tier_max (linear scaling per tier). Value range = \
                    [base_min + tier × per_tier_min, base_max + tier × per_tier_max]."
                    .to_string(),
            },
        ],
    }
}

fn build_determinism() -> DesignerSection {
    DesignerSection {
        title: "Determinism & Reproducibility".to_string(),
        description: "Given the same game version, same seed, and same ordered \
            list of player actions, the game deterministically produces the exact \
            same state. This is the foundation for save/load and testing."
            .to_string(),
        entries: vec![
            ReferenceEntry {
                name: "Seeded RNG".to_string(),
                description: "The game uses rand_pcg::Pcg64 initialized with the \
                    game seed. All random decisions (card draws, contract generation) \
                    flow through this single RNG."
                    .to_string(),
            },
            ReferenceEntry {
                name: "Action Log as Save File".to_string(),
                description: "GET /actions/history returns every player action taken. \
                    Replaying these actions on a fresh game with the same seed reproduces \
                    the exact same state. No separate save file format needed."
                    .to_string(),
            },
        ],
    }
}

/// Designer reference for contract, card, token, and effect authoring.
///
/// Returns a structured guide covering all game building blocks: token types
/// and their categories, card effects and their variants, contract requirement
/// types, the tier system with formula-based scaling, card locations, game
/// configuration, and the determinism model. Useful for understanding how
/// game content is structured and how to reason about balance.
#[openapi]
#[get("/docs/designer")]
pub fn get_designer_guide() -> Json<DesignerGuide> {
    Json(build_designer_guide())
}
