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
            "Between contracts, use ReplaceCard to swap weak deck cards for strong shelved reward cards.".to_string(),
            "ReplaceCard costs a sacrifice from shelved copies — choose carefully which card to permanently destroy.".to_string(),
            "Reward cards always go to the shelf — use ReplaceCard to bring them into your active cycle.".to_string(),
            "Use /metrics to track your efficiency — lower average cards per contract means you're improving.".to_string(),
            "Contracts can fail — check HarmfulTokenLimit requirements before accepting to avoid nasty surprises.".to_string(),
            "After a contract failure, the adaptive system eases difficulty — it's okay to fail occasionally.".to_string(),
            "Check adaptive_adjustments on offered contracts to see how the game is adapting to your style.".to_string(),
            "Diversify your strategies to keep adaptive pressure balanced — specialization gets punished over time.".to_string(),
        ],
        tiers: vec![
            build_tier0_hints(),
            build_tier1_hints(),
            build_tier6_hints(),
            build_tier12_hints(),
        ],
    }
}

fn build_tier0_hints() -> TierHints {
    TierHints {
        tier: 0,
        overview: "Tier 0 contracts have a single TokenRequirement min (formerly OutputThreshold): \
            produce enough ProductionUnits. Your starter deck contains only pure production cards \
            with no inputs required."
            .to_string(),
        strategies: vec![
            Strategy {
                name: "Focus on high-output cards".to_string(),
                description: "Prioritize playing cards that produce 5-7 ProductionUnits \
                    over weaker cards. Save discards for the weakest cards in your hand."
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
                description: "Each completed contract adds its reward card to the \
                    shelf. Use ReplaceCard between contracts to swap these stronger cards \
                    into your active cycle."
                    .to_string(),
            },
            Strategy {
                name: "Replace weak starter cards".to_string(),
                description: "Once you have shelved reward cards, use ReplaceCard between \
                    contracts to swap weak 2-ProductionUnit starter cards for stronger rewards. \
                    Sacrifice the weakest card you own to minimize loss."
                    .to_string(),
            },
        ],
        common_pitfalls: vec![
            "Discarding too much — the 1 ProductionUnit bonus adds up slowly compared to playing cards.".to_string(),
            "Ignoring the reward card preview — always compare reward cards between available contracts.".to_string(),
            "Not checking /state between plays — you might already meet the contract threshold.".to_string(),
        ],
        tips: vec![
            "Starter cards produce 2-7 ProductionUnits per play (generated via tier 0 formula).".to_string(),
            "Tier 0 thresholds range from 5-15 ProductionUnits.".to_string(),
            "The market always has 3 contracts available per tier.".to_string(),
            "After completing a contract, the market refills (not regenerates) — remaining contracts stay.".to_string(),
        ],
    }
}

fn build_tier1_hints() -> TierHints {
    TierHints {
        tier: 1,
        overview: "Tier 1 introduces Heat — your first harmful token. Contracts may \
            include a TokenRequirement max (formerly HarmfulTokenLimit) constraining how \
            much Heat you can accumulate. Heat producer and remover card effects become \
            available, along with self-consuming variations."
            .to_string(),
        strategies: vec![
            Strategy {
                name: "Balance production and Heat management".to_string(),
                description: "Some reward cards produce Heat as a byproduct of higher \
                    output. Watch for TokenRequirement max constraints — you \
                    may need Heat removal cards in your deck."
                    .to_string(),
            },
            Strategy {
                name: "Use self-consuming variations".to_string(),
                description: "Self-consuming variations (e.g., PU producer that also \
                    consumes PU) are strictly better than pure effects due to the \
                    boost_factor. Seek these in reward cards."
                    .to_string(),
            },
            Strategy {
                name: "Consider variation tradeoffs".to_string(),
                description: "Variations with harmful output (Heat) boost your primary \
                    production. Variations that remove harm or produce extra beneficial \
                    tokens reduce it. Choose based on your contract requirements."
                    .to_string(),
            },
            Strategy {
                name: "Manage harmful tokens proactively".to_string(),
                description: "Harmful tokens persist between contracts. If you accept a \
                    contract with a tight max bound and you're already near the cap, \
                    you risk immediate failure. Clean up before accepting tight contracts."
                    .to_string(),
            },
            Strategy {
                name: "Adapt to the adaptive system".to_string(),
                description: "If contracts keep tightening Heat max bounds, the game is \
                    pushing you to diversify. Try strategies that produce less Heat, or \
                    invest in Heat removal cards. The pressure relaxes once you shift."
                    .to_string(),
            },
        ],
        common_pitfalls: vec![
            "Ignoring Heat accumulation — TokenRequirement max contracts will fail if Heat spirals.".to_string(),
            "Only chasing raw ProductionUnit output — balance matters more than max output.".to_string(),
            "Accepting contracts with tight max bounds when your harmful token balance is already high.".to_string(),
            "Over-specializing in one strategy — the adaptive system gradually tightens requirements on dominant approaches.".to_string(),
        ],
        tips: vec![
            "Heat is the first harmful token introduced at tier 1.".to_string(),
            "TokenRequirement max bounds cap how much of a harmful token you can have.".to_string(),
            "Variation effects with direction_sign +1 boost primary output (tradeoff for the player).".to_string(),
            "Variation effects with direction_sign -1 reduce primary output (advantage for the player).".to_string(),
        ],
    }
}

