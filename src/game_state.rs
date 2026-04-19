//! Core game state: card locations, token balances, contract, and RNG.
//!
//! All game-mutating logic lives here. Endpoints and action dispatch
//! delegate to `GameState` methods, keeping this module the single
//! source of truth for game rules.

use std::collections::HashMap;

use rand::RngCore;
use rand_pcg::Pcg64;

use crate::action_log::{ActionLog, PlayerAction};
use crate::config::GameRulesConfig;
use crate::config_loader::load_game_rules;
use crate::contract_generation::generate_contract;
use crate::starter_cards::create_starter_deck;
use crate::types::{
    add_card_to_entries, CardEffect, CardEntry, CardLocation, Contract, ContractRequirementKind,
    ContractTier, PlayerActionCard, TierContracts, TokenAmount, TokenType,
};

use rocket::serde::Serialize;
use schemars::JsonSchema;

// ---------------------------------------------------------------------------
// Action result types
// ---------------------------------------------------------------------------

/// Typed outcome of processing a player action.
///
/// Wraps `ActionSuccess` or `ActionError`, making the distinction explicit
/// at the type level. Each `PlayerAction` has dedicated success and error
/// variants so the API response is self-describing and exhaustive.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "outcome", content = "detail", crate = "rocket::serde")]
pub enum ActionResult {
    Success(ActionSuccess),
    Error(ActionError),
}

/// Successful outcomes — one variant per `PlayerAction`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "result_type", crate = "rocket::serde")]
pub enum ActionSuccess {
    NewGameStarted {
        seed: u64,
    },
    ContractAccepted,
    CardPlayed {
        #[serde(skip_serializing_if = "Option::is_none")]
        contract_completed: Option<Contract>,
    },
    CardDiscarded {
        #[serde(skip_serializing_if = "Option::is_none")]
        contract_completed: Option<Contract>,
    },
    /// A deck/discard card was replaced with a shelved card; sacrifice destroyed.
    CardReplaced,
}

/// Error outcomes — explicit variants for every failure mode.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "error_type", crate = "rocket::serde")]
pub enum ActionError {
    ContractAlreadyActive,
    InvalidTierIndex {
        tier_index: usize,
    },
    InvalidContractIndex {
        tier_index: usize,
        contract_index: usize,
    },
    NoActiveContract,
    InvalidHandIndex {
        index: usize,
    },
    InsufficientTokens {
        missing: Vec<TokenAmount>,
    },
    /// ReplaceCard attempted while a contract is active.
    ContractActiveForDeckbuilding,
    /// target_card_index is out of bounds.
    InvalidTargetCardIndex {
        index: usize,
    },
    /// Target card has no copies in the active cycle (deck or discard).
    NoTargetCopies {
        index: usize,
    },
    /// replacement_card_index is out of bounds.
    InvalidReplacementCardIndex {
        index: usize,
    },
    /// Replacement card has no copies on the shelf.
    NoShelvedCopies {
        index: usize,
    },
    /// sacrifice_card_index is out of bounds.
    InvalidSacrificeCardIndex {
        index: usize,
    },
    /// Sacrifice card has no shelved copies to destroy.
    NoSacrificeCopies {
        index: usize,
    },
    /// Cannot sacrifice the same card being replaced.
    SacrificeIsTarget {
        index: usize,
    },
}

// ---------------------------------------------------------------------------
// Serializable state view (for GET /state)
// ---------------------------------------------------------------------------

/// A read-only snapshot of the game state for the `/state` endpoint.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct GameStateView {
    pub seed: u64,
    pub cards: Vec<CardEntry>,
    pub tokens: Vec<TokenAmount>,
    pub active_contract: Option<Contract>,
    pub offered_contracts: Vec<TierContracts>,
}

/// Token balances grouped by tag category for the `/player/tokens` endpoint.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct PlayerTokensView {
    pub beneficial: Vec<TokenAmount>,
    pub harmful: Vec<TokenAmount>,
    pub progression: Vec<TokenAmount>,
}

/// A possible action the player can take in the current game state.
///
/// An inclusive range of valid indices.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct IndexRange {
    pub min: usize,
    pub max: usize,
}

/// The valid contract index range within a single tier.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TierContractRange {
    pub tier_index: usize,
    pub valid_contract_index_range: IndexRange,
}

