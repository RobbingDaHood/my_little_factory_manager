use std::cell::Cell;
use std::collections::HashMap;

use serde_json::{json, Value};

use crate::game_driver::GameSnapshot;
use crate::strategies::Strategy;

const DISCARD_STUCK_THRESHOLD: u32 = 50;

/// Contract-aware strategy with active deckbuilding.
///
/// Uses `GET /state` alongside `GET /actions/possible` to:
/// 1. Deckbuild between contracts — replacing weak deck cards with strong shelved reward cards.
/// 2. Play cards that progress toward the active contract's requirements.
/// 3. Avoid playing cards that would push harmful tokens past their contract limits.
/// 4. Choose the contract with the best reward card and most feasible requirements.
///
/// Action priority:
///   ReplaceCard (beneficial swap) → PlayCard (best score) → AcceptContract (best score)
///   → DiscardCard (worst score) → AbandonContract (last resort, or after stuck detection)
pub struct SmartStrategy {
    consecutive_discards: Cell<u32>,
}

impl SmartStrategy {
    pub fn new() -> Self {
        Self {
            consecutive_discards: Cell::new(0),
        }
    }

    // -------------------------------------------------------------------
    // State helpers
    // -------------------------------------------------------------------

    fn token_balances(state: &Value) -> HashMap<String, i64> {
        let mut balances = HashMap::new();
        if let Some(tokens) = state["tokens"].as_array() {
            for t in tokens {
                if let (Some(name), Some(amount)) = (t["token_type"].as_str(), t["amount"].as_i64())
                {
                    balances.insert(name.to_string(), amount);
                }
            }
        }
        balances
    }

    fn tags_played(state: &Value) -> HashMap<String, u32> {
        let mut played = HashMap::new();
        if let Some(arr) = state["cards_played_per_tag_contract"].as_array() {
            for entry in arr {
                if let (Some(tag), Some(count)) = (entry["tag"].as_str(), entry["count"].as_u64()) {
                    played.insert(tag.to_string(), count as u32);
                }
            }
        }
        played
    }

    // -------------------------------------------------------------------
    // Affordability check
    // -------------------------------------------------------------------

    /// Returns true if the player currently has enough tokens to pay all
    /// input costs for the given card's effects.
    fn can_afford_card(card: &Value, token_balances: &HashMap<String, i64>) -> bool {
        let mut required: HashMap<&str, i64> = HashMap::new();
        if let Some(effects) = card["effects"].as_array() {
            for effect in effects {
                if let Some(inputs) = effect["inputs"].as_array() {
                    for input in inputs {
                        let token = input["token_type"].as_str().unwrap_or("");
                        let amount = input["amount"].as_i64().unwrap_or(0);
                        *required.entry(token).or_insert(0) += amount;
                    }
                }
            }
        }
        for (token, needed) in required {
            let available = *token_balances.get(token).unwrap_or(&0);
            if available < needed {
                return false;
            }
        }
        true
    }

    // -------------------------------------------------------------------
    // Card quality (for deckbuilding decisions)
    // -------------------------------------------------------------------

    /// Score a card for its general usefulness across any contract.
    /// Uses NET production per token (outputs minus inputs) so self-consuming
    /// cards with low net gain rank below simple producers of the same token.
    fn card_general_quality(card: &Value) -> f64 {
        let mut net: HashMap<String, f64> = HashMap::new();
        if let Some(effects) = card["effects"].as_array() {
            for effect in effects {
                if let Some(outputs) = effect["outputs"].as_array() {
                    for o in outputs {
                        let token = o["token_type"].as_str().unwrap_or("").to_string();
                        let amount = o["amount"].as_f64().unwrap_or(0.0);
                        *net.entry(token).or_insert(0.0) += amount;
                    }
                }
                if let Some(inputs) = effect["inputs"].as_array() {
                    for i in inputs {
                        let token = i["token_type"].as_str().unwrap_or("").to_string();
                        let amount = i["amount"].as_f64().unwrap_or(0.0);
                        *net.entry(token).or_insert(0.0) -= amount;
                    }
                }
            }
        }
        let mut score = 0.0;
        for (token, &n) in &net {
            match token.as_str() {
                "ProductionUnit" | "Energy" | "QualityPoint" | "Innovation" => {
                    score += if n >= 0.0 { n * 2.0 } else { n * 4.0 };
                }
                "Heat" | "Waste" | "Pollution" => {
                    // Lighter harmful penalty: high-output cards with harmful side effects
                    // are still valuable as long as the contract's max isn't violated
                    // (which is checked separately in `card_contract_score`).
                    score += -n * 1.5;
                }
                _ => {}
            }
        }
        score
    }

