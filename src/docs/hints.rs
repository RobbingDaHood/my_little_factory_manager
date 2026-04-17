//! Strategy hints and tips endpoint.

use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket_okapi::{openapi, JsonSchema};

/// A named strategy with description.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct Strategy {
    pub name: String,
    pub description: String,
}

/// Hints and strategies for a specific contract tier.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TierHints {
    pub tier: u32,
    pub overview: String,
    pub strategies: Vec<Strategy>,
    pub common_pitfalls: Vec<String>,
    pub tips: Vec<String>,
}

/// Complete hints guide covering general gameplay and per-tier strategies.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct HintsGuide {
    pub title: String,
    pub general_tips: Vec<String>,
    pub tiers: Vec<TierHints>,
}

fn build_hints() -> HintsGuide {
    HintsGuide {
        title: "My Little Factory Manager — Hints & Strategies".to_string(),
        general_tips: vec![
            "Always check /actions/possible before acting — it shows exactly what's valid.".to_string(),
            "Use /player/tokens to monitor your resource levels between card plays.".to_string(),
            "Your hand persists between contracts, so plan ahead.".to_string(),
            "Playing a card is almost always better than discarding — the discard bonus is intentionally small.".to_string(),
            "Check the reward card preview before accepting a contract — some rewards are better than others.".to_string(),
            "Completing 10 contracts in a tier unlocks the next tier with new challenges and stronger reward cards.".to_string(),
            "The seed + action log is your save file — use GET /actions/history to export it.".to_string(),
        ],
        tiers: vec![
            build_tier1_hints(),
        ],
    }
}

fn build_tier1_hints() -> TierHints {
    TierHints {
        tier: 1,
        overview: "Tier 1 contracts have a single OutputThreshold requirement: produce \
            enough ProductionUnits. Your starter deck contains only pure production cards \
            with no inputs required."
            .to_string(),
        strategies: vec![
            Strategy {
                name: "Focus on high-output cards".to_string(),
                description: "Prioritize playing cards that produce 2-3 ProductionUnits \
                    over 1-unit cards. Save discards for the weakest cards in your hand."
                    .to_string(),
            },
            Strategy {
                name: "Choose lower-threshold contracts first".to_string(),
                description: "When starting out, pick contracts with the lowest \
                    ProductionUnit threshold. This lets you complete them faster and \
                    accumulate reward cards that strengthen your deck."
                    .to_string(),
            },
            Strategy {
                name: "Build your deck through rewards".to_string(),
                description: "Each completed contract adds its reward card to your deck. \
                    These new cards improve your production capacity for future contracts."
                    .to_string(),
            },
        ],
        common_pitfalls: vec![
            "Discarding too much — the 1 ProductionUnit bonus adds up slowly compared to playing cards.".to_string(),
            "Ignoring the reward card preview — always compare reward cards between available contracts.".to_string(),
            "Not checking /state between plays — you might already meet the contract threshold.".to_string(),
        ],
        tips: vec![
            "Starter cards produce 1, 2, or 3 ProductionUnits per play.".to_string(),
            "Tier 1 thresholds range from 5-15 ProductionUnits.".to_string(),
            "The market always has 3 contracts available per tier.".to_string(),
            "After completing a contract, the market refills (not regenerates) — remaining contracts stay.".to_string(),
        ],
    }
}

/// Strategy hints and tips for each contract tier.
///
/// Returns general gameplay tips and tier-specific strategies, common
/// pitfalls, and tactical advice. Useful for players looking to optimize
/// their approach or understand the strategic depth of each tier level.
#[openapi]
#[get("/docs/hints")]
pub fn get_hints() -> Json<HintsGuide> {
    Json(build_hints())
}