/// A compact descriptor of what actions the player can take.
///
/// Instead of enumerating every concrete action instance, each variant
/// describes the valid parameter ranges for its action type.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "action_type", crate = "rocket::serde")]
pub enum PossibleAction {
    NewGame,
    PlayCard {
        valid_hand_index_range: IndexRange,
    },
    DiscardCard {
        valid_hand_index_range: IndexRange,
    },
    AcceptContract {
        valid_tiers: Vec<TierContractRange>,
    },
    ReplaceCard {
        valid_target_card_indices: Vec<usize>,
        valid_replacement_card_indices: Vec<usize>,
        valid_sacrifice_card_indices: Vec<usize>,
    },
}

// ---------------------------------------------------------------------------
// GameState
// ---------------------------------------------------------------------------

pub struct GameState {
    // Card management (count-based)
    cards: Vec<CardEntry>,

    // Token balances
    tokens: HashMap<TokenType, u32>,

    // Contract state
    active_contract: Option<Contract>,
    offered_contracts: Vec<TierContracts>,

    // RNG and metadata
    rng: Pcg64,
    seed: u64,

    // Config
    rules: GameRulesConfig,

    // Action log
    action_log: ActionLog,
}

impl GameState {
    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// Create a new game with the given seed. If `None`, generates a random seed.
    pub fn new(seed: Option<u64>) -> Self {
        let rules = load_game_rules().expect("embedded game rules must parse");
        Self::new_with_rules(seed, rules)
    }

    /// Create a new game with explicit rules (useful for testing).
    pub fn new_with_rules(seed: Option<u64>, rules: GameRulesConfig) -> Self {
        let actual_seed = seed.unwrap_or_else(|| {
            let mut fallback_rng = Pcg64::new(
                0xcafe_f00d_d15e_a5e5,
                0xa02b_dbf7_bb3c_0a7a_c28f_5c28_f5c2_8f5c,
            );
            fallback_rng.next_u64()
        });

        let mut rng = Pcg64::new(
            actual_seed as u128,
            0xa02b_dbf7_bb3c_0a7a_c28f_5c28_f5c2_8f5c,
        );

        let mut cards = create_starter_deck(rules.general.starting_deck_size, &mut rng);

        // Deal starting hand
        let hand_size = rules.general.starting_hand_size;
        for _ in 0..hand_size {
            draw_from_deck(&mut cards, &mut rng);
        }

        let mut state = Self {
            cards,
            tokens: HashMap::new(),
            active_contract: None,
            offered_contracts: Vec::new(),
            rng,
            seed: actual_seed,
            rules: rules.clone(),
            action_log: ActionLog::new(),
        };

        // Initialize deck slots to starting deck size
        state.add_tokens(&TokenType::DeckSlots, rules.general.starting_deck_size);

        // Generate first offered contracts
        state.refill_contract_market();

        state
    }

    // -------------------------------------------------------------------
    // State view
    // -------------------------------------------------------------------

    pub fn view(&self) -> GameStateView {
        GameStateView {
            seed: self.seed,
            cards: self.cards.clone(),
            tokens: {
                let mut t: Vec<_> = self
                    .tokens
                    .iter()
                    .map(|(token_type, &amount)| TokenAmount {
                        token_type: token_type.clone(),
                        amount,
                    })
                    .collect();
                t.sort_by(|a, b| a.token_type.cmp(&b.token_type));
                t
            },
            active_contract: self.active_contract.clone(),
            offered_contracts: self.offered_contracts.clone(),
        }
    }

    pub fn action_log(&self) -> &ActionLog {
        &self.action_log
    }

    pub fn offered_contracts(&self) -> &[TierContracts] {
        &self.offered_contracts
    }

    pub fn active_contract(&self) -> Option<&Contract> {
        self.active_contract.as_ref()
    }

