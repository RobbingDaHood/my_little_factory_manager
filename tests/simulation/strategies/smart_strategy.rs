use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use my_little_factory_manager::action_log::PlayerAction;
use my_little_factory_manager::game_state::{GameStateView, PossibleAction};
use my_little_factory_manager::types::{
    CardEntry, CardTag, Contract, ContractRequirementKind, PlayerActionCard, TierContracts,
    TokenType,
};

use crate::game_driver::GameSnapshot;
use crate::strategies::Strategy;

const DISCARD_STUCK_THRESHOLD: u32 = 50;
const NO_RESOLUTION_STUCK_THRESHOLD: u32 = 1_500;
const TOP_N: usize = 30;
const BOTTOM_N: usize = 30;
const PASS1_CANDIDATES: usize = 200;
const SAFETY_MARGIN: f64 = 0.7;

/// Mid-contract abandonment threshold in turns.
/// Contracts running longer than this without sufficient progress are voluntarily abandoned.
/// Loosened from 40 → 60 to reduce abandonment churn — the previous setting fired
/// against contracts that would naturally complete in 50–80 turns.
const SLOW_PROGRESS_TURN_LIMIT: u32 = 60;
/// Minimum fractional progress required by SLOW_PROGRESS_TURN_LIMIT to keep going.
/// Lowered 0.5 → 0.3 so contracts only abandon when meaningfully stalled, not just slow.
const SLOW_PROGRESS_MIN_FRACTION: f64 = 0.3;

/// Contract-aware strategy with active deckbuilding.
///
/// Uses game state alongside possible actions to:
/// 1. Deckbuild between contracts — replacing weak deck cards with strong shelved reward cards.
/// 2. Play cards that progress toward the active contract's requirements.
/// 3. Avoid playing cards that would push harmful tokens past their contract limits.
/// 4. Choose the contract with the best reward card and most feasible requirements.
///
/// Action priority:
///   ReplaceCard (beneficial swap) → PlayCard (best score) → AcceptContract (best score)
///   → DiscardCard (worst score) → AbandonContract (last resort, or after stuck detection)
///
/// ## Adaptive adjustments
///
/// `Contract::requirements[].min/max` are **post-adjustment** values — the adaptive balance
/// overlay mutates them in place before the contract is stored. This means all feasibility
/// checks in `is_contract_impossible` and `score_contract` already operate on the real
/// (tighter) bounds without any extra work.
///
/// Additionally, `score_contract` reads `Contract::adaptive_adjustments` to apply a
/// tightening penalty: each adjusted requirement contributes a penalty proportional to
/// `abs(adjustment_percent)`, scaled by `TIGHTENING_PENALTY_PER_PCT`. This steers the
/// strategy away from contracts whose requirements were heavily tightened by the adaptive
/// system (indicating those token dimensions are under pressure), and toward contracts
/// in areas the player has under-used — naturally relaxing adaptive pressure over time.
pub struct SmartStrategy {
    consecutive_discards: Cell<u32>,
    // top-30 per tag sorted desc by quality; updated incrementally on new shelf arrivals
    best_per_tag: RefCell<HashMap<CardTag, Vec<(u64, f64)>>>,
    // bottom-30 per tag sorted asc by quality; filled lazily when a tag's list runs dry
    worst_per_tag: RefCell<HashMap<CardTag, Vec<(u64, f64)>>>,
    // content hashes of known shelved cards; used to detect new arrivals each deckbuild call
    known_shelved: RefCell<HashSet<u64>>,
    // tracks actions taken since the last contract resolution; used to detect livelock
    actions_since_last_resolution: Cell<u32>,
    // hash signature of the current active contract; used to detect contract changes
    last_active_contract_signature: RefCell<Option<u64>>,
}

impl SmartStrategy {
    pub fn new() -> Self {
        Self {
            consecutive_discards: Cell::new(0),
            best_per_tag: RefCell::new(HashMap::new()),
            worst_per_tag: RefCell::new(HashMap::new()),
            known_shelved: RefCell::new(HashSet::new()),
            actions_since_last_resolution: Cell::new(0),
            last_active_contract_signature: RefCell::new(None),
        }
    }

    // State helpers

    fn token_balances(state: &GameStateView) -> HashMap<TokenType, i64> {
        state
            .tokens
            .iter()
            .map(|t| (t.token_type.clone(), t.amount as i64))
            .collect()
    }

    fn tags_played(state: &GameStateView) -> HashMap<CardTag, u32> {
        state
            .cards_played_per_tag_contract
            .iter()
            .map(|e| (e.tag.clone(), e.count))
            .collect()
    }

    // Affordability check

    fn can_afford_card(card: &PlayerActionCard, token_balances: &HashMap<TokenType, i64>) -> bool {
        let mut required: HashMap<&TokenType, i64> = HashMap::new();
        for effect in &card.effects {
            for input in &effect.inputs {
                *required.entry(&input.token_type).or_insert(0) += input.amount as i64;
            }
        }
        for (token_type, needed) in required {
            if *token_balances.get(token_type).unwrap_or(&0) < needed {
                return false;
            }
        }
        true
    }

    // Card quality

    fn card_general_quality(card: &PlayerActionCard) -> f64 {
        let mut net: HashMap<&TokenType, f64> = HashMap::new();
        for effect in &card.effects {
            for o in &effect.outputs {
                *net.entry(&o.token_type).or_insert(0.0) += o.amount as f64;
            }
            for i in &effect.inputs {
                *net.entry(&i.token_type).or_insert(0.0) -= i.amount as f64;
            }
        }
        let mut score = 0.0;
        for (token_type, &n) in &net {
            match token_type {
                TokenType::ProductionUnit
                | TokenType::Energy
                | TokenType::QualityPoint
                | TokenType::Innovation => {
                    score += if n >= 0.0 { n * 2.0 } else { n * 4.0 };
                }
                TokenType::Heat | TokenType::Waste | TokenType::Pollution => {
                    score += -n * 1.5;
                }
                _ => {}
            }
        }
        score
    }

    fn card_net_production(card: &PlayerActionCard, token_type: &TokenType) -> f64 {
        let mut net = 0.0;
        for effect in &card.effects {
            for o in &effect.outputs {
                if &o.token_type == token_type {
                    net += o.amount as f64;
                }
            }
            for i in &effect.inputs {
                if &i.token_type == token_type {
                    net -= i.amount as f64;
                }
            }
        }
        net
    }

    // Deck analysis helpers

