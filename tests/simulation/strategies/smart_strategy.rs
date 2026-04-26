use std::cell::Cell;
use std::collections::HashMap;

use my_little_factory_manager::action_log::PlayerAction;
use my_little_factory_manager::game_state::{GameStateView, PossibleAction};
use my_little_factory_manager::types::{
    CardEntry, CardTag, Contract, ContractRequirementKind, PlayerActionCard, TierContracts,
    TokenType,
};

use crate::game_driver::GameSnapshot;
use crate::strategies::Strategy;

const DISCARD_STUCK_THRESHOLD: u32 = 50;

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
pub struct SmartStrategy {
    consecutive_discards: Cell<u32>,
}

impl SmartStrategy {
    pub fn new() -> Self {
        Self {
            consecutive_discards: Cell::new(0),
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
            if in_cycle <= 0.0 { continue; }
            let prod = Self::card_net_production(&entry.card, token_type);
            if prod <= 0.0 { continue; }
            for _ in 0..(in_cycle as usize) { productions.push(prod); }
        }
        if productions.is_empty() { return 0.0; }
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
        cards.iter().filter(|e| {
            e.counts.has_non_shelved() && Self::card_net_production(&e.card, token_type) > 0.0
        }).count()
    }

    fn deck_producing_copy_count(cards: &[CardEntry], token_type: &TokenType) -> f64 {
        cards.iter()
            .filter(|e| Self::card_net_production(&e.card, token_type) > 0.0)
            .map(|e| e.counts.non_shelved() as f64)
            .sum()
    }