    pub fn cards(&self) -> &[CardEntry] {
        &self.cards
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Token balances grouped by tag for the `/player/tokens` endpoint.
    pub fn tokens_view(&self) -> PlayerTokensView {
        use crate::types::TokenTag;

        let all_tokens: Vec<TokenAmount> = self
            .tokens
            .iter()
            .filter(|(_, &amount)| amount > 0)
            .map(|(token_type, &amount)| TokenAmount {
                token_type: token_type.clone(),
                amount,
            })
            .collect();

        let mut beneficial: Vec<TokenAmount> = all_tokens
            .iter()
            .filter(|t| t.token_type.tags().contains(&TokenTag::Beneficial))
            .cloned()
            .collect();
        beneficial.sort_by(|a, b| a.token_type.cmp(&b.token_type));

        let mut harmful: Vec<TokenAmount> = all_tokens
            .iter()
            .filter(|t| t.token_type.tags().contains(&TokenTag::Harmful))
            .cloned()
            .collect();
        harmful.sort_by(|a, b| a.token_type.cmp(&b.token_type));

        let mut progression: Vec<TokenAmount> = all_tokens
            .iter()
            .filter(|t| t.token_type.tags().contains(&TokenTag::Progression))
            .cloned()
            .collect();
        progression.sort_by(|a, b| a.token_type.cmp(&b.token_type));

        PlayerTokensView {
            beneficial,
            harmful,
            progression,
        }
    }

    /// Returns the list of valid actions in the current game state.
    pub fn possible_actions(&self) -> Vec<PossibleAction> {
        let mut actions = Vec::new();

        actions.push(PossibleAction::NewGame);

        if self.active_contract.is_some() {
            let hand_size = hand_total(&self.cards);
            if hand_size > 0 {
                let range = IndexRange {
                    min: 0,
                    max: hand_size - 1,
                };
                actions.push(PossibleAction::PlayCard {
                    valid_hand_index_range: range.clone(),
                });
                actions.push(PossibleAction::DiscardCard {
                    valid_hand_index_range: range,
                });
            }
        } else {
            let valid_tiers: Vec<TierContractRange> = self
                .offered_contracts
                .iter()
                .enumerate()
                .filter(|(_, tc)| !tc.contracts.is_empty())
                .map(|(tier_idx, tc)| TierContractRange {
                    tier_index: tier_idx,
                    valid_contract_index_range: IndexRange {
                        min: 0,
                        max: tc.contracts.len() - 1,
                    },
                })
                .collect();

            if !valid_tiers.is_empty() {
                actions.push(PossibleAction::AcceptContract { valid_tiers });
            }

            // List ReplaceCard options when shelved and sacrifice candidates exist
            self.add_replace_card_action(&mut actions);
        }

        actions
    }

    /// Adds a single ReplaceCard action descriptor with valid index sets.
    fn add_replace_card_action(&self, actions: &mut Vec<PossibleAction>) {
        // Collect shelved card indices (cards with copies on the shelf)
        let shelved_indices: Vec<usize> = self
            .cards
            .iter()
            .enumerate()
            .filter(|(_, e)| e.counts.has_shelved())
            .map(|(i, _)| i)
            .collect();

        if shelved_indices.is_empty() {
            return;
        }

        // Target cards: any card with copies in deck or discard
        let target_indices: Vec<usize> = self
            .cards
            .iter()
            .enumerate()
            .filter(|(_, e)| e.counts.deck > 0 || e.counts.discard > 0)
            .map(|(i, _)| i)
            .collect();

        if target_indices.is_empty() {
            return;
        }

        // Sacrifice candidates: any card with copies on the shelf
        let sacrifice_indices: Vec<usize> = self
            .cards
            .iter()
            .enumerate()
            .filter(|(_, e)| e.counts.has_shelved())
            .map(|(i, _)| i)
            .collect();

        if sacrifice_indices.is_empty() {
            return;
        }

        actions.push(PossibleAction::ReplaceCard {
            valid_target_card_indices: target_indices,
            valid_replacement_card_indices: shelved_indices,
            valid_sacrifice_card_indices: sacrifice_indices,
        });
    }

    // -------------------------------------------------------------------
    // Action dispatch
    // -------------------------------------------------------------------

    /// Process a player action and return the result.
    pub fn dispatch(&mut self, action: PlayerAction) -> ActionResult {
        self.action_log.append(action.clone());

        match action {
            PlayerAction::NewGame { seed } => self.handle_new_game(seed),
            PlayerAction::AcceptContract {
                tier_index,
                contract_index,
            } => self.handle_accept_contract(tier_index, contract_index),
            PlayerAction::PlayCard { hand_index } => self.handle_play_card(hand_index),
            PlayerAction::DiscardCard { hand_index } => self.handle_discard_card(hand_index),
            PlayerAction::ReplaceCard {
                target_card_index,
                replacement_card_index,
                sacrifice_card_index,
            } => self.handle_replace_card(
                target_card_index,
                replacement_card_index,
                sacrifice_card_index,
            ),
        }
    }

    // -------------------------------------------------------------------
    // Action handlers
    // -------------------------------------------------------------------

    fn handle_new_game(&mut self, seed: Option<u64>) -> ActionResult {
        let new_state = Self::new_with_rules(seed, self.rules.clone());
        let log = self.action_log.clone();
        *self = new_state;
        self.action_log = log;
        ActionResult::Success(ActionSuccess::NewGameStarted { seed: self.seed })
    }

    fn handle_accept_contract(&mut self, tier_index: usize, contract_index: usize) -> ActionResult {
        if self.active_contract.is_some() {
            return ActionResult::Error(ActionError::ContractAlreadyActive);
        }

        let tier = match self.offered_contracts.get(tier_index) {
            Some(tc) => tc,
            None => {
                return ActionResult::Error(ActionError::InvalidTierIndex { tier_index });
            }
        };

        let contract = match tier.contracts.get(contract_index) {
            Some(c) => c.clone(),
            None => {
                return ActionResult::Error(ActionError::InvalidContractIndex {
                    tier_index,
                    contract_index,
                });
            }
        };

        self.offered_contracts[tier_index]
            .contracts
            .remove(contract_index);
        if self.offered_contracts[tier_index].contracts.is_empty() {
            self.offered_contracts.remove(tier_index);
        }
        self.active_contract = Some(contract);
        ActionResult::Success(ActionSuccess::ContractAccepted)
    }

    fn handle_play_card(&mut self, hand_index: usize) -> ActionResult {
        if self.active_contract.is_none() {
            return ActionResult::Error(ActionError::NoActiveContract);
        }

        let hand_size = hand_total(&self.cards);
        if hand_index >= hand_size {
            return ActionResult::Error(ActionError::InvalidHandIndex { index: hand_index });
        }

        let entry_idx = resolve_hand_index(&self.cards, hand_index);
        let card = self.cards[entry_idx].card.clone();

        let missing = self.get_missing_tokens_for_effects(&card.effects);
        if !missing.is_empty() {
            return ActionResult::Error(ActionError::InsufficientTokens { missing });
        }

        self.apply_effects(&card.effects);

        self.cards[entry_idx].counts.hand -= 1;
        self.cards[entry_idx].counts.discard += 1;

        draw_from_deck(&mut self.cards, &mut self.rng);

        let contract_completed = self.try_complete_contract();
        ActionResult::Success(ActionSuccess::CardPlayed { contract_completed })
    }

    fn handle_discard_card(&mut self, hand_index: usize) -> ActionResult {
        if self.active_contract.is_none() {
            return ActionResult::Error(ActionError::NoActiveContract);
        }

        let hand_size = hand_total(&self.cards);
        if hand_index >= hand_size {
            return ActionResult::Error(ActionError::InvalidHandIndex { index: hand_index });
        }

        let entry_idx = resolve_hand_index(&self.cards, hand_index);
        self.cards[entry_idx].counts.hand -= 1;
        self.cards[entry_idx].counts.discard += 1;

        let bonus = self.rules.general.discard_production_unit_bonus;
        *self.tokens.entry(TokenType::ProductionUnit).or_insert(0) += bonus;

        draw_from_deck(&mut self.cards, &mut self.rng);

        let contract_completed = self.try_complete_contract();
        ActionResult::Success(ActionSuccess::CardDiscarded { contract_completed })
    }

    fn handle_replace_card(
        &mut self,
        target_card_index: usize,
        replacement_card_index: usize,
        sacrifice_card_index: usize,
    ) -> ActionResult {
        // Must not have an active contract
        if self.active_contract.is_some() {
            return ActionResult::Error(ActionError::ContractActiveForDeckbuilding);
        }

        let card_count = self.cards.len();

        // Validate target index
        if target_card_index >= card_count {
            return ActionResult::Error(ActionError::InvalidTargetCardIndex {
                index: target_card_index,
            });
        }

        // Auto-determine location: deck first, then discard
        let target_entry = &self.cards[target_card_index];
        let use_deck = target_entry.counts.deck > 0;
        let use_discard = target_entry.counts.discard > 0;
        if !use_deck && !use_discard {
            return ActionResult::Error(ActionError::NoTargetCopies {
                index: target_card_index,
            });
        }

        // Validate replacement index
        if replacement_card_index >= card_count {
            return ActionResult::Error(ActionError::InvalidReplacementCardIndex {
                index: replacement_card_index,
            });
        }

        // Validate replacement has shelved copies
        let replacement = &self.cards[replacement_card_index].counts;
        if !replacement.has_shelved() {
            return ActionResult::Error(ActionError::NoShelvedCopies {
                index: replacement_card_index,
            });
        }

        // Validate sacrifice index
        if sacrifice_card_index >= card_count {
            return ActionResult::Error(ActionError::InvalidSacrificeCardIndex {
                index: sacrifice_card_index,
            });
        }

        // Sacrifice == target is allowed when the card has shelved copies
        // (one will be destroyed by the sacrifice).
        if sacrifice_card_index == target_card_index
            && !self.cards[sacrifice_card_index].counts.has_shelved()
        {
            return ActionResult::Error(ActionError::SacrificeIsTarget {
                index: sacrifice_card_index,
            });
        }

        // Sacrifice must come from shelved copies
        if !self.cards[sacrifice_card_index].counts.has_shelved() {
            return ActionResult::Error(ActionError::NoSacrificeCopies {
                index: sacrifice_card_index,
            });
        }

        // Sacrifice == replacement requires at least 2 shelved copies
        // (one consumed by replacement move, one destroyed by sacrifice).
        if sacrifice_card_index == replacement_card_index
            && self.cards[sacrifice_card_index].counts.shelved < 2
        {
            return ActionResult::Error(ActionError::NoSacrificeCopies {
                index: sacrifice_card_index,
            });
        }

        // --- Apply the replacement ---

        // Remove target from deck (preferred) or discard, move to shelf
        if use_deck {
            self.cards[target_card_index].counts.deck -= 1;
        } else {
            self.cards[target_card_index].counts.discard -= 1;
        }
        self.cards[target_card_index].counts.shelved += 1;

        // Move replacement from shelf to the deck
        self.cards[replacement_card_index].counts.shelved -= 1;
        self.cards[replacement_card_index].counts.deck += 1;

        // Destroy sacrifice (permanently remove from shelf)
        self.cards[sacrifice_card_index].counts.shelved -= 1;

        // Clean up entries where all counts are zero
        self.cards.retain(|e| e.counts.total() > 0);

        ActionResult::Success(ActionSuccess::CardReplaced)
    }

    // -------------------------------------------------------------------
    // Token mechanics
    // -------------------------------------------------------------------

    /// Returns the tokens the player is missing to pay all inputs of the given effects.
    /// An empty Vec means the player can afford them.
    fn get_missing_tokens_for_effects(&self, effects: &[CardEffect]) -> Vec<TokenAmount> {
        let mut required: HashMap<TokenType, u32> = HashMap::new();
        for effect in effects {
            for input in &effect.inputs {
                *required.entry(input.token_type.clone()).or_insert(0) += input.amount;
            }
        }
        let mut missing = Vec::new();
        for (token_type, needed) in required {
            let available = self.tokens.get(&token_type).copied().unwrap_or(0);
            if available < needed {
                missing.push(TokenAmount {
                    token_type,
                    amount: needed - available,
                });
            }
        }
        missing.sort_by(|a, b| a.token_type.cmp(&b.token_type));
        missing
    }

    fn apply_effects(&mut self, effects: &[CardEffect]) {
        for effect in effects {
            for input in &effect.inputs {
                self.remove_tokens(&input.token_type, input.amount);
            }
            for output in &effect.outputs {
                self.add_tokens(&output.token_type, output.amount);
            }
        }
    }

    fn add_tokens(&mut self, token_type: &TokenType, amount: u32) {
        *self.tokens.entry(token_type.clone()).or_insert(0) += amount;
    }

    fn remove_tokens(&mut self, token_type: &TokenType, amount: u32) {
        let balance = self.tokens.entry(token_type.clone()).or_insert(0);
        *balance = balance.saturating_sub(amount);
    }

    // -------------------------------------------------------------------
    // Contract mechanics
    // -------------------------------------------------------------------

    fn refill_contract_market(&mut self) {
        let target = self.rules.general.contract_market_size_per_tier;

        for tier_num in self.unlocked_tiers() {
            let tier = ContractTier(tier_num);

            let existing_count = self
                .offered_contracts
                .iter()
                .find(|tc| tc.tier == tier)
                .map(|tc| tc.contracts.len() as u32)
                .unwrap_or(0);

            let needed = target.saturating_sub(existing_count);
            if needed == 0 {
                continue;
            }

            let new_contracts: Vec<Contract> = (0..needed)
                .map(|_| generate_contract(tier, &mut self.rng, &self.rules.contract_formulas))
                .collect();

            if let Some(tc) = self.offered_contracts.iter_mut().find(|tc| tc.tier == tier) {
                tc.contracts.extend(new_contracts);
            } else {
                self.offered_contracts.push(TierContracts {
                    tier,
                    contracts: new_contracts,
                });
            }
        }
    }

    /// Returns the list of unlocked tier numbers.
    /// Tier 0 is always unlocked. Tier N+1 unlocks when
    /// `ContractsTierCompleted(N) >= contracts_per_tier_to_advance`.
    fn unlocked_tiers(&self) -> Vec<u32> {
        let threshold = self.rules.general.contracts_per_tier_to_advance;
        let mut tiers = vec![0u32];
        for tier in 0.. {
            let completed = self
                .tokens
                .get(&TokenType::ContractsTierCompleted(tier))
                .copied()
                .unwrap_or(0);
            if completed >= threshold {
                tiers.push(tier + 1);
            } else {
                break;
            }
        }
        tiers
    }

    fn try_complete_contract(&mut self) -> Option<Contract> {
        let contract = self.active_contract.as_ref()?.clone();

        if !self.all_requirements_met(&contract) {
            return None;
        }

        self.subtract_contract_tokens(&contract);
        self.add_tokens(&TokenType::ContractsTierCompleted(contract.tier.0), 1);

        // Add reward card to shelved
        self.add_reward_card(&contract.reward_card);

        self.active_contract = None;
        self.refill_contract_market();

        Some(contract)
    }

    fn add_reward_card(&mut self, card: &PlayerActionCard) {
        add_card_to_entries(&mut self.cards, card, CardLocation::Shelved);
    }

    fn all_requirements_met(&self, contract: &Contract) -> bool {
        contract.requirements.iter().all(|req| match req {
            ContractRequirementKind::OutputThreshold {
                token_type,
                min_amount,
            } => self.tokens.get(token_type).copied().unwrap_or(0) >= *min_amount,
            ContractRequirementKind::HarmfulTokenLimit {
                token_type,
                max_amount,
            } => self.tokens.get(token_type).copied().unwrap_or(0) <= *max_amount,
            ContractRequirementKind::CardTagRestriction { .. } => {
                // Phase 2 does not generate contracts with tag restrictions
                true
            }
            ContractRequirementKind::TurnWindow { .. } => {
                // TODO: Phase 7 statistics will provide turn tracking for this requirement
                true
            }
        })
    }

    fn subtract_contract_tokens(&mut self, contract: &Contract) {
        for req in &contract.requirements {
            if let ContractRequirementKind::OutputThreshold {
                token_type,
                min_amount,
            } = req
            {
                self.remove_tokens(token_type, *min_amount);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Card helper functions (free functions operating on Vec<CardEntry>)
// ---------------------------------------------------------------------------

/// Total number of cards currently in hand.
fn hand_total(cards: &[CardEntry]) -> usize {
    cards.iter().map(|e| e.counts.hand as usize).sum()
}

/// Given a hand_index (position in the expanded hand), return the
/// index into the cards Vec for the corresponding entry.
fn resolve_hand_index(cards: &[CardEntry], hand_index: usize) -> usize {
    let mut remaining = hand_index;
    for (i, entry) in cards.iter().enumerate() {
        let count = entry.counts.hand as usize;
        if remaining < count {
            return i;
        }
        remaining -= count;
    }
    unreachable!("hand_index validated before calling resolve_hand_index")
}

/// Draw one card from deck to hand via random selection.
/// If deck is empty, recycles discard counts back into deck first.
fn draw_from_deck(cards: &mut [CardEntry], rng: &mut Pcg64) {
    let deck_total: u32 = cards.iter().map(|e| e.counts.deck).sum();

    if deck_total == 0 {
        let mut discard_total = 0u32;
        for entry in cards.iter_mut() {
            discard_total += entry.counts.discard;
            entry.counts.deck += entry.counts.discard;
            entry.counts.discard = 0;
        }
        if discard_total == 0 {
            return;
        }
        draw_random(cards, rng, discard_total);
    } else {
        draw_random(cards, rng, deck_total);
    }
}

/// Pick a random card from the deck proportional to deck counts and move
/// one copy from deck to hand.
fn draw_random(cards: &mut [CardEntry], rng: &mut Pcg64, deck_total: u32) {
    let roll = rng.next_u32() % deck_total;
    let mut cumulative = 0u32;
    for entry in cards.iter_mut() {
        cumulative += entry.counts.deck;
        if roll < cumulative {
            entry.counts.deck -= 1;
            entry.counts.hand += 1;
            return;
        }
    }
}