    fn deck_effective_production(cards: &[CardEntry], token_type: &TokenType) -> f64 {
        let mut productions: Vec<f64> = Vec::new();
        for entry in cards {
            let in_cycle = entry.counts.non_shelved() as f64;
            if in_cycle <= 0.0 {
                continue;
            }
            let prod = Self::card_net_production(&entry.card, token_type);
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

    fn by_quality(cards: &[CardEntry], a: usize, b: usize) -> std::cmp::Ordering {
        Self::card_general_quality(&cards[a].card)
            .partial_cmp(&Self::card_general_quality(&cards[b].card))
            .unwrap_or(std::cmp::Ordering::Equal)
    }

    fn deck_producing_card_count(cards: &[CardEntry], token_type: &TokenType) -> usize {
        cards
            .iter()
            .filter(|e| {
                e.counts.has_non_shelved() && Self::card_net_production(&e.card, token_type) > 0.0
            })
            .count()
    }

    fn deck_producing_copy_count(cards: &[CardEntry], token_type: &TokenType) -> f64 {
        cards
            .iter()
            .filter(|e| Self::card_net_production(&e.card, token_type) > 0.0)
            .map(|e| e.counts.non_shelved() as f64)
            .sum()
    }

    fn shelved_producing_card_count(cards: &[CardEntry], token_type: &TokenType) -> usize {
        cards
            .iter()
            .filter(|e| {
                e.counts.has_shelved() && Self::card_net_production(&e.card, token_type) > 0.0
            })
            .count()
    }
    fn deck_tag_count(cards: &[CardEntry], tag: &CardTag) -> f64 {
        cards
            .iter()
            .filter(|e| e.counts.has_non_shelved() && e.card.tags.contains(tag))
            .map(|e| e.counts.non_shelved() as f64)
            .sum()
    }
    fn deck_cycle_size(cards: &[CardEntry]) -> f64 {
        cards.iter().map(|e| e.counts.non_shelved() as f64).sum()
    }

    fn expected_turns_to_draw_producer(cards: &[CardEntry], token_type: &TokenType) -> f64 {
        let cycle_size = Self::deck_cycle_size(cards);
        if cycle_size <= 0.0 {
            return f64::INFINITY;
        }
        let producer_copies = Self::deck_producing_copy_count(cards, token_type);
        if producer_copies <= 0.0 {
            return f64::INFINITY;
        }
        cycle_size / producer_copies
    }

    fn card_hash(card: &PlayerActionCard) -> u64 {
        let mut h = DefaultHasher::new();
        card.tags.len().hash(&mut h);
        for tag in &card.tags {
            tag.hash(&mut h);
        }
        card.effects.len().hash(&mut h);
        for effect in &card.effects {
            effect.inputs.len().hash(&mut h);
            for inp in &effect.inputs {
                inp.token_type.hash(&mut h);
                inp.amount.hash(&mut h);
            }
            effect.outputs.len().hash(&mut h);
            for out in &effect.outputs {
                out.token_type.hash(&mut h);
                out.amount.hash(&mut h);
            }
        }
        h.finish()
    }

    fn contract_signature(contract: &Contract) -> u64 {
        let mut h = DefaultHasher::new();
        contract.tier.0.hash(&mut h);
        contract.requirements.len().hash(&mut h);
        for req in &contract.requirements {
            format!("{:?}", req).hash(&mut h);
        }
        h.finish()
    }

    fn process_new_arrivals(
        &self,
        cards: &[CardEntry],
        replacement_indices_raw: &[usize],
    ) -> HashMap<u64, usize> {
        let current: HashMap<u64, usize> = replacement_indices_raw
            .iter()
            .map(|&i| (Self::card_hash(&cards[i].card), i))
            .collect();

        let mut known = self.known_shelved.borrow_mut();
        let mut best = self.best_per_tag.borrow_mut();

        for (&hash, &idx) in &current {
            if !known.contains(&hash) {
                known.insert(hash);
                let quality = Self::card_general_quality(&cards[idx].card);
                for tag in &cards[idx].card.tags {
                    let entries = best.entry(tag.clone()).or_default();
                    let worst_q = entries.last().map(|(_, q)| *q).unwrap_or(f64::NEG_INFINITY);
                    if entries.len() < TOP_N || quality > worst_q {
                        let pos = entries.partition_point(|(_, q)| *q >= quality);
                        entries.insert(pos, (hash, quality));
                        if entries.len() > TOP_N {
                            entries.pop();
                        }
                    }
                }
            }
        }

        known.retain(|name| current.contains_key(name));
        current
    }

    fn refill_worst_for_tag(
        &self,
        tag: &CardTag,
        cards: &[CardEntry],
        replacement_indices_raw: &[usize],
    ) {
        let mut candidates: Vec<(u64, f64)> = replacement_indices_raw
            .iter()
            .copied()
            .filter(|&i| cards[i].card.tags.contains(tag))
            .map(|i| {
                (
                    Self::card_hash(&cards[i].card),
                    Self::card_general_quality(&cards[i].card),
                )
            })
            .collect();
        candidates.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(BOTTOM_N);
        self.worst_per_tag
            .borrow_mut()
            .insert(tag.clone(), candidates);
    }

    fn worst_sacrifice_for_tag(
        &self,
        tag: &CardTag,
        cards: &[CardEntry],
        replacement_indices_raw: &[usize],
        hash_to_index: &HashMap<u64, usize>,
        exclude: usize,
        sacrifice_indices: &[usize],
    ) -> Option<usize> {
        let mut refilled = false;
        loop {
            let need_refill = {
                let mut worst = self.worst_per_tag.borrow_mut();
                let entries = worst.entry(tag.clone()).or_default();
                while entries
                    .first()
                    .map(|(h, _)| !hash_to_index.contains_key(h))
                    .unwrap_or(false)
                {
                    entries.remove(0);
                }
                if entries.is_empty() {
                    true
                } else {
                    let found = entries.iter().find_map(|(hash, _)| {
                        hash_to_index
                            .get(hash)
                            .copied()
                            .filter(|&i| i != exclude && sacrifice_indices.contains(&i))
                    });
                    return found;
                }
            };
            if need_refill {
                if refilled {
                    return None;
                }
                self.refill_worst_for_tag(tag, cards, replacement_indices_raw);
                refilled = true;
            }
        }
    }

    // Risk helpers

    fn tokens_at_risk_for_sacrifice(cards: &[CardEntry]) -> Vec<TokenType> {
        const EXTINCTION_FLOOR: usize = 2;
        let at_risk_tokens = [
            TokenType::Energy,
            TokenType::QualityPoint,
            TokenType::Innovation,
        ];
        let mut at_risk = Vec::new();
        for token in &at_risk_tokens {
            if Self::shelved_producing_card_count(cards, token) > 0
                && Self::deck_producing_card_count(cards, token) < EXTINCTION_FLOOR
            {
                at_risk.push(token.clone());
            }
        }
        at_risk
    }

    fn safe_sacrifice_index(
        &self,
        cards: &[CardEntry],
        sacrifice_indices: &[usize],
        exclude: usize,
        hash_to_index: &HashMap<u64, usize>,
        replacement_indices_raw: &[usize],
    ) -> Option<usize> {
        let at_risk = Self::tokens_at_risk_for_sacrifice(cards);
        let known_tags: Vec<CardTag> = self.best_per_tag.borrow().keys().cloned().collect();

        // Phase 1: per-tag worst index — avoids scanning all sacrifice_indices
        let via_tags = known_tags
            .iter()
            .filter_map(|tag| {
                self.worst_sacrifice_for_tag(
                    tag,
                    cards,
                    replacement_indices_raw,
                    hash_to_index,
                    exclude,
                    sacrifice_indices,
                )
            })
            .filter(|&i| {
                at_risk
                    .iter()
                    .all(|t| Self::card_net_production(&cards[i].card, t) <= 0.0)
            })
            .min_by(|&a, &b| Self::by_quality(cards, a, b));
        if via_tags.is_some() {
            return via_tags;
        }

        // Phase 2: relax at_risk filter via tag index
        let via_tags_any = known_tags
            .iter()
            .filter_map(|tag| {
                self.worst_sacrifice_for_tag(
                    tag,
                    cards,
                    replacement_indices_raw,
                    hash_to_index,
                    exclude,
                    sacrifice_indices,
                )
            })
            .min_by(|&a, &b| Self::by_quality(cards, a, b));
        if via_tags_any.is_some() {
            return via_tags_any;
        }

        // Phase 3: full fallback scan (tagless cards or empty index)
        sacrifice_indices
            .iter()
            .copied()
            .filter(|&i| i != exclude)
            .filter(|&i| {
                at_risk
                    .iter()
                    .all(|t| Self::card_net_production(&cards[i].card, t) <= 0.0)
            })
            .min_by(|&a, &b| Self::by_quality(cards, a, b))
            .or_else(|| {
                sacrifice_indices
                    .iter()
                    .copied()
                    .filter(|&i| i != exclude)
                    .min_by(|&a, &b| Self::by_quality(cards, a, b))
            })
    }

    // Contract feasibility helpers

    fn is_contract_impossible(
        contract: &Contract,
        cards: &[CardEntry],
        token_balances: &HashMap<TokenType, i64>,
        contract_turns_played: u32,
    ) -> bool {
        let turn_window_max: Option<u32> = contract.requirements.iter().find_map(|req| {
            if let ContractRequirementKind::TurnWindow { max_turn, .. } = req {
                *max_turn
            } else {
                None
            }
        });

        for req in &contract.requirements {
            if let ContractRequirementKind::TokenRequirement {
                token_type,
                min: Some(min),
                ..
            } = req
            {
                let current = *token_balances.get(token_type).unwrap_or(&0) as f64;
                let remaining = *min as f64 - current;
                if remaining > 0.0 {
                    if Self::deck_producing_card_count(cards, token_type) == 0 {
                        return true;
                    }
                    if let Some(max_t) = turn_window_max {
                        let turns_left = max_t.saturating_sub(contract_turns_played) as f64;
                        if turns_left <= 0.0 {
                            return true;
                        }
                        let mean_prod = Self::deck_effective_production(cards, token_type);
                        if turns_left * mean_prod < remaining * SAFETY_MARGIN {
                            return true;
                        }
                    }
                }
            }
        }

        // Harmful-overflow trajectory check: if the deck's mean production rate for a
        // token with a max constraint will hit that ceiling before the deck can meet any
        // unmet min requirement, the contract is unwinnable.
        for harmful_req in &contract.requirements {
            if let ContractRequirementKind::TokenRequirement {
                token_type: harmful_token,
                max: Some(max_val),
                ..
            } = harmful_req
            {
                let current_harmful = *token_balances.get(harmful_token).unwrap_or(&0) as f64;
                let headroom = *max_val as f64 - current_harmful;
                if headroom <= 0.0 {
                    return true;
                }
                let mean_harmful_prod = Self::deck_effective_production(cards, harmful_token);
                if mean_harmful_prod <= 0.0 {
                    continue;
                }
                let turns_to_overflow = headroom / mean_harmful_prod;
                for min_req in &contract.requirements {
                    if let ContractRequirementKind::TokenRequirement {
                        token_type: beneficial_token,
                        min: Some(min_val),
                        ..
                    } = min_req
                    {
                        let current_beneficial =
                            *token_balances.get(beneficial_token).unwrap_or(&0) as f64;
                        let remaining = *min_val as f64 - current_beneficial;
                        if remaining <= 0.0 {
                            continue;
                        }
                        let mean_beneficial_prod =
                            Self::deck_effective_production(cards, beneficial_token);
                        if mean_beneficial_prod <= 0.0 {
                            continue;
                        }
                        let turns_to_complete = remaining / mean_beneficial_prod;
                        if turns_to_overflow < turns_to_complete * SAFETY_MARGIN {
                            return true;
                        }
                    }
                }
            }
        }

        let cycle_size = Self::deck_cycle_size(cards);
        if cycle_size > 0.0 {
            for req in &contract.requirements {
                if let ContractRequirementKind::CardTagConstraint {
                    tag,
                    max: Some(max),
                    ..
                } = req
                {
                    let tagged = Self::deck_tag_count(cards, tag);
                    let banned = (tagged - *max as f64).max(0.0);
                    if banned / cycle_size > 0.5 {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn is_progress_too_slow(
        contract: &Contract,
        _cards: &[CardEntry],
        token_balances: &HashMap<TokenType, i64>,
        contract_turns_played: u32,
    ) -> bool {
        if contract_turns_played < SLOW_PROGRESS_TURN_LIMIT {
            return false;
        }
        // Compute the worst-progressed beneficial min requirement.
        let mut worst: Option<f64> = None;
        for req in &contract.requirements {
            if let ContractRequirementKind::TokenRequirement {
                token_type,
                min: Some(min_val),
                ..
            } = req
            {
                let current = *token_balances.get(token_type).unwrap_or(&0) as f64;
                let target = *min_val as f64;
                if target <= 0.0 {
                    continue;
                }
                let frac = (current / target).min(1.0);
                worst = Some(match worst {
                    Some(w) => w.min(frac),
                    None => frac,
                });
            }
        }
        matches!(worst, Some(f) if f < SLOW_PROGRESS_MIN_FRACTION)
    }

    /// Estimate the highest tier where the current deck can win contracts comfortably.
    ///
    /// Each beneficial token contributes to comfort proportionally to how well it is
    /// covered by the deck (or available on the shelf for installation). Full coverage
    /// (≥ MIN_PRODUCERS distinct cards in the deck with ≥ MIN_COPIES total copies) gives
    /// the token's full bracket; a single producer (shelved or in deck) gives a partial
    /// credit so the strategy can stretch toward tiers whose key cards are already on
    /// the shelf waiting to be installed.
    ///
    /// Beneficial token introduction tiers (per `src/types.rs::TokenType` doc):
    ///   ProductionUnit:0, Energy:4, QualityPoint:16, Innovation:36
    fn comfort_tier(cards: &[CardEntry]) -> u32 {
        const MIN_PRODUCERS: usize = 2;
        const MIN_COPIES: f64 = 4.0;

        let beneficial_tiers: [(TokenType, u32); 4] = [
            (TokenType::ProductionUnit, 0),
            (TokenType::Energy, 4),
            (TokenType::QualityPoint, 16),
            (TokenType::Innovation, 36),
        ];

        let mut comfort = 0u32;
        for (token, unlock_tier) in &beneficial_tiers {
            let producers = Self::deck_producing_card_count(cards, token);
            let copies = Self::deck_producing_copy_count(cards, token);
            let any_producer_owned = cards
                .iter()
                .any(|e| Self::card_net_production(&e.card, token) > 0.0);

            if producers >= MIN_PRODUCERS && copies >= MIN_COPIES {
                comfort = comfort.max(*unlock_tier + 4);
            } else if any_producer_owned {
                // Soft credit: at least one producer somewhere (deck or shelf). Lets the
                // strategy stretch one bracket above its current strict comfort instead
                // of refusing to engage entirely with a tier whose tokens it has begun
                // to acquire.
                comfort = comfort.max(*unlock_tier + 1);
                break;
            } else {
                // No producer at all for this bracket — every higher bracket is also out
                // of reach, so stop.
                break;
            }
        }
        comfort
    }

    fn highest_offered_tier(offered: &[TierContracts]) -> u32 {
        offered.iter().map(|tg| tg.tier.0).max().unwrap_or(0)
    }

    fn beneficial_tokens_unlocked_by(target_tier: u32) -> Vec<TokenType> {
        let mut tokens = vec![TokenType::ProductionUnit];
        if target_tier >= 4 {
            tokens.push(TokenType::Energy);
        }
        if target_tier >= 16 {
            tokens.push(TokenType::QualityPoint);
        }
        if target_tier >= 36 {
            tokens.push(TokenType::Innovation);
        }
        tokens
    }

    fn tokens_needed_for_advancement(
        cards: &[CardEntry],
        offered: &[TierContracts],
    ) -> Vec<TokenType> {
        const MIN_COPIES: f64 = 8.0;
        let mut needed = Vec::new();
        for req in offered
            .iter()
            .flat_map(|tg| tg.contracts.iter())
            .flat_map(|c| c.requirements.iter())
        {
            if let ContractRequirementKind::TokenRequirement {
                token_type,
                min: Some(_),
                ..
            } = req
            {
                if !needed.contains(token_type)
                    && Self::deck_producing_copy_count(cards, token_type) < MIN_COPIES
                {
                    needed.push(token_type.clone());
                }
            }
        }

        // Look-ahead for tokens unlocking at tier+2
        let highest_tier = Self::highest_offered_tier(offered);
        let lookahead_tokens = Self::beneficial_tokens_unlocked_by(highest_tier + 2);
        for token_type in lookahead_tokens {
            if !needed.contains(&token_type)
                && Self::deck_producing_copy_count(cards, &token_type) < MIN_COPIES
            {
                needed.push(token_type);
            }
        }

        needed
    }

    fn tokens_needing_diversity(cards: &[CardEntry], offered: &[TierContracts]) -> Vec<TokenType> {
        const MIN: f64 = 10.0;
        let mut needed = Vec::new();
        for req in offered
            .iter()
            .flat_map(|tg| tg.contracts.iter())
            .flat_map(|c| c.requirements.iter())
        {
            if let ContractRequirementKind::TokenRequirement {
                token_type,
                min: Some(_),
                ..
            } = req
            {
                if !needed.contains(token_type)
                    && Self::shelved_producing_card_count(cards, token_type) > 0
                    && Self::deck_producing_copy_count(cards, token_type) < MIN
                {
                    needed.push(token_type.clone());
                }
            }
        }

        // Look-ahead for tokens unlocking at tier+2
        let highest_tier = Self::highest_offered_tier(offered);
        let lookahead_tokens = Self::beneficial_tokens_unlocked_by(highest_tier + 2);
        for token_type in lookahead_tokens {
            if !needed.contains(&token_type)
                && Self::shelved_producing_card_count(cards, &token_type) > 0
                && Self::deck_producing_copy_count(cards, &token_type) < MIN
            {
                needed.push(token_type);
            }
        }

        needed
    }

    // Contract scoring

    fn tag_diversity_bonus(reward_card: &PlayerActionCard, cards: &[CardEntry]) -> f64 {
        let mut bonus = 0.0;
        let cycle_size = Self::deck_cycle_size(cards);
        if cycle_size <= 0.0 {
            // Bonus is modest if deck is empty
            return 10.0;
        }

        for tag in &reward_card.tags {
            let tag_count = Self::deck_tag_count(cards, tag);
            // Normalize: if less than 5% of deck, full bonus; if more than 15%, no bonus
            let saturation = tag_count / (cycle_size * 0.15);
            let tag_bonus = 5.0 * (1.0 - saturation.min(1.0));
            bonus += tag_bonus;
        }

        bonus
    }

    fn token_diversity_bonus(reward_card: &PlayerActionCard, cards: &[CardEntry]) -> f64 {
        let mut bonus = 0.0;

        // Find all tokens produced by the reward card
        for effect in &reward_card.effects {
            for output in &effect.outputs {
                let token_type = &output.token_type;
                let reward_prod = Self::card_net_production(reward_card, token_type);
                if reward_prod <= 0.0 {
                    continue;
                }

                // Check how well the token is covered by existing deck
                let deck_prod = Self::deck_effective_production(cards, token_type);
                let deck_card_count = Self::deck_producing_card_count(cards, token_type);

                // Bonus is high when:
                // - Token has zero producers (deficit)
                // - Token has very few producers (scarcity)
                // - Token production is low relative to needs
                let token_bonus = if deck_card_count == 0 {
                    30.0 // Bonus for missing token types
                } else if deck_card_count == 1 {
                    15.0 // Bonus for single producer
                } else if deck_prod <= reward_prod {
                    8.0 // Bonus if deck production is low
                } else {
                    3.0 // Minimal bonus if well-covered
                };

                bonus += token_bonus;
            }
        }

        bonus
    }

    fn score_contract(
        contract: &Contract,
        cards: &[CardEntry],
        token_balances: &HashMap<TokenType, i64>,
        needed_tokens: &[TokenType],
        target_deficit: &HashMap<TokenType, u32>,
    ) -> f64 {
        const TIER_WEIGHT: f64 = 5_000.0;
        const ZERO_PRODUCER_PENALTY: f64 = 30_000.0;
        const SOFT_INFEASIBILITY_PENALTY: f64 = 3_000.0;
        const ADVANCEMENT_BONUS: f64 = 8_000.0;
        const BASE_REWARD_WEIGHT: f64 = 0.3;
        const DIVERSITY_WEIGHT: f64 = 0.5;
        // Each 1% of adaptive tightening costs this many score points.
        // At max 30% tightening on a single requirement that's -6000; multiple adjustments compound.
        const TIGHTENING_PENALTY_PER_PCT: f64 = 200.0;
        // Penalty per tier the contract is above (comfort_tier + 1).
        // Lowered 8000 → 4000 so the strategy will stretch toward higher-tier reward
        // cards (notably QualityPoint and Innovation producers) instead of grinding
        // its current comfort tier indefinitely.
        const OVERREACH_PENALTY_PER_TIER: f64 = 4_000.0;
        // Per-token bonus for a contract that produces a token currently needed
        // by the higher-tier "target" contract on the menu.
        const STOCKPILE_BONUS_PER_TOKEN: f64 = 6_000.0;

        let tier = contract.tier.0 as f64;
        if contract.requirements.is_empty() {
            return tier * TIER_WEIGHT;
        }

        let max_turns = contract
            .requirements
            .iter()
            .find_map(|req| {
                if let ContractRequirementKind::TurnWindow { max_turn, .. } = req {
                    max_turn.map(|m| m as f64)
                } else {
                    None
                }
            })
            .unwrap_or(50.0);

        let mut feasibility = 1.0f64;
        let cycle_size = Self::deck_cycle_size(cards);

        for req in &contract.requirements {
            match req {
                ContractRequirementKind::TokenRequirement {
                    token_type,
                    min,
                    max,
                } => {
                    let current = *token_balances.get(token_type).unwrap_or(&0) as f64;
                    if let Some(max_val) = max {
                        let max_f = *max_val as f64;
                        if current > max_f {
                            return tier - 100_000.0;
                        }
                        let mean_prod = Self::deck_effective_production(cards, token_type);
                        if mean_prod > 0.0 {
                            let turns_until_fail = (max_f - current) / mean_prod;
                            if turns_until_fail < 5.0 {
                                feasibility = feasibility.min(0.3);
                            } else if turns_until_fail < 15.0 {
                                feasibility = feasibility.min(0.7);
                            }
                        }
                    }
                    if let Some(min_val) = min {
                        let min_f = *min_val as f64;
                        let needed = (min_f - current).max(0.0);
                        if needed > 0.0 {
                            if Self::deck_producing_card_count(cards, token_type) == 0 {
                                feasibility = feasibility.min(0.0);
                            } else {
                                let mean_prod = Self::deck_effective_production(cards, token_type);
                                let expected = current + max_turns * mean_prod;
                                feasibility = feasibility.min((expected / min_f).min(1.0));
                            }
                        }
                    }
                }
                ContractRequirementKind::CardTagConstraint { tag, min, max } => {
                    if let Some(min_val) = min {
                        let min_f = *min_val as f64;
                        let deck_count = Self::deck_tag_count(cards, tag);
                        if deck_count < min_f {
                            feasibility = feasibility.min((deck_count / min_f).clamp(0.0, 1.0));
                        }
                    }
                    if let Some(max_val) = max {
                        if cycle_size > 0.0 {
                            let tagged = Self::deck_tag_count(cards, tag);
                            let banned_fraction = (tagged - *max_val as f64).max(0.0) / cycle_size;
                            if banned_fraction > 0.5 {
                                feasibility = 0.0;
                            } else if banned_fraction > 0.1 {
                                feasibility = feasibility.min(1.0 - banned_fraction);
                            }
                        }
                    }
                }
                ContractRequirementKind::TurnWindow { max_turn, .. } => {
                    if let Some(max_turn_val) = max_turn {
                        let max_turn_f = *max_turn_val as f64;
                        let mut tightest_ratio = 0.0f64;

                        for req in &contract.requirements {
                            if let ContractRequirementKind::TokenRequirement {
                                token_type,
                                min: Some(min_val),
                                ..
                            } = req
                            {
                                let current = *token_balances.get(token_type).unwrap_or(&0) as f64;
                                let needed = (*min_val as f64 - current).max(0.0);
                                if needed > 0.0 {
                                    // Optimistic estimate: use deck effective production
                                    let mean_prod =
                                        Self::deck_effective_production(cards, token_type);
                                    if mean_prod > 0.0 {
                                        // Account for time needed to draw producer cards into hand
                                        let draw_turns = Self::expected_turns_to_draw_producer(
                                            cards, token_type,
                                        );
                                        // Combined estimate: account for both production rate and draw time
                                        let effective_turns =
                                            draw_turns + (needed / mean_prod).max(0.0);
                                        let ratio = effective_turns / max_turn_f;
                                        tightest_ratio = tightest_ratio.max(ratio);
                                    }
                                }
                            }
                        }

                        if tightest_ratio > 0.5 {
                            let excess = (tightest_ratio - 0.5).min(1.5);
                            feasibility = feasibility.min((1.0 - excess * 0.7).max(0.0));
                        }
                    }
                }
            }
        }

        let infeasibility_cost = if feasibility <= 0.0 {
            ZERO_PRODUCER_PENALTY
        } else {
            (1.0 - feasibility).powi(2) * SOFT_INFEASIBILITY_PENALTY * 4.0
        };

        let advancement_bonus: f64 = if feasibility > 0.0 {
            needed_tokens
                .iter()
                .map(|t| {
                    if Self::card_net_production(&contract.reward_card, t) > 0.0 {
                        ADVANCEMENT_BONUS
                    } else {
                        0.0
                    }
                })
                .fold(0.0_f64, f64::max)
        } else {
            0.0
        };

        let base_quality = Self::card_general_quality(&contract.reward_card);
        let tag_bonus = Self::tag_diversity_bonus(&contract.reward_card, cards);
        let token_bonus = Self::token_diversity_bonus(&contract.reward_card, cards);
        let reward_value =
            base_quality * BASE_REWARD_WEIGHT + (tag_bonus + token_bonus) * DIVERSITY_WEIGHT;

        // Penalize contracts whose requirements were tightened by the adaptive balance system.
        // Both negative pct (max lowered) and positive pct (min raised) make the contract harder.
        let adaptive_penalty: f64 = contract
            .adaptive_adjustments
            .iter()
            .map(|adj| adj.adjustment_percent.unsigned_abs() as f64 * TIGHTENING_PENALTY_PER_PCT)
            .sum::<f64>();

        let comfort = Self::comfort_tier(cards);
        let allowed_tier = comfort + 1;
        let overreach = (contract.tier.0 as i64 - allowed_tier as i64).max(0) as f64;
        // Scale the penalty by (1 - feasibility) so very-feasible contracts get a discount.
        // A perfectly feasible contract still pays half the overreach penalty; a barely
        // feasible one pays the full amount.
        let overreach_penalty = overreach * OVERREACH_PENALTY_PER_TIER * (1.0 - 0.5 * feasibility);

        // Stockpile bonus: this contract is being scored as a candidate to accept.
        // If its requirements include any token in the target's deficit map, reward it for
        // being a useful stepping stone. The bonus is gated on:
        //   - this contract has a min requirement for the deficit token
        //   - this contract has NO max constraint that the producer card we'd use would breach
        let stockpile_bonus: f64 = if !target_deficit.is_empty() {
            let mut bonus = 0.0;
            for req in &contract.requirements {
                if let ContractRequirementKind::TokenRequirement {
                    token_type,
                    min: Some(_),
                    ..
                } = req
                {
                    if let Some(&needed) = target_deficit.get(token_type) {
                        // Risk guard: skip if this same contract has a tight max
                        // on a token our deck produces a lot of (heuristic: any harmful max).
                        let has_tight_harmful_max = contract.requirements.iter().any(|r| {
                            matches!(
                                r,
                                ContractRequirementKind::TokenRequirement {
                                    max: Some(m),
                                    token_type: tt,
                                    ..
                                } if tt.is_harmful() && (*m as i64) < 20
                            )
                        });
                        if !has_tight_harmful_max {
                            // Cap the bonus at the deficit (no benefit from stockpiling more
                            // than the target needs).
                            let amount_useful = (needed as f64).min(20.0);
                            bonus += STOCKPILE_BONUS_PER_TOKEN * (amount_useful / 20.0);
                        }
                    }
                }
            }
            bonus
        } else {
            0.0
        };

        tier * TIER_WEIGHT - infeasibility_cost + advancement_bonus + reward_value
            - adaptive_penalty
            - overreach_penalty
            + stockpile_bonus
    }

    // Card play/discard scoring

    fn card_contract_score(
        card: &PlayerActionCard,
        contract: &Contract,
        token_balances: &HashMap<TokenType, i64>,
        tags_played: &HashMap<CardTag, u32>,
        contract_turns_played: u32,
    ) -> f64 {
        const TAG_MIN_BASE: f64 = 60.0;
        const TAG_MAX_OVERFLOW: f64 = -1e9;
        const TAG_MAX_NEAR_LIMIT: f64 = -150.0;

        let urgency = contract
            .requirements
            .iter()
            .find_map(|r| {
                if let ContractRequirementKind::TurnWindow {
                    max_turn: Some(m), ..
                } = r
                {
                    let left = (*m as i64 - contract_turns_played as i64).max(1) as f64;
                    Some((1.0 + (8.0 / left)).min(4.0))
                } else {
                    None
                }
            })
            .unwrap_or(1.0);

        let mut score = 0.0;
        for effect in &card.effects {
            for output in &effect.outputs {
                let amount = output.amount as f64;
                let current = *token_balances.get(&output.token_type).unwrap_or(&0) as f64;
                for req in &contract.requirements {
                    if let ContractRequirementKind::TokenRequirement {
                        token_type,
                        min,
                        max,
                    } = req
                    {
                        if token_type != &output.token_type {
                            continue;
                        }
                        if let Some(max_val) = max {
                            let max_f = *max_val as f64;
                            if current + amount > max_f {
                                return f64::NEG_INFINITY;
                            }
                            if current + amount > max_f * 0.85 {
                                return f64::NEG_INFINITY;
                            }
                            let remaining = max_f - current;
                            if amount > remaining * 0.70 {
                                score -= 200.0 * (amount / remaining.max(1.0));
                            }
                        }
                        if let Some(min_val) = min {
                            let needed = (*min_val as f64 - current).max(0.0);
                            if needed > 0.0 {
                                score += amount.min(needed) * 5.0 * urgency;
                            }
                        }
                    }
                }
            }
            for input in &effect.inputs {
                let amount = input.amount as f64;
                let current = *token_balances.get(&input.token_type).unwrap_or(&0) as f64;
                for req in &contract.requirements {
                    if let ContractRequirementKind::TokenRequirement {
                        token_type,
                        min,
                        max,
                    } = req
                    {
                        if token_type != &input.token_type {
                            continue;
                        }
                        if let Some(max_val) = max {
                            let headroom = *max_val as f64 - current;
                            if headroom < amount * 3.0 {
                                score += amount * 3.0;
                            }
                        }
                        if let Some(min_val) = min {
                            if current <= *min_val as f64 {
                                score -= amount * 5.0;
                            }
                        }
                    }
                }
            }
        }
        for tag in &card.tags {
            let played = *tags_played.get(tag).unwrap_or(&0) as f64;
            for req in &contract.requirements {
                if let ContractRequirementKind::CardTagConstraint {
                    tag: req_tag,
                    min,
                    max,
                } = req
                {
                    if req_tag != tag {
                        continue;
                    }

                    if let Some(min_val) = min {
                        let needed = (*min_val as f64 - played).max(0.0);
                        if needed > 0.0 {
                            score +=
                                TAG_MIN_BASE * urgency * (1.0 / needed.max(1.0)).clamp(0.05, 1.0)
                                    + TAG_MIN_BASE * 0.25 * urgency;
                        }
                    }

                    if let Some(max_val) = max {
                        let after = played + 1.0;
                        if after > *max_val as f64 {
                            return TAG_MAX_OVERFLOW;
                        }
                        let max_f = *max_val as f64;
                        if after >= max_f - 1.0 && max_f > 0.0 {
                            score += TAG_MAX_NEAR_LIMIT;
                        }
                    }
                }
            }
        }
        score
    }

    // Action builders

    fn choose_deckbuild_action(&self, state: &GameStateView) -> Option<PlayerAction> {
        let cards = &state.cards;

        // Collect valid indices based on card availability
        let replacement_indices_all: Vec<usize> = cards
            .iter()
            .enumerate()
            .filter(|(_, e)| e.counts.has_shelved())
            .map(|(i, _)| i)
            .collect();
        let hash_to_index = self.process_new_arrivals(cards, &replacement_indices_all);

        let target_indices: Vec<usize> = cards
            .iter()
            .enumerate()
            .filter(|(_, e)| e.counts.deck > 0 || e.counts.discard > 0)
            .map(|(i, _)| i)
            .collect();

        // Pass 1 uses a capped list; token production has no direct tag correlation
        let replacement_indices: Vec<usize> = replacement_indices_all
            .iter()
            .copied()
            .take(PASS1_CANDIDATES)
            .collect();
        let sacrifice_indices = replacement_indices_all.clone();

        if replacement_indices.is_empty() || target_indices.is_empty() {
            return None;
        }

        // Pass 1: diversity forcing
        let needed_tokens = Self::tokens_needing_diversity(cards, &state.offered_contracts);
        for token in &needed_tokens {
            if let Some(replacement) = replacement_indices
                .iter()
                .copied()
                .filter(|&i| Self::card_net_production(&cards[i].card, token) > 0.0)
                .max_by(|&a, &b| Self::by_quality(cards, a, b))
            {
                let worst_target = target_indices
                    .iter()
                    .copied()
                    .min_by(|&a, &b| Self::by_quality(cards, a, b))?;
                let worst_sacrifice = self.safe_sacrifice_index(
                    cards,
                    &sacrifice_indices,
                    replacement,
                    &hash_to_index,
                    &replacement_indices_all,
                )?;
                return Some(PlayerAction::ReplaceCard {
                    target_card_index: worst_target,
                    replacement_card_index: replacement,
                    sacrifice_card_index: worst_sacrifice,
                });
            }
        }

        // Pass 2: quality upgrade — O(tag count) lookup across ALL shelved cards
        let advancement_tokens =
            Self::tokens_needed_for_advancement(cards, &state.offered_contracts);
        let best_replacement = {
            let best = self.best_per_tag.borrow();
            best.values()
                .filter_map(|entries| {
                    entries
                        .iter()
                        .find_map(|(hash, _)| hash_to_index.get(hash).copied())
                })
                .max_by(|&a, &b| Self::by_quality(cards, a, b))
        }
        .or_else(|| {
            replacement_indices
                .iter()
                .copied()
                .max_by(|&a, &b| Self::by_quality(cards, a, b))
        })?;

        let worst_target = target_indices
            .iter()
            .copied()
            .filter(|&idx| {
                advancement_tokens
                    .iter()
                    .all(|t| Self::card_net_production(&cards[idx].card, t) <= 0.0)
            })
            .min_by(|&a, &b| Self::by_quality(cards, a, b))?;

        if Self::card_general_quality(&cards[best_replacement].card)
            <= Self::card_general_quality(&cards[worst_target].card)
        {
            return None;
        }

        let worst_sacrifice = self.safe_sacrifice_index(
            cards,
            &sacrifice_indices,
            best_replacement,
            &hash_to_index,
            &replacement_indices_all,
        )?;
        Some(PlayerAction::ReplaceCard {
            target_card_index: worst_target,
            replacement_card_index: best_replacement,
            sacrifice_card_index: worst_sacrifice,
        })
    }

    fn choose_play_card(
        valid_indices: &[usize],
        state: &GameStateView,
        token_balances: &HashMap<TokenType, i64>,
        tags_played: &HashMap<CardTag, u32>,
    ) -> Option<PlayerAction> {
        valid_indices
            .iter()
            .copied()
            .map(|i| {
                let card = &state.cards[i].card;
                if !Self::can_afford_card(card, token_balances) {
                    return (i, f64::NEG_INFINITY);
                }
                let score = if let Some(contract) = &state.active_contract {
                    Self::card_contract_score(
                        card,
                        contract,
                        token_balances,
                        tags_played,
                        state.contract_turns_played,
                    )
                } else {
                    Self::card_general_quality(card)
                };
                (i, score)
            })
            .filter(|&(_, score)| score.is_finite())
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| PlayerAction::PlayCard { card_index: idx })
    }

    fn choose_discard_card(
        &self,
        valid_indices: &[usize],
        state: &GameStateView,
        token_balances: &HashMap<TokenType, i64>,
        tags_played: &HashMap<CardTag, u32>,
    ) -> Option<PlayerAction> {
        let score = |idx: usize| -> f64 {
            if let Some(contract) = &state.active_contract {
                Self::card_contract_score(
                    &state.cards[idx].card,
                    contract,
                    token_balances,
                    tags_played,
                    state.contract_turns_played,
                )
            } else {
                Self::card_general_quality(&state.cards[idx].card)
            }
        };

        // Phase 0: discard the card most likely to push a near-limit harmful token over its max
        if let Some(contract) = &state.active_contract {
            let near_limit: Vec<(TokenType, i64)> = contract
                .requirements
                .iter()
                .filter_map(|req| {
                    if let ContractRequirementKind::TokenRequirement {
                        token_type,
                        max: Some(max),
                        ..
                    } = req
                    {
                        let current = token_balances.get(token_type).copied().unwrap_or(0);
                        let headroom = *max as i64 - current;
                        if headroom <= 5 {
                            Some((token_type.clone(), headroom))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();

            if !near_limit.is_empty() {
                let max_output_for_card = |idx: usize| -> f64 {
                    let card = &state.cards[idx].card;
                    card.effects
                        .iter()
                        .flat_map(|e| e.outputs.iter())
                        .filter_map(|ta| {
                            near_limit
                                .iter()
                                .find(|(tt, _)| *tt == ta.token_type)
                                .map(|(_, headroom)| (ta.amount as f64, *headroom))
                        })
                        .filter(|(amount, headroom)| *amount > *headroom as f64 * 0.5)
                        .map(|(amount, _)| amount)
                        .fold(f64::NEG_INFINITY, f64::max)
                };

                if let Some(idx) = valid_indices
                    .iter()
                    .copied()
                    .filter(|&i| max_output_for_card(i).is_finite())
                    .max_by(|&a, &b| {
                        max_output_for_card(a)
                            .partial_cmp(&max_output_for_card(b))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    return Some(PlayerAction::DiscardCard { card_index: idx });
                }
            }
        }

        // Phase 1: prefer discarding cards whose tags have a shelf backup
        let backed_tags: HashSet<CardTag> = self.best_per_tag.borrow().keys().cloned().collect();
        let phase1 = valid_indices
            .iter()
            .copied()
            .filter(|&i| {
                state.cards[i]
                    .card
                    .tags
                    .iter()
                    .any(|t| backed_tags.contains(t))
            })
            .min_by(|&a, &b| {
                score(a)
                    .partial_cmp(&score(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(idx) = phase1 {
            return Some(PlayerAction::DiscardCard { card_index: idx });
        }

        // Phase 2: fallback — score-based minimum across all candidates
        valid_indices
            .iter()
            .copied()
            .min_by(|&a, &b| {
                score(a)
                    .partial_cmp(&score(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|idx| PlayerAction::DiscardCard { card_index: idx })
    }

    fn target_progression_contract<'a>(
        offered: &'a [TierContracts],
        cards: &[CardEntry],
        token_balances: &HashMap<TokenType, i64>,
    ) -> Option<&'a Contract> {
        /// Tier weight when picking the stockpile target.
        /// Lower than score_contract::TIER_WEIGHT — we deliberately want
        /// reward quality to compete with raw tier preference.
        const TARGET_TIER_WEIGHT: f64 = 1_500.0;

        offered
            .iter()
            .flat_map(|tg| tg.contracts.iter())
            .filter(|c| !Self::is_contract_impossible(c, cards, token_balances, 0))
            .filter(|c| {
                c.requirements.iter().any(|req| {
                    if let ContractRequirementKind::TokenRequirement {
                        token_type,
                        min: Some(m),
                        ..
                    } = req
                    {
                        let cur = *token_balances.get(token_type).unwrap_or(&0) as f64;
                        *m as f64 > cur
                    } else {
                        false
                    }
                })
            })
            .max_by(|a, b| {
                let score = |c: &Contract| -> f64 {
                    let base = Self::card_general_quality(&c.reward_card);
                    let tag = Self::tag_diversity_bonus(&c.reward_card, cards);
                    let tok = Self::token_diversity_bonus(&c.reward_card, cards);
                    c.tier.0 as f64 * TARGET_TIER_WEIGHT + base + tag + tok
                };
                score(a)
                    .partial_cmp(&score(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    fn target_token_deficit(
        target: &Contract,
        token_balances: &HashMap<TokenType, i64>,
    ) -> HashMap<TokenType, u32> {
        let mut deficit = HashMap::new();
        for req in &target.requirements {
            if let ContractRequirementKind::TokenRequirement {
                token_type,
                min: Some(m),
                ..
            } = req
            {
                let cur = *token_balances.get(token_type).unwrap_or(&0);
                let still_needed = (*m as i64 - cur).max(0) as u32;
                if still_needed > 0 {
                    deficit.insert(token_type.clone(), still_needed);
                }
            }
        }
        deficit
    }

    fn choose_accept_contract(
        valid_tiers: &[my_little_factory_manager::game_state::TierContractRange],
        state: &GameStateView,
        token_balances: &HashMap<TokenType, i64>,
    ) -> Option<PlayerAction> {
        let offered = &state.offered_contracts;
        let needed_tokens = Self::tokens_needed_for_advancement(&state.cards, offered);

        // Compute the target contract and its token deficit once per call.
        let target = Self::target_progression_contract(offered, &state.cards, token_balances);
        let target_deficit = match target {
            Some(t) => Self::target_token_deficit(t, token_balances),
            None => HashMap::new(),
        };

        for feasible_only in [true, false] {
            let mut best: Option<(usize, usize, f64)> = None;
            for tier_range in valid_tiers {
                let tier_idx = tier_range.tier_index;
                let min_c = tier_range.valid_contract_index_range.min;
                let max_c = tier_range.valid_contract_index_range.max;
                if let Some(tier_contracts) = offered.get(tier_idx) {
                    for c_idx in min_c..=max_c {
                        if let Some(contract) = tier_contracts.contracts.get(c_idx) {
                            if feasible_only
                                && Self::is_contract_impossible(
                                    contract,
                                    &state.cards,
                                    token_balances,
                                    0,
                                )
                            {
                                continue;
                            }
                            let s = Self::score_contract(
                                contract,
                                &state.cards,
                                token_balances,
                                &needed_tokens,
                                &target_deficit,
                            );
                            if best.is_none_or(|(_, _, prev)| s > prev) {
                                best = Some((tier_idx, c_idx, s));
                            }
                        }
                    }
                }
            }
            if let Some((t, c, _)) = best {
                return Some(PlayerAction::AcceptContract {
                    tier_index: t,
                    contract_index: c,
                });
            }
        }

        let highest = valid_tiers.last()?;
        Some(PlayerAction::AcceptContract {
            tier_index: highest.tier_index,
            contract_index: highest.valid_contract_index_range.min,
        })
    }
}

impl Strategy for SmartStrategy {
    fn name(&self) -> &str {
        "smart"
    }

    fn needs_state(&self) -> bool {
        true
    }

    fn choose_action(
        &self,
        possible_actions: &[PossibleAction],
        snapshot: &GameSnapshot,
    ) -> PlayerAction {
        let state = snapshot
            .state
            .as_ref()
            .expect("SmartStrategy requires state");
        let token_balances = Self::token_balances(state);
        let tags_played = Self::tags_played(state);

        // Track actions and detect livelock (no contract resolution).
        self.actions_since_last_resolution
            .set(self.actions_since_last_resolution.get() + 1);

        // Detect if the active contract has changed and reset the counter.
        let current_signature = state.active_contract.as_ref().map(Self::contract_signature);
        let mut last_sig = self.last_active_contract_signature.borrow_mut();
        if *last_sig != current_signature {
            *last_sig = current_signature;
            self.actions_since_last_resolution.set(0);
        }
        drop(last_sig);

        // If livelock detected (too many actions with no contract resolution),
        // force-abandon to break the cycle.
        if self.actions_since_last_resolution.get() > NO_RESOLUTION_STUCK_THRESHOLD {
            if possible_actions
                .iter()
                .any(|a| matches!(a, PossibleAction::AbandonContract))
            {
                self.consecutive_discards.set(0);
                self.actions_since_last_resolution.set(0);
                return PlayerAction::AbandonContract;
            }
            // If abandon is not yet legal, discard to burn turns until it becomes legal.
            if let Some(PossibleAction::DiscardCard { valid_card_indices }) = possible_actions
                .iter()
                .find(|a| matches!(a, PossibleAction::DiscardCard { .. }))
            {
                if let Some(action) = self.choose_discard_card(
                    valid_card_indices,
                    state,
                    &token_balances,
                    &tags_played,
                ) {
                    self.consecutive_discards
                        .set(self.consecutive_discards.get() + 1);
                    return action;
                }
            }
        }

        // 0. Detect impossible active contract; abandon or discard to accumulate turns.
        let impossible = state.active_contract.as_ref().is_some_and(|c| {
            Self::is_contract_impossible(
                c,
                &state.cards,
                &token_balances,
                state.contract_turns_played,
            )
        });
        if impossible {
            if possible_actions
                .iter()
                .any(|a| matches!(a, PossibleAction::AbandonContract))
            {
                self.consecutive_discards.set(0);
                return PlayerAction::AbandonContract;
            }
            if let Some(PossibleAction::DiscardCard { valid_card_indices }) = possible_actions
                .iter()
                .find(|a| matches!(a, PossibleAction::DiscardCard { .. }))
            {
                if let Some(action) = self.choose_discard_card(
                    valid_card_indices,
                    state,
                    &token_balances,
                    &tags_played,
                ) {
                    self.consecutive_discards
                        .set(self.consecutive_discards.get() + 1);
                    return action;
                }
            }
        }

        // 0.5. Detect slow progress on active contract; abandon early.
        let too_slow = state.active_contract.as_ref().is_some_and(|c| {
            Self::is_progress_too_slow(
                c,
                &state.cards,
                &token_balances,
                state.contract_turns_played,
            )
        });
        if too_slow
            && possible_actions
                .iter()
                .any(|a| matches!(a, PossibleAction::AbandonContract))
        {
            self.consecutive_discards.set(0);
            return PlayerAction::AbandonContract;
        }

        // 1. Deckbuild when available and beneficial.
        if possible_actions
            .iter()
            .any(|a| matches!(a, PossibleAction::ReplaceCard))
        {
            if let Some(action) = self.choose_deckbuild_action(state) {
                return action;
            }
        }

        // 2. Play the best-scoring card.
        if let Some(PossibleAction::PlayCard { valid_card_indices }) = possible_actions
            .iter()
            .find(|a| matches!(a, PossibleAction::PlayCard { .. }))
        {
            if let Some(action) =
                Self::choose_play_card(valid_card_indices, state, &token_balances, &tags_played)
            {
                self.consecutive_discards.set(0);
                return action;
            }
        }

        // 3. Accept the highest-scoring contract.
        if let Some(PossibleAction::AcceptContract { valid_tiers }) = possible_actions
            .iter()
            .find(|a| matches!(a, PossibleAction::AcceptContract { .. }))
        {
            if let Some(action) = Self::choose_accept_contract(valid_tiers, state, &token_balances)
            {
                self.consecutive_discards.set(0);
                return action;
            }
        }

        // 4. Discard the least useful card, but abandon after DISCARD_STUCK_THRESHOLD.
        if self.consecutive_discards.get() < DISCARD_STUCK_THRESHOLD {
            if let Some(PossibleAction::DiscardCard { valid_card_indices }) = possible_actions
                .iter()
                .find(|a| matches!(a, PossibleAction::DiscardCard { .. }))
            {
                if let Some(action) = self.choose_discard_card(
                    valid_card_indices,
                    state,
                    &token_balances,
                    &tags_played,
                ) {
                    self.consecutive_discards
                        .set(self.consecutive_discards.get() + 1);
                    return action;
                }
            }
        }

        // 5. Abandon as last resort.
        if possible_actions
            .iter()
            .any(|a| matches!(a, PossibleAction::AbandonContract))
        {
            self.consecutive_discards.set(0);
            return PlayerAction::AbandonContract;
        }

        panic!(
            "SmartStrategy: no actionable option found in {:?}",
            possible_actions
        );
    }
}