    fn shelved_producing_card_count(cards: &[CardEntry], token_type: &TokenType) -> usize {
        cards.iter().filter(|e| e.counts.has_shelved() && Self::card_net_production(&e.card, token_type) > 0.0).count()
    }
    fn deck_tag_count(cards: &[CardEntry], tag: &CardTag) -> f64 {
        cards.iter().filter(|e| e.counts.has_non_shelved() && e.card.tags.contains(tag)).map(|e| e.counts.non_shelved() as f64).sum()
    }
    fn deck_cycle_size(cards: &[CardEntry]) -> f64 {
        cards.iter().map(|e| e.counts.non_shelved() as f64).sum()
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
        cards: &[CardEntry],
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
            if let ContractRequirementKind::TurnWindow { max_turn, .. } = req { *max_turn } else { None }
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
                        if turns_left * mean_prod < remaining * 0.5 {
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

    fn tokens_needed_for_advancement(cards: &[CardEntry], offered: &[TierContracts]) -> Vec<TokenType> {
        const MIN_COPIES: f64 = 8.0;
        let mut needed = Vec::new();
        for req in offered.iter().flat_map(|tg| tg.contracts.iter()).flat_map(|c| c.requirements.iter()) {
            if let ContractRequirementKind::TokenRequirement { token_type, min: Some(_), .. } = req {
                if !needed.contains(token_type) && Self::deck_producing_copy_count(cards, token_type) < MIN_COPIES {
                    needed.push(token_type.clone());
                }
            }
        }
        needed
    }

    fn tokens_needing_diversity(cards: &[CardEntry], offered: &[TierContracts]) -> Vec<TokenType> {
        const MIN: f64 = 10.0;
        let mut needed = Vec::new();
        for req in offered.iter().flat_map(|tg| tg.contracts.iter()).flat_map(|c| c.requirements.iter()) {
            if let ContractRequirementKind::TokenRequirement { token_type, min: Some(_), .. } = req {
                if !needed.contains(token_type)
                    && Self::shelved_producing_card_count(cards, token_type) > 0
                    && Self::deck_producing_copy_count(cards, token_type) < MIN
                {
                    needed.push(token_type.clone());
                }
            }
        }
        needed
    }

    // Contract scoring

    fn score_contract(
        contract: &Contract,
        cards: &[CardEntry],
        token_balances: &HashMap<TokenType, i64>,
        needed_tokens: &[TokenType],
    ) -> f64 {
        const TIER_WEIGHT: f64 = 25_000.0;
        const ZERO_PRODUCER_PENALTY: f64 = 30_000.0;
        const SOFT_INFEASIBILITY_PENALTY: f64 = 3_000.0;
        const ADVANCEMENT_BONUS: f64 = 2_000.0;

        let tier = contract.tier.0 as f64;
        if contract.requirements.is_empty() {
            return tier * TIER_WEIGHT;
        }

        let max_turns = contract.requirements.iter().find_map(|req| {
            if let ContractRequirementKind::TurnWindow { max_turn, .. } = req { max_turn.map(|m| m as f64) } else { None }
        }).unwrap_or(50.0);

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
                            feasibility =
                                feasibility.min((deck_count / min_f).clamp(0.0, 1.0));
                        }
                    }
                    if let Some(max_val) = max {
                        if cycle_size > 0.0 {
                            let tagged = Self::deck_tag_count(cards, tag);
                            let banned_fraction =
                                (tagged - *max_val as f64).max(0.0) / cycle_size;
                            if banned_fraction > 0.5 {
                                feasibility = 0.0;
                            } else if banned_fraction > 0.1 {
                                feasibility = feasibility.min(1.0 - banned_fraction);
                            }
                        }
                    }
                }
                ContractRequirementKind::TurnWindow { .. } => {}
            }
        }

        let infeasibility_cost = if feasibility <= 0.0 {
            ZERO_PRODUCER_PENALTY
        } else {
            (1.0 - feasibility) * SOFT_INFEASIBILITY_PENALTY
        };

        let advancement_bonus: f64 = if feasibility > 0.0 {
            needed_tokens.iter()
                .map(|t| if Self::card_net_production(&contract.reward_card, t) > 0.0 { ADVANCEMENT_BONUS } else { 0.0 })
                .fold(0.0_f64, f64::max)
        } else { 0.0 };

        tier * TIER_WEIGHT - infeasibility_cost
            + advancement_bonus
            + Self::card_general_quality(&contract.reward_card) * 0.1
    }

    // Card play/discard scoring

    fn card_contract_score(
        card: &PlayerActionCard,
        contract: &Contract,
        token_balances: &HashMap<TokenType, i64>,
        tags_played: &HashMap<CardTag, u32>,
    ) -> f64 {
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
                            if amount > (max_f - current) * 0.75 {
                                score -= 50.0;
                            }
                        }
                        if let Some(min_val) = min {
                            let needed = (*min_val as f64 - current).max(0.0);
                            if needed > 0.0 {
                                score += amount.min(needed) * 5.0;
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
                    min: Some(min_val),
                    ..
                } = req
                {
                    if req_tag == tag && played < *min_val as f64 {
                        score += 5.0;
                    }
                }
            }
        }
        score
    }

    // Action builders

    fn choose_deckbuild_action(
        target_indices_raw: &[usize],
        replacement_indices_raw: &[usize],
        sacrifice_indices_raw: &[usize],
        state: &GameStateView,
    ) -> Option<PlayerAction> {
        const MAX_SHELVED_ENTRIES: usize = 30;
        const MAX_CANDIDATES: usize = 200;

        let cards = &state.cards;
        let shelved_count = replacement_indices_raw.len();
        let target_indices = target_indices_raw.to_vec();
        let replacement_indices: Vec<usize> =
            replacement_indices_raw.iter().copied().take(MAX_CANDIDATES).collect();
        let sacrifice_indices: Vec<usize> =
            sacrifice_indices_raw.iter().copied().take(MAX_CANDIDATES).collect();

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
                let worst_sacrifice =
                    Self::safe_sacrifice_index(cards, &sacrifice_indices, replacement)?;
                return Some(PlayerAction::ReplaceCard {
                    target_card_index: worst_target,
                    replacement_card_index: replacement,
                    sacrifice_card_index: worst_sacrifice,
                });
            }
        }

        // Pass 3: flood control
        if shelved_count > MAX_SHELVED_ENTRIES && sacrifice_indices.len() >= 2 {
            let best_replacement = replacement_indices
                .iter()
                .copied()
                .max_by(|&a, &b| Self::by_quality(cards, a, b))?;
            let worst_sacrifice =
                Self::safe_sacrifice_index(cards, &sacrifice_indices, best_replacement)?;
            let worst_target = target_indices
                .iter()
                .copied()
                .min_by(|&a, &b| Self::by_quality(cards, a, b))?;
            return Some(PlayerAction::ReplaceCard {
                target_card_index: worst_target,
                replacement_card_index: best_replacement,
                sacrifice_card_index: worst_sacrifice,
            });
        }

        // Pass 2: quality upgrade
        let advancement_tokens =
            Self::tokens_needed_for_advancement(cards, &state.offered_contracts);
        let best_replacement = replacement_indices
            .iter()
            .copied()
            .max_by(|&a, &b| Self::by_quality(cards, a, b))?;
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

        let worst_sacrifice =
            Self::safe_sacrifice_index(cards, &sacrifice_indices, best_replacement)?;
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
                    Self::card_contract_score(card, contract, token_balances, tags_played)
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
        valid_indices: &[usize],
        state: &GameStateView,
        token_balances: &HashMap<TokenType, i64>,
        tags_played: &HashMap<CardTag, u32>,
    ) -> Option<PlayerAction> {
        valid_indices
            .iter()
            .copied()
            .min_by(|&a, &b| {
                let score_a = if let Some(contract) = &state.active_contract {
                    Self::card_contract_score(
                        &state.cards[a].card,
                        contract,
                        token_balances,
                        tags_played,
                    )
                } else {
                    Self::card_general_quality(&state.cards[a].card)
                };
                let score_b = if let Some(contract) = &state.active_contract {
                    Self::card_contract_score(
                        &state.cards[b].card,
                        contract,
                        token_balances,
                        tags_played,
                    )
                } else {
                    Self::card_general_quality(&state.cards[b].card)
                };
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|idx| PlayerAction::DiscardCard { card_index: idx })
    }

    fn choose_accept_contract(
        valid_tiers: &[my_little_factory_manager::game_state::TierContractRange],
        state: &GameStateView,
        token_balances: &HashMap<TokenType, i64>,
    ) -> Option<PlayerAction> {
        let offered = &state.offered_contracts;
        let needed_tokens =
            Self::tokens_needed_for_advancement(&state.cards, offered);

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
                            );
                            if best.map_or(true, |(_, _, prev)| s > prev) {
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
        let state = snapshot.state.as_ref().expect("SmartStrategy requires state");
        let token_balances = Self::token_balances(state);
        let tags_played = Self::tags_played(state);

        // 0. Detect impossible active contract; abandon or discard to accumulate turns.
        let impossible = state.active_contract.as_ref().is_some_and(|c| {
            Self::is_contract_impossible(c, &state.cards, &token_balances, state.contract_turns_played)
        });
        if impossible {
            if possible_actions.iter().any(|a| matches!(a, PossibleAction::AbandonContract)) {
                self.consecutive_discards.set(0);
                return PlayerAction::AbandonContract;
            }
            if let Some(PossibleAction::DiscardCard { valid_card_indices }) =
                possible_actions.iter().find(|a| matches!(a, PossibleAction::DiscardCard { .. }))
            {
                if let Some(action) = Self::choose_discard_card(valid_card_indices, state, &token_balances, &tags_played) {
                    self.consecutive_discards.set(self.consecutive_discards.get() + 1);
                    return action;
                }
            }
        }

        // 1. Deckbuild when available and beneficial.
        if let Some(PossibleAction::ReplaceCard {
            valid_target_card_indices,
            valid_replacement_card_indices,
            valid_sacrifice_card_indices,
        }) = possible_actions
            .iter()
            .find(|a| matches!(a, PossibleAction::ReplaceCard { .. }))
        {
            if let Some(action) = Self::choose_deckbuild_action(
                valid_target_card_indices,
                valid_replacement_card_indices,
                valid_sacrifice_card_indices,
                state,
            ) {
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
        if let Some(PossibleAction::AcceptContract { valid_tiers }) =
            possible_actions.iter().find(|a| matches!(a, PossibleAction::AcceptContract { .. }))
        {
            if let Some(action) = Self::choose_accept_contract(valid_tiers, state, &token_balances) {
                self.consecutive_discards.set(0);
                return action;
            }
        }

        // 4. Discard the least useful card, but abandon after DISCARD_STUCK_THRESHOLD.
        if self.consecutive_discards.get() < DISCARD_STUCK_THRESHOLD {
            if let Some(PossibleAction::DiscardCard { valid_card_indices }) =
                possible_actions.iter().find(|a| matches!(a, PossibleAction::DiscardCard { .. }))
            {
                if let Some(action) = Self::choose_discard_card(valid_card_indices, state, &token_balances, &tags_played) {
                    self.consecutive_discards.set(self.consecutive_discards.get() + 1);
                    return action;
                }
            }
        }

        // 5. Abandon as last resort.
        if possible_actions.iter().any(|a| matches!(a, PossibleAction::AbandonContract)) {
            self.consecutive_discards.set(0);
            return PlayerAction::AbandonContract;
        }

        panic!(
            "SmartStrategy: no actionable option found in {:?}",
            possible_actions
        );
    }
}