fn build_tier6_hints() -> TierHints {
    TierHints {
        tier: 6,
        overview: "Tier 6 introduces TurnWindow requirements in three progressive variants. \
            Only-Max (deadline): max_turn set — exceeding it fails immediately. \
            Only-Min (earliest-start, unlocks at tier 10): min_turn set — cannot complete early. \
            Both (window, unlocks at tier 14): must complete between turns. \
            Windows narrow at higher tiers, rewarding efficient deck construction."
            .to_string(),
        strategies: vec![
            Strategy {
                name: "Prioritize high-output cards".to_string(),
                description: "With a hard turn limit, you cannot afford weak plays. \
                    Build your deck to consistently hit high output per turn so you \
                    reach the completion threshold within the window."
                    .to_string(),
            },
            Strategy {
                name: "Know your expected turns-to-complete".to_string(),
                description: "Before accepting a TurnWindow contract, estimate how many \
                    turns your current deck needs. If the deadline is 8 turns and you \
                    typically need 12, you have little margin — consider a different contract."
                    .to_string(),
            },
            Strategy {
                name: "Balance speed and patience".to_string(),
                description: "Only-Min contracts (min_turn only) reward patience — you cannot \
                    rush them. Only-Max is pure speed pressure. Full-window contracts require both. \
                    Choose the variant that fits your current deck's cadence."
                    .to_string(),
            },
        ],
        common_pitfalls: vec![
            "Accepting a tight-deadline contract before optimizing your deck.".to_string(),
            "Forgetting to check contract_turns_played in /state — missing how close you are to max_turn.".to_string(),
            "Playing cards that don't contribute to completion — every turn matters with a window constraint.".to_string(),
        ],
        tips: vec![
            "TurnWindow unlocks at tier 6 (Energy→Waste gap in the token unlock schedule).".to_string(),
            "contract_turns_played in /state shows how many turns you've used.".to_string(),
            "Only-Min contracts (min_turn only) cannot fail from the turn constraint — only from token bounds.".to_string(),
            "max_turn is hard — exceeding it immediately fails the contract with no reward.".to_string(),
        ],
    }
}

fn build_tier12_hints() -> TierHints {
    TierHints {
        tier: 12,
        overview: "Tier 12 introduces CardTagConstraint requirements in three progressive variants. \
            Only-Max (tier 12): max only — at most N cards with this tag; 0 is a full ban. \
            Only-Min (tier 16): min only — must play at least N cards with this tag. \
            Both (tier 20): range — must play between N and M cards of this tag. \
            The valid_card_indices in /actions/possible already filters out banned/over-limit cards."
            .to_string(),
        strategies: vec![
            Strategy {
                name: "Check tag constraints before accepting".to_string(),
                description: "Read the CardTagConstraint requirements carefully before \
                    accepting. A banned tag (max=0) you rely on heavily may make the contract \
                    very difficult. Choose contracts compatible with your deck composition."
                    .to_string(),
            },
            Strategy {
                name: "Build a balanced deck across tags".to_string(),
                description: "If your deck is all Production cards, a contract banning \
                    Production will be nearly impossible. Diversify your deck across \
                    Production, Transformation, and QualityControl to handle any ban."
                    .to_string(),
            },
            Strategy {
                name: "Leverage must-play constraints strategically".to_string(),
                description: "Contracts with a min CardTagConstraint (must play N of a tag) \
                    pair well with decks strong in that tag. Accept these contracts when \
                    your deck naturally plays many cards of the required type."
                    .to_string(),
            },
        ],
        common_pitfalls: vec![
            "Accepting a contract that bans your primary strategy tag without a backup plan.".to_string(),
            "Not checking /actions/possible — banned tag plays are blocked, not silently ignored.".to_string(),
            "Ignoring min constraints — you must play the required number of tagged cards or the contract won't complete.".to_string(),
        ],
        tips: vec![
            "CardTagConstraint unlocks at tier 12 (Waste→QP gap in the token unlock schedule).".to_string(),
            "valid_card_indices in /actions/possible already excludes banned/over-limit cards.".to_string(),
            "cards_played_per_tag_contract in /state shows your progress against tag constraints.".to_string(),
            "Tag constraints reset with each new contract — they track per-contract play counts.".to_string(),
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