    /// Estimate effective production rate of `token_name` per card the strategy plays.
    /// Uses the top half of producing cards by production amount — closer to actual play
    /// behaviour than `deck_mean_production`, which dilutes high-value cards with the
    /// 50-card deck average.  Used for feasibility scoring at acceptance and during contracts.
    fn deck_effective_production(cards: &[Value], token_name: &str) -> f64 {
        let mut productions: Vec<f64> = Vec::new();
        for entry in cards {
            let in_cycle = entry["counts"]["deck"].as_f64().unwrap_or(0.0)
                + entry["counts"]["hand"].as_f64().unwrap_or(0.0)
                + entry["counts"]["discard"].as_f64().unwrap_or(0.0);
            if in_cycle <= 0.0 {
                continue;
            }
            let prod = Self::card_net_production(&entry["card"], token_name);
            if prod <= 0.0 {
                continue;
            }
            for _ in 0..(in_cycle as usize) {
                productions.push(prod);
            }
        }
        if productions.is_empty() {
            return 0.0;
        }
        productions.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let take = productions.len().div_ceil(2);
        productions.iter().take(take).sum::<f64>() / take as f64
    }

    // -------------------------------------------------------------------
    // Card play scoring (during a contract)
    // -------------------------------------------------------------------

    /// Score a card for playing during the active contract.
    /// Returns `f64::NEG_INFINITY` if playing this card would immediately fail the contract
    /// by exceeding a harmful token's `max` requirement.
    fn card_contract_score(
        card: &Value,
        contract: &Value,
        token_balances: &HashMap<String, i64>,
        tags_played: &HashMap<String, u32>,
    ) -> f64 {
        let mut score = 0.0;
        let reqs = contract["requirements"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        if let Some(effects) = card["effects"].as_array() {
            for effect in effects {
                if let Some(outputs) = effect["outputs"].as_array() {
                    for output in outputs {
                        let amount = output["amount"].as_f64().unwrap_or(0.0);
                        let token_name = output["token_type"].as_str().unwrap_or("");
                        let current = *token_balances.get(token_name).unwrap_or(&0) as f64;

                        for req in &reqs {
                            if req["requirement_type"].as_str() != Some("TokenRequirement") {
                                continue;
                            }
                            if req["token_type"].as_str() != Some(token_name) {
                                continue;
                            }

                            if let Some(max) = req["max"].as_f64() {
                                if current + amount > max {
                                    return f64::NEG_INFINITY;
                                }
                                let headroom = max - current;
                                if amount > headroom * 0.75 {
                                    score -= 50.0;
                                }
                            }

                            if let Some(min) = req["min"].as_f64() {
                                let needed = (min - current).max(0.0);
                                if needed > 0.0 {
                                    score += amount.min(needed) * 5.0;
                                }
                            }
                        }
                    }
                }

                if let Some(inputs) = effect["inputs"].as_array() {
                    for input in inputs {
                        let amount = input["amount"].as_f64().unwrap_or(0.0);
                        let token_name = input["token_type"].as_str().unwrap_or("");
                        let current = *token_balances.get(token_name).unwrap_or(&0) as f64;

                        for req in &reqs {
                            if req["requirement_type"].as_str() != Some("TokenRequirement") {
                                continue;
                            }
                            if req["token_type"].as_str() != Some(token_name) {
                                continue;
                            }

                            if let Some(max) = req["max"].as_f64() {
                                let headroom = max - current;
                                if headroom < amount * 3.0 {
                                    score += amount * 3.0;
                                }
                            }

                            if let Some(min) = req["min"].as_f64() {
                                if current <= min {
                                    score -= amount * 5.0;
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(tags) = card["tags"].as_array() {
            for tag in tags {
                let tag_name = tag.as_str().unwrap_or("");
                let played = *tags_played.get(tag_name).unwrap_or(&0) as f64;

                for req in &reqs {
                    if req["requirement_type"].as_str() != Some("CardTagConstraint") {
                        continue;
                    }
                    if req["tag"].as_str() != Some(tag_name) {
                        continue;
                    }
                    if let Some(min) = req["min"].as_f64() {
                        if played < min {
                            score += 5.0;
                        }
                    }
                }
            }
        }

        score
    }

    // -------------------------------------------------------------------
    // Contract scoring helpers
    // -------------------------------------------------------------------

    fn card_net_production(card: &Value, token_name: &str) -> f64 {
        let mut net = 0.0;
        if let Some(effects) = card["effects"].as_array() {
            for effect in effects {
                if let Some(outputs) = effect["outputs"].as_array() {
                    for o in outputs {
                        if o["token_type"].as_str() == Some(token_name) {
                            net += o["amount"].as_f64().unwrap_or(0.0);
                        }
                    }
                }
                if let Some(inputs) = effect["inputs"].as_array() {
                    for i in inputs {
                        if i["token_type"].as_str() == Some(token_name) {
                            net -= i["amount"].as_f64().unwrap_or(0.0);
                        }
                    }
                }
            }
        }
        net
    }

    /// Estimate mean net production of `token_name` per card drawn from the active cycle.
    fn deck_mean_production(cards: &[Value], token_name: &str) -> f64 {
        let mut total_prod = 0.0;
        let mut total_cards = 0.0;
        for entry in cards {
            let in_cycle = entry["counts"]["deck"].as_f64().unwrap_or(0.0)
                + entry["counts"]["hand"].as_f64().unwrap_or(0.0)
                + entry["counts"]["discard"].as_f64().unwrap_or(0.0);
            if in_cycle > 0.0 {
                let prod = Self::card_net_production(&entry["card"], token_name);
                total_prod += prod * in_cycle;
                total_cards += in_cycle;
            }
        }
        if total_cards > 0.0 {
            total_prod / total_cards
        } else {
            0.0
        }
    }

    /// Count card types in the active cycle (deck + hand + discard) that produce
    /// a net positive amount of `token_name`.
    fn deck_producing_card_count(cards: &[Value], token_name: &str) -> usize {
        cards
            .iter()
            .filter(|entry| {
                let in_cycle = entry["counts"]["deck"].as_f64().unwrap_or(0.0)
                    + entry["counts"]["hand"].as_f64().unwrap_or(0.0)
                    + entry["counts"]["discard"].as_f64().unwrap_or(0.0);
                in_cycle > 0.0 && Self::card_net_production(&entry["card"], token_name) > 0.0
            })
            .count()
    }

    /// Sum total copies of token-producing cards in the active cycle (deck + hand + discard).
    /// Unlike `deck_producing_card_count`, counts all copies, not unique types.
    fn deck_producing_copy_count(cards: &[Value], token_name: &str) -> f64 {
        cards
            .iter()
            .filter(|entry| Self::card_net_production(&entry["card"], token_name) > 0.0)
            .map(|entry| {
                entry["counts"]["deck"].as_f64().unwrap_or(0.0)
                    + entry["counts"]["hand"].as_f64().unwrap_or(0.0)
                    + entry["counts"]["discard"].as_f64().unwrap_or(0.0)
            })
            .sum()
    }

    /// Count shelved card types that produce a net positive amount of `token_name`.
    fn shelved_producing_card_count(cards: &[Value], token_name: &str) -> usize {
        cards
            .iter()
            .filter(|entry| {
                let shelved = entry["counts"]["shelved"].as_f64().unwrap_or(0.0);
                shelved > 0.0 && Self::card_net_production(&entry["card"], token_name) > 0.0
            })
            .count()
    }

    /// Return non-PU beneficial tokens that are at risk of extinction:
    ///   active-cycle producers < 2  AND  at least one shelved producer exists.
    ///
    /// PU is excluded because the 50-card starter deck always provides ample PU production.
    fn tokens_at_risk_for_sacrifice(cards: &[Value]) -> Vec<String> {
        const EXTINCTION_FLOOR: usize = 2;
        const AT_RISK_TOKENS: &[&str] = &["Energy", "QualityPoint", "Innovation"];
        let mut at_risk = Vec::new();
        for &token in AT_RISK_TOKENS {
            let shelved = Self::shelved_producing_card_count(cards, token);
            if shelved > 0 && Self::deck_producing_card_count(cards, token) < EXTINCTION_FLOOR {
                at_risk.push(token.to_string());
            }
        }
        at_risk
    }

    /// Pick the worst-quality sacrifice from `sacrifice_indices`, excluding `exclude`.
    /// Prefers cards that do not produce any at-risk token to prevent accidental extinction
    /// of rare beneficial token producers. Falls back to worst-quality card unconditionally.
    fn safe_sacrifice_index(
        cards: &[Value],
        sacrifice_indices: &[usize],
        exclude: usize,
    ) -> Option<usize> {
        let at_risk = Self::tokens_at_risk_for_sacrifice(cards);
        sacrifice_indices
            .iter()
            .copied()
            .filter(|&i| i != exclude)
            .filter(|&i| {
                at_risk
                    .iter()
                    .all(|t| Self::card_net_production(&cards[i]["card"], t) <= 0.0)
            })
            .min_by(|&a, &b| {
                Self::card_general_quality(&cards[a]["card"])
                    .partial_cmp(&Self::card_general_quality(&cards[b]["card"]))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .or_else(|| {
                sacrifice_indices
                    .iter()
                    .copied()
                    .filter(|&i| i != exclude)
                    .min_by(|&a, &b| {
                        Self::card_general_quality(&cards[a]["card"])
                            .partial_cmp(&Self::card_general_quality(&cards[b]["card"]))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            })
    }

    /// Total number of active-cycle card copies (deck + hand + discard).
    fn deck_cycle_size(cards: &[Value]) -> f64 {
        cards
            .iter()
            .map(|e| {
                e["counts"]["deck"].as_f64().unwrap_or(0.0)
                    + e["counts"]["hand"].as_f64().unwrap_or(0.0)
                    + e["counts"]["discard"].as_f64().unwrap_or(0.0)
            })
            .sum()
    }

    /// Returns true if the active contract cannot be completed with the current
    /// active cycle: either a required token has zero producers, the TurnWindow
    /// remaining turns cannot plausibly accumulate enough tokens, or a
    /// CardTagConstraint max=0 ban covers the majority of the cycle deck.
    fn is_contract_impossible(
        contract: &Value,
        cards: &[Value],
        token_balances: &HashMap<String, i64>,
        contract_turns_played: u32,
    ) -> bool {
        let reqs = match contract["requirements"].as_array() {
            Some(r) => r,
            None => return false,
        };

        // Extract TurnWindow max for remaining-turns calculation.
        let turn_window_max: Option<u32> = reqs.iter().find_map(|req| {
            if req["requirement_type"].as_str() == Some("TurnWindow") {
                req["max_turn"].as_u64().map(|m| m as u32)
            } else {
                None
            }
        });

        for req in reqs {
            if req["requirement_type"].as_str() != Some("TokenRequirement") {
                continue;
            }
            if let Some(min) = req["min"].as_f64() {
                let token = req["token_type"].as_str().unwrap_or("");
                let current = *token_balances.get(token).unwrap_or(&0) as f64;
                let remaining = min - current;
                if remaining > 0.0 {
                    if Self::deck_producing_card_count(cards, token) == 0 {
                        return true; // Can never produce this token.
                    }
                    if let Some(max_t) = turn_window_max {
                        let turns_left = max_t.saturating_sub(contract_turns_played) as f64;
                        if turns_left <= 0.0 {
                            return true; // Already at or past the deadline.
                        }
                        let mean_prod = Self::deck_effective_production(cards, token);
                        let expected = turns_left * mean_prod;
                        if expected < remaining * 0.5 {
                            // Expected production is < 50 % of what's needed: abandon.
                            return true;
                        }
                    }
                }
            }
        }

        // Check CardTagConstraint max bans: if more than half the cycle is banned,
        // the contract is effectively unplayable.
        let cycle_size = Self::deck_cycle_size(cards);
        if cycle_size > 0.0 {
            for req in reqs {
                if req["requirement_type"].as_str() != Some("CardTagConstraint") {
                    continue;
                }
                if let (Some(max), Some(tag)) = (req["max"].as_f64(), req["tag"].as_str()) {
                    let tagged = Self::deck_tag_count(cards, tag);
                    let banned = (tagged - max).max(0.0);
                    if banned / cycle_size > 0.5 {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Return beneficial token names that appear as `min` requirements in any offered contract
    /// but for which the active cycle has fewer than the minimum threshold.
    /// Unlike `tokens_needing_diversity`, does NOT require a shelved producer to exist — used
    /// to detect bootstrap bottlenecks so reward-card scoring can steer us toward those tokens.
    fn tokens_needed_for_advancement(cards: &[Value], state: &Value) -> Vec<String> {
        const MIN_COPIES: f64 = 8.0;
        let mut needed: Vec<String> = Vec::new();
        let offered = match state["offered_contracts"].as_array() {
            Some(o) => o,
            None => return needed,
        };
        for tier_group in offered {
            if let Some(contracts) = tier_group["contracts"].as_array() {
                for contract in contracts {
                    if let Some(reqs) = contract["requirements"].as_array() {
                        for req in reqs {
                            if req["requirement_type"].as_str() != Some("TokenRequirement") {
                                continue;
                            }
                            if !req["min"].is_number() {
                                continue;
                            }
                            if let Some(token) = req["token_type"].as_str() {
                                if !needed.contains(&token.to_string())
                                    && Self::deck_producing_copy_count(cards, token) < MIN_COPIES
                                {
                                    needed.push(token.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        needed
    }

    /// Return beneficial token names that appear as `min` requirements in any offered contract
    /// but for which the active cycle has fewer than `MIN_DECK_PRODUCER_COPIES` total copies
    /// AND at least one shelved producer is available to swap in.
    fn tokens_needing_diversity(cards: &[Value], state: &Value) -> Vec<String> {
        // 10 copies ≈ 20 % of the 50-card deck — enough for reliable mid-game production.
        const MIN_DECK_PRODUCER_COPIES: f64 = 10.0;
        let mut needed: Vec<String> = Vec::new();
        let offered = match state["offered_contracts"].as_array() {
            Some(o) => o,
            None => return needed,
        };
        for tier_group in offered {
            if let Some(contracts) = tier_group["contracts"].as_array() {
                for contract in contracts {
                    if let Some(reqs) = contract["requirements"].as_array() {
                        for req in reqs {
                            if req["requirement_type"].as_str() != Some("TokenRequirement") {
                                continue;
                            }
                            if !req["min"].is_number() {
                                continue;
                            }
                            if let Some(token) = req["token_type"].as_str() {
                                if !needed.contains(&token.to_string())
                                    && Self::shelved_producing_card_count(cards, token) > 0
                                    && Self::deck_producing_copy_count(cards, token)
                                        < MIN_DECK_PRODUCER_COPIES
                                {
                                    needed.push(token.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        needed
    }

    fn deck_tag_count(cards: &[Value], tag: &str) -> f64 {
        let mut count = 0.0;
        for entry in cards {
            let in_cycle = entry["counts"]["deck"].as_f64().unwrap_or(0.0)
                + entry["counts"]["hand"].as_f64().unwrap_or(0.0)
                + entry["counts"]["discard"].as_f64().unwrap_or(0.0);
            if in_cycle > 0.0 {
                let has_tag = entry["card"]["tags"]
                    .as_array()
                    .map(|ts| ts.iter().any(|t| t.as_str() == Some(tag)))
                    .unwrap_or(false);
                if has_tag {
                    count += in_cycle;
                }
            }
        }
        count
    }

    /// Score a contract for selection.
    ///
    /// Score = `tier × TIER_WEIGHT − infeasibility_cost + advancement_bonus`.
    ///
    /// Tuning rationale (issue #16): tier dominance is set strongly enough that *any*
    /// tier-N+1 contract with at least one producer beats a fully feasible tier-N
    /// contract — this prevents the strategy from farming the highest-feasible tier
    /// indefinitely (which saturates adaptive-balance pressure and stalls progression).
    /// Hopeless contracts (no producers / tag-banned majority) still lose to feasible
    /// lower-tier contracts via `ZERO_PRODUCER_PENALTY > TIER_WEIGHT`.
    ///
    /// The advancement bonus is only applied when `feasibility > 0` — prevents hard-impossible
    /// contracts from sneaking past the tier weight threshold via the bonus.
    fn score_contract(
        contract: &Value,
        cards: &[Value],
        token_balances: &HashMap<String, i64>,
        needed_tokens: &[String],
    ) -> f64 {
        const TIER_WEIGHT: f64 = 25_000.0;
        // Must exceed TIER_WEIGHT so that no-producer tier-N+1 still loses to feasible tier-N.
        const ZERO_PRODUCER_PENALTY: f64 = 30_000.0;
        // Soft penalty for partial-feasibility contracts; smaller than (TIER_WEIGHT − ZERO_PRODUCER_PENALTY)
        // gap so tier-N+1 with producers always beats tier-N regardless of feasibility shortfall.
        const SOFT_INFEASIBILITY_PENALTY: f64 = 3_000.0;
        const ADVANCEMENT_BONUS: f64 = 2_000.0;

        let tier = contract["tier"].as_f64().unwrap_or(0.0);

        let reqs = match contract["requirements"].as_array() {
            Some(r) => r,
            None => return tier * TIER_WEIGHT,
        };

        // Extract TurnWindow max (None = no hard deadline).
        let turn_window_max: Option<f64> = reqs.iter().find_map(|req| {
            if req["requirement_type"].as_str() == Some("TurnWindow") {
                req["max_turn"].as_f64()
            } else {
                None
            }
        });
        let max_turns = turn_window_max.unwrap_or(50.0);

        // Minimum feasibility across all requirements.  1.0 = fully feasible,
        // 0.0 = no producers in deck / tag-banned majority.
        let mut feasibility = 1.0f64;

        let cycle_size = Self::deck_cycle_size(cards);

        for req in reqs {
            match req["requirement_type"].as_str() {
                Some("TokenRequirement") => {
                    let token_name = req["token_type"].as_str().unwrap_or("");
                    let current = *token_balances.get(token_name).unwrap_or(&0) as f64;

                    // Already violating a harmful max → contract immediately fails.
                    if let Some(max) = req["max"].as_f64() {
                        if current > max {
                            return tier - 100_000.0;
                        }
                        // Near the harmful max: risky — reduce feasibility.
                        let mean_prod = Self::deck_effective_production(cards, token_name);
                        if mean_prod > 0.0 {
                            let headroom = max - current;
                            let turns_until_fail = headroom / mean_prod;
                            if turns_until_fail < 5.0 {
                                feasibility = feasibility.min(0.3);
                            } else if turns_until_fail < 15.0 {
                                feasibility = feasibility.min(0.7);
                            }
                        }
                    }

                    if let Some(min) = req["min"].as_f64() {
                        let needed = (min - current).max(0.0);
                        if needed > 0.0 {
                            if Self::deck_producing_card_count(cards, token_name) == 0 {
                                // No producers at all — deck cannot contribute anything.
                                feasibility = feasibility.min(0.0);
                            } else {
                                let mean_prod = Self::deck_effective_production(cards, token_name);
                                let expected_total = current + max_turns * mean_prod;
                                let token_feasibility = (expected_total / min).min(1.0);
                                feasibility = feasibility.min(token_feasibility);
                            }
                        }
                    }
                }
                Some("CardTagConstraint") => {
                    let tag = req["tag"].as_str().unwrap_or("");
                    // Min requirement: need enough tagged cards played.
                    if let Some(min) = req["min"].as_f64() {
                        let deck_count = Self::deck_tag_count(cards, tag);
                        if deck_count < min {
                            let tag_feasibility = (deck_count / min).clamp(0.0, 1.0);
                            feasibility = feasibility.min(tag_feasibility);
                        }
                    }
                    // Max requirement: banned/limited tag.  If more than half the active
                    // cycle is banned, the contract is effectively unplayable.
                    if let Some(max) = req["max"].as_f64() {
                        if cycle_size > 0.0 {
                            let tagged = Self::deck_tag_count(cards, tag);
                            let banned = (tagged - max).max(0.0);
                            let banned_fraction = banned / cycle_size;
                            if banned_fraction > 0.5 {
                                feasibility = 0.0;
                            } else if banned_fraction > 0.1 {
                                feasibility = feasibility.min(1.0 - banned_fraction);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let infeasibility_cost = if feasibility <= 0.0 {
            ZERO_PRODUCER_PENALTY
        } else {
            (1.0 - feasibility) * SOFT_INFEASIBILITY_PENALTY
        };

        // Advancement bonus only when contract is actually playable.
        let reward_card = &contract["reward_card"];
        let advancement_bonus: f64 = if feasibility > 0.0 {
            needed_tokens
                .iter()
                .map(|token| {
                    if Self::card_net_production(reward_card, token) > 0.0 {
                        ADVANCEMENT_BONUS
                    } else {
                        0.0
                    }
                })
                .fold(0.0_f64, f64::max)
        } else {
            0.0
        };

        let reward_quality = Self::card_general_quality(reward_card);
        tier * TIER_WEIGHT - infeasibility_cost + advancement_bonus + reward_quality * 0.1
    }

    // -------------------------------------------------------------------
    // Action builders
    // -------------------------------------------------------------------

    fn choose_deckbuild_action(replace_action: &Value, state: &Value) -> Option<Value> {
        // Cap candidate lists to prevent O(n²) scans during any brief transition before
        // flood control has had time to drain the shelved list to MAX_SHELVED_ENTRIES.
        const MAX_SHELVED_ENTRIES: usize = 30;
        const MAX_CANDIDATES: usize = 200;

        let cards = state["cards"].as_array()?;

        let target_indices: Vec<usize> = replace_action["valid_target_card_indices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64().map(|i| i as usize))
            .collect();

        // Collect the raw count before capping so Pass 3 can check the true shelved size.
        let replacement_indices_all: Vec<usize> = replace_action["valid_replacement_card_indices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64().map(|i| i as usize))
            .collect();
        let shelved_count = replacement_indices_all.len();
        let replacement_indices: Vec<usize> = replacement_indices_all
            .into_iter()
            .take(MAX_CANDIDATES)
            .collect();

        let sacrifice_indices: Vec<usize> = replace_action["valid_sacrifice_card_indices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64().map(|i| i as usize))
            .take(MAX_CANDIDATES)
            .collect();

        if replacement_indices.is_empty() || target_indices.is_empty() {
            return None;
        }

        // Pass 1 — Diversity forcing: if any contract-required token lacks enough active-cycle
        // producers, force-swap in the best available producer regardless of quality.
        let needed_tokens = Self::tokens_needing_diversity(cards, state);
        for token in &needed_tokens {
            let best_diversity_replacement = replacement_indices
                .iter()
                .copied()
                .filter(|&i| Self::card_net_production(&cards[i]["card"], token) > 0.0)
                .max_by(|&a, &b| {
                    let qa = Self::card_general_quality(&cards[a]["card"]);
                    let qb = Self::card_general_quality(&cards[b]["card"]);
                    qa.partial_cmp(&qb).unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some(replacement) = best_diversity_replacement {
                let worst_target = target_indices.iter().copied().min_by(|&a, &b| {
                    let qa = Self::card_general_quality(&cards[a]["card"]);
                    let qb = Self::card_general_quality(&cards[b]["card"]);
                    qa.partial_cmp(&qb).unwrap_or(std::cmp::Ordering::Equal)
                })?;

                let worst_sacrifice =
                    Self::safe_sacrifice_index(cards, &sacrifice_indices, replacement)?;

                return Some(json!({
                    "action_type": "ReplaceCard",
                    "target_card_index": worst_target,
                    "replacement_card_index": replacement,
                    "sacrifice_card_index": worst_sacrifice
                }));
            }
        }

        // Pass 3 — Flood control: when unique shelved entries exceed MAX_SHELVED_ENTRIES,
        // always execute a ReplaceCard to drain by 1 (net effect: −1 unique shelved entry
        // per action).  Chooses the best shelved card as replacement (quality gain),
        // sacrifices the worst (quality cleanup), and evicts the worst deck card as target.
        // This keeps `state["cards"].len()` bounded at ≈ cycle_size + MAX_SHELVED_ENTRIES,
        // preventing O(n²) iteration slowdown in all helper functions.
        if shelved_count > MAX_SHELVED_ENTRIES && sacrifice_indices.len() >= 2 {
            let best_replacement = replacement_indices.iter().copied().max_by(|&a, &b| {
                Self::card_general_quality(&cards[a]["card"])
                    .partial_cmp(&Self::card_general_quality(&cards[b]["card"]))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })?;
            let worst_sacrifice =
                Self::safe_sacrifice_index(cards, &sacrifice_indices, best_replacement)?;
            let worst_target = target_indices.iter().copied().min_by(|&a, &b| {
                Self::card_general_quality(&cards[a]["card"])
                    .partial_cmp(&Self::card_general_quality(&cards[b]["card"]))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })?;
            return Some(json!({
                "action_type": "ReplaceCard",
                "target_card_index": worst_target,
                "replacement_card_index": best_replacement,
                "sacrifice_card_index": worst_sacrifice
            }));
        }

        // Pass 2 — Quality upgrade: swap the best shelved card in for the worst deck card,
        // but only when the swap is strictly beneficial AND the target doesn't produce a
        // token still needed for advancement (preventing eviction of newly-bootstrapped
        // token producers before the deck has enough copies).
        let advancement_tokens = Self::tokens_needed_for_advancement(cards, state);
        let best_replacement = replacement_indices.iter().copied().max_by(|&a, &b| {
            let qa = Self::card_general_quality(&cards[a]["card"]);
            let qb = Self::card_general_quality(&cards[b]["card"]);
            qa.partial_cmp(&qb).unwrap_or(std::cmp::Ordering::Equal)
        })?;

        let worst_target = target_indices
            .iter()
            .copied()
            .filter(|&idx| {
                // Never evict a card that produces an advancement-needed token.
                advancement_tokens
                    .iter()
                    .all(|t| Self::card_net_production(&cards[idx]["card"], t) <= 0.0)
            })
            .min_by(|&a, &b| {
                let qa = Self::card_general_quality(&cards[a]["card"]);
                let qb = Self::card_general_quality(&cards[b]["card"]);
                qa.partial_cmp(&qb).unwrap_or(std::cmp::Ordering::Equal)
            });

        let worst_target = worst_target?;

        let replacement_q = Self::card_general_quality(&cards[best_replacement]["card"]);
        let target_q = Self::card_general_quality(&cards[worst_target]["card"]);

        if replacement_q <= target_q {
            return None;
        }

        let worst_sacrifice =
            Self::safe_sacrifice_index(cards, &sacrifice_indices, best_replacement)?;

        Some(json!({
            "action_type": "ReplaceCard",
            "target_card_index": worst_target,
            "replacement_card_index": best_replacement,
            "sacrifice_card_index": worst_sacrifice
        }))
    }

    fn choose_play_card(
        play_action: &Value,
        state: &Value,
        token_balances: &HashMap<String, i64>,
        tags_played: &HashMap<String, u32>,
    ) -> Option<Value> {
        let indices = play_action["valid_card_indices"].as_array()?;
        let cards = state["cards"].as_array()?;
        let active_contract = &state["active_contract"];

        let best = indices
            .iter()
            .filter_map(|v| v.as_u64().map(|i| i as usize))
            .map(|i| {
                let card = &cards[i]["card"];
                if !Self::can_afford_card(card, token_balances) {
                    return (i, f64::NEG_INFINITY);
                }
                let score = if active_contract.is_null() {
                    Self::card_general_quality(card)
                } else {
                    Self::card_contract_score(card, active_contract, token_balances, tags_played)
                };
                (i, score)
            })
            .filter(|&(_, score)| score.is_finite())
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        best.map(|(idx, _)| json!({ "action_type": "PlayCard", "card_index": idx }))
    }

    fn choose_discard_card(
        discard_action: &Value,
        state: &Value,
        token_balances: &HashMap<String, i64>,
        tags_played: &HashMap<String, u32>,
    ) -> Option<Value> {
        let indices = discard_action["valid_card_indices"].as_array()?;
        let cards = state["cards"].as_array()?;
        let active_contract = &state["active_contract"];

        let worst = indices
            .iter()
            .filter_map(|v| v.as_u64().map(|i| i as usize))
            .min_by(|&a, &b| {
                let score_a = if active_contract.is_null() {
                    Self::card_general_quality(&cards[a]["card"])
                } else {
                    Self::card_contract_score(
                        &cards[a]["card"],
                        active_contract,
                        token_balances,
                        tags_played,
                    )
                };
                let score_b = if active_contract.is_null() {
                    Self::card_general_quality(&cards[b]["card"])
                } else {
                    Self::card_contract_score(
                        &cards[b]["card"],
                        active_contract,
                        token_balances,
                        tags_played,
                    )
                };
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        worst.map(|idx| json!({ "action_type": "DiscardCard", "card_index": idx }))
    }

    fn choose_accept_contract(
        accept_action: &Value,
        state: &Value,
        token_balances: &HashMap<String, i64>,
    ) -> Option<Value> {
        let valid_tiers = accept_action["valid_tiers"].as_array()?;
        let cards = state["cards"].as_array().cloned().unwrap_or_default();
        let offered = &state["offered_contracts"];

        let needed_tokens = Self::tokens_needed_for_advancement(&cards, state);

        // Pass 1: consider only contracts that are not pre-flight-impossible.
        // Pass 2 (fallback): if nothing is feasible, accept the highest-scored contract anyway.
        for feasible_only in [true, false] {
            let mut best: Option<(usize, usize, f64)> = None;

            for tier_range in valid_tiers {
                let tier_idx = tier_range["tier_index"].as_u64().unwrap_or(0) as usize;
                let min_c = tier_range["valid_contract_index_range"]["min"]
                    .as_u64()
                    .unwrap_or(0) as usize;
                let max_c = tier_range["valid_contract_index_range"]["max"]
                    .as_u64()
                    .unwrap_or(0) as usize;

                if let Some(tier_contracts) = offered.as_array().and_then(|a| a.get(tier_idx)) {
                    if let Some(contracts) = tier_contracts["contracts"].as_array() {
                        for c_idx in min_c..=max_c {
                            if let Some(contract) = contracts.get(c_idx) {
                                if feasible_only
                                    && Self::is_contract_impossible(
                                        contract,
                                        &cards,
                                        token_balances,
                                        0,
                                    )
                                {
                                    continue;
                                }
                                let s = Self::score_contract(
                                    contract,
                                    &cards,
                                    token_balances,
                                    &needed_tokens,
                                );
                                if best.is_none() || s > best.unwrap().2 {
                                    best = Some((tier_idx, c_idx, s));
                                }
                            }
                        }
                    }
                }
            }

            if let Some((t, c, _)) = best {
                return Some(json!({
                    "action_type": "AcceptContract",
                    "tier_index": t,
                    "contract_index": c
                }));
            }
        }

        // Final fallback: take first available contract.
        let highest = valid_tiers.last()?;
        let tier_index = highest["tier_index"].as_u64().unwrap_or(0);
        let contract_index = highest["valid_contract_index_range"]["min"]
            .as_u64()
            .unwrap_or(0);
        Some(json!({
            "action_type": "AcceptContract",
            "tier_index": tier_index,
            "contract_index": contract_index
        }))
    }
}

impl Strategy for SmartStrategy {
    fn name(&self) -> &str {
        "smart"
    }

    fn needs_state(&self) -> bool {
        true
    }

    fn choose_action(&self, possible_actions: &[Value], snapshot: &GameSnapshot) -> Value {
        let state = &snapshot.state;
        let token_balances = Self::token_balances(state);
        let tags_played = Self::tags_played(state);

        // 0. Detect and abandon active contracts that the current deck cannot complete.
        //    If AbandonContract is not yet available (< min_turns_before_abandon turns played),
        //    skip PlayCard and fall through to DiscardCard to accumulate the required turns.
        let impossible_contract = state["cards"].as_array().is_some_and(|cards| {
            !state["active_contract"].is_null() && {
                let turns_played = state["contract_turns_played"].as_u64().unwrap_or(0) as u32;
                Self::is_contract_impossible(
                    &state["active_contract"],
                    cards,
                    &token_balances,
                    turns_played,
                )
            }
        });
        if impossible_contract {
            if possible_actions
                .iter()
                .any(|a| a["action_type"] == "AbandonContract")
            {
                self.consecutive_discards.set(0);
                return json!({ "action_type": "AbandonContract" });
            }
            // Not yet abandonable: discard to accumulate turns, then abandon.
            if let Some(discard) = possible_actions
                .iter()
                .find(|a| a["action_type"] == "DiscardCard")
            {
                if let Some(action) =
                    Self::choose_discard_card(discard, state, &token_balances, &tags_played)
                {
                    self.consecutive_discards
                        .set(self.consecutive_discards.get() + 1);
                    return action;
                }
            }
        }

        // 1. Deckbuild when available and beneficial.
        if let Some(replace) = possible_actions
            .iter()
            .find(|a| a["action_type"] == "ReplaceCard")
        {
            if let Some(action) = Self::choose_deckbuild_action(replace, state) {
                return action;
            }
        }

        // 2. Play the best-scoring card for the active contract.
        if let Some(play) = possible_actions
            .iter()
            .find(|a| a["action_type"] == "PlayCard")
        {
            if let Some(action) = Self::choose_play_card(play, state, &token_balances, &tags_played)
            {
                self.consecutive_discards.set(0);
                return action;
            }
        }

        // 3. Accept the highest-scoring contract.
        if let Some(accept) = possible_actions
            .iter()
            .find(|a| a["action_type"] == "AcceptContract")
        {
            if let Some(action) = Self::choose_accept_contract(accept, state, &token_balances) {
                self.consecutive_discards.set(0);
                return action;
            }
        }

        // 4. Discard the least useful card, but abandon after DISCARD_STUCK_THRESHOLD
        //    consecutive discards with no PlayCard progress.
        if self.consecutive_discards.get() < DISCARD_STUCK_THRESHOLD {
            if let Some(discard) = possible_actions
                .iter()
                .find(|a| a["action_type"] == "DiscardCard")
            {
                if let Some(action) =
                    Self::choose_discard_card(discard, state, &token_balances, &tags_played)
                {
                    self.consecutive_discards
                        .set(self.consecutive_discards.get() + 1);
                    return action;
                }
            }
        }

        // 5. Abandon — either as last resort or when stuck discarding without progress.
        if possible_actions
            .iter()
            .any(|a| a["action_type"] == "AbandonContract")
        {
            self.consecutive_discards.set(0);
            return json!({ "action_type": "AbandonContract" });
        }

        panic!(
            "SmartStrategy: no actionable option found in {:?}",
            possible_actions
        );
    }
}
