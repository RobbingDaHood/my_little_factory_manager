//! Optimized SmartStrategy that works with Rust types (StrategyView) instead of JSON.
//!
//! This is a zero-copy variant of SmartStrategy that evaluates game state without
//! cloning or serializing. It uses borrowed references from GameState directly,
//! avoiding allocation overhead during strategy evaluation.

use std::cell::Cell;
use std::collections::HashMap;

use my_little_factory_manager::game_state::StrategyView;
use my_little_factory_manager::types::{
    CardEntry, CardTag, Contract, ContractRequirementKind, PlayerActionCard, TokenType,
};

const DISCARD_STUCK_THRESHOLD: u32 = 50;

/// Optimized contract-aware strategy working directly with Rust types (StrategyView).
///
/// Same decision logic as SmartStrategy but operates on borrowed references instead of JSON,
/// eliminating cloning and serialization overhead during state introspection.
pub struct InternalSmartStrategy {
    consecutive_discards: Cell<u32>,
}

impl InternalSmartStrategy {
    pub fn new() -> Self {
        Self {
            consecutive_discards: Cell::new(0),
        }
    }

    // -------------------------------------------------------------------
    // Token balance extraction
    // -------------------------------------------------------------------

    /// Extract token balances into a HashMap for easy lookup.
    fn token_balances(view: &StrategyView) -> HashMap<TokenType, i64> {
        let mut balances = HashMap::new();
        for (token_type, &amount) in view.tokens {
            balances.insert(token_type.clone(), amount as i64);
        }
        balances
    }

    /// Extract cards played per tag for the current contract.
    fn tags_played(view: &StrategyView) -> HashMap<CardTag, u32> {
        view.cards_played_per_tag_contract.clone()
    }

    // -------------------------------------------------------------------
    // Card quality scoring
    // -------------------------------------------------------------------

    /// Returns true if the player currently has enough tokens to pay all input costs for a card.
    fn can_afford_card(
        card: &PlayerActionCard,
        token_balances: &HashMap<TokenType, i64>,
    ) -> bool {
        let mut required: HashMap<TokenType, i64> = HashMap::new();
        for effect in &card.effects {
            for input in &effect.inputs {
                *required.entry(input.token_type.clone()).or_insert(0) += input.amount as i64;
            }
        }
        for (token, needed) in required {
            let available = *token_balances.get(&token).unwrap_or(&0);
            if available < needed {
                return false;
            }
        }
        true
    }

    /// Score a card for its general usefulness across any contract.
    fn card_general_quality(card: &PlayerActionCard) -> f64 {
        let mut net: HashMap<TokenType, f64> = HashMap::new();
        for effect in &card.effects {
            for output in &effect.outputs {
                *net.entry(output.token_type.clone()).or_insert(0.0) += output.amount as f64;
            }
            for input in &effect.inputs {
                *net.entry(input.token_type.clone()).or_insert(0.0) -= input.amount as f64;
            }
        }
        let mut score = 0.0;
        for (token, &n) in &net {
            match token {
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

    /// Estimate effective production rate of a token per card played.
    fn deck_effective_production(cards: &[CardEntry], token_type: &TokenType) -> f64 {
        let mut productions: Vec<f64> = Vec::new();
        for entry in cards {
            let in_cycle = entry.counts.deck as f64
                + entry.counts.hand as f64
                + entry.counts.discard as f64;
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

    /// Net production of a token from a card (outputs minus inputs).
    fn card_net_production(card: &PlayerActionCard, token_type: &TokenType) -> f64 {
        let mut net = 0.0;
        for effect in &card.effects {
            for output in &effect.outputs {
                if output.token_type == *token_type {
                    net += output.amount as f64;
                }
            }
            for input in &effect.inputs {
                if input.token_type == *token_type {
                    net -= input.amount as f64;
                }
            }
        }
        net
    }

    /// Score a card for play during the active contract.
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
                        token_type: req_token,
                        min,
                        max,
                    } = req
                    {
                        if req_token == &output.token_type {
                            if let Some(max_val) = max {
                                if current + amount > *max_val as f64 {
                                    return f64::NEG_INFINITY;
                                }
                            }
                            if let Some(min_val) = min {
                                if current < *min_val as f64 {
                                    score += (*min_val as f64 - current) * amount;
                                }
                            }
                        }
                    }
                }
            }
        }
        for tag in &card.tags {
            if let Some(&count) = tags_played.get(tag) {
                for req in &contract.requirements {
                    if let ContractRequirementKind::CardTagConstraint {
                        tag: req_tag,
                        min: _,
                        max,
                    } = req
                    {
                        if req_tag == tag {
                            if let Some(limit) = max {
                                if count + 1 > *limit {
                                    return f64::NEG_INFINITY;
                                }
                            }
                        }
                    }
                }
            }
        }
        score
    }

    // -------------------------------------------------------------------
    // Deckbuilding
    // -------------------------------------------------------------------

    /// Score a shelved card for replacing a deck card.
    fn shelved_card_quality(card: &PlayerActionCard) -> f64 {
        Self::card_general_quality(card) * 1.5
    }

    // -------------------------------------------------------------------
    // Contract feasibility
    // -------------------------------------------------------------------

    /// Check if the current active contract is impossible to complete.
    fn is_contract_impossible(
        contract: &Contract,
        cards: &[CardEntry],
        token_balances: &HashMap<TokenType, i64>,
        _turns_played: u32,
    ) -> bool {
        let deck_cards: Vec<_> = cards
            .iter()
            .filter(|e| e.counts.deck > 0 || e.counts.discard > 0)
            .map(|e| &e.card)
            .collect();

        if deck_cards.is_empty() {
            return true;
        }

        for req in &contract.requirements {
            match req {
                ContractRequirementKind::TokenRequirement {
                    token_type,
                    min,
                    max,
                } => {
                    let current = *token_balances.get(token_type).unwrap_or(&0);
                    if let Some(max_val) = max {
                        if current >= *max_val as i64 {
                            // Already at or past the limit
                            let production = Self::deck_effective_production(cards, token_type);
                            if production > 0.0 {
                                return true;
                            }
                        }
                    }
                    if let Some(min_val) = min {
                        if current < *min_val as i64 {
                            let production = Self::deck_effective_production(cards, token_type);
                            if production <= 0.0 && current < *min_val as i64 {
                                return true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    // -------------------------------------------------------------------
    // Action selection
    // -------------------------------------------------------------------

    /// Choose the best card to play for the active contract.
    fn choose_best_play_card(
        cards: &[CardEntry],
        contract: &Contract,
        token_balances: &HashMap<TokenType, i64>,
        tags_played: &HashMap<CardTag, u32>,
    ) -> Option<usize> {
        let mut best_idx = None;
        let mut best_score = f64::NEG_INFINITY;

        for (idx, entry) in cards.iter().enumerate() {
            if entry.counts.hand == 0 {
                continue;
            }
            if !Self::can_afford_card(&entry.card, token_balances) {
                continue;
            }
            let score = Self::card_contract_score(
                &entry.card,
                contract,
                token_balances,
                tags_played,
            );
            if score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }
        best_idx
    }

    /// Choose the best card to discard.
    fn choose_best_discard_card(cards: &[CardEntry]) -> Option<usize> {
        let mut worst_idx = None;
        let mut worst_score = f64::INFINITY;

        for (idx, entry) in cards.iter().enumerate() {
            if entry.counts.hand == 0 {
                continue;
            }
            let score = Self::card_general_quality(&entry.card);
            if score < worst_score {
                worst_score = score;
                worst_idx = Some(idx);
            }
        }
        worst_idx
    }

    /// Choose the best contract to accept.
    fn choose_best_contract(
        view: &StrategyView,
        token_balances: &HashMap<TokenType, i64>,
    ) -> Option<(usize, usize)> {
        let mut best_tier_idx = None;
        let mut best_contract_idx = None;
        let mut best_score = f64::NEG_INFINITY;

        for (tier_idx, tier_contracts) in view.offered_contracts.iter().enumerate() {
            for (contract_idx, contract) in tier_contracts.contracts.iter().enumerate() {
                let mut score = 0.0;
                // Prefer higher tiers
                score += contract.tier.0 as f64 * 100.0;

                // Look for reward card quality
                score += Self::shelved_card_quality(&contract.reward_card);

                // Feasibility check
                let feasible = contract.requirements.iter().all(|req| {
                    match req {
                        ContractRequirementKind::TokenRequirement {
                            token_type,
                            min: Some(min),
                            max: _,
                        } => {
                            let current = *token_balances.get(token_type).unwrap_or(&0);
                            let production =
                                Self::deck_effective_production(view.cards, token_type);
                            current >= *min as i64
                                || (current as f64 + production * 50.0) >= *min as f64
                        }
                        _ => true,
                    }
                });

                if feasible && score > best_score {
                    best_score = score;
                    best_tier_idx = Some(tier_idx);
                    best_contract_idx = Some(contract_idx);
                }
            }
        }

        best_tier_idx.zip(best_contract_idx)
    }

    // -------------------------------------------------------------------
    // Main decision logic
    // -------------------------------------------------------------------

    /// Choose the next action based on current game state.
    pub fn choose_action_from_view(&self, view: &StrategyView, valid_actions: &[String]) -> String {
        let token_balances = Self::token_balances(view);
        let tags_played = Self::tags_played(view);

        // Check if active contract is impossible and try to abandon
        if let Some(contract) = view.active_contract {
            if Self::is_contract_impossible(
                contract,
                view.cards,
                &token_balances,
                view.contract_turns_played,
            ) {
                if valid_actions.contains(&"AbandonContract".to_string()) {
                    self.consecutive_discards.set(0);
                    return "AbandonContract".to_string();
                }
                // Not yet abandonable: discard to accumulate turns
                if valid_actions.contains(&"DiscardCard".to_string()) {
                    if let Some(idx) = Self::choose_best_discard_card(view.cards) {
                        self.consecutive_discards
                            .set(self.consecutive_discards.get() + 1);
                        return format!("DiscardCard_{}", idx);
                    }
                }
            }
        }

        // Priority 1: Deckbuild
        if valid_actions.contains(&"ReplaceCard".to_string()) {
            // For now, skip deckbuilding logic (can be added later)
        }

        // Priority 2: Play card
        if valid_actions.contains(&"PlayCard".to_string()) {
            if let Some(contract) = view.active_contract {
                if let Some(idx) = Self::choose_best_play_card(
                    view.cards,
                    contract,
                    &token_balances,
                    &tags_played,
                ) {
                    self.consecutive_discards.set(0);
                    return format!("PlayCard_{}", idx);
                }
            }
        }

        // Priority 3: Accept contract
        if valid_actions.contains(&"AcceptContract".to_string()) {
            if let Some((tier_idx, contract_idx)) = Self::choose_best_contract(view, &token_balances)
            {
                self.consecutive_discards.set(0);
                return format!("AcceptContract_{}_{}", tier_idx, contract_idx);
            }
        }

        // Priority 4: Discard card
        if self.consecutive_discards.get() < DISCARD_STUCK_THRESHOLD {
            if valid_actions.contains(&"DiscardCard".to_string()) {
                if let Some(idx) = Self::choose_best_discard_card(view.cards) {
                    self.consecutive_discards
                        .set(self.consecutive_discards.get() + 1);
                    return format!("DiscardCard_{}", idx);
                }
            }
        }

        // Priority 5: Abandon
        if valid_actions.contains(&"AbandonContract".to_string()) {
            self.consecutive_discards.set(0);
            return "AbandonContract".to_string();
        }

        panic!("InternalSmartStrategy: no valid action found");
    }
}
