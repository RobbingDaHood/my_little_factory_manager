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
use crate::starter_cards::create_starter_deck;
use crate::types::{
    CardCounts, CardEffect, CardEntry, CardTag, Contract, ContractRequirementKind, ContractTier,
    PlayerActionCard, TierContracts, TokenAmount, TokenType,
};

use rocket::serde::Serialize;
use schemars::JsonSchema;

// ---------------------------------------------------------------------------
// Action result types
// ---------------------------------------------------------------------------

/// Typed outcome of processing a player action.
///
/// Each `PlayerAction` has dedicated success and error variants, making the
/// API response self-describing and exhaustive.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "result_type", crate = "rocket::serde")]
pub enum ActionResult {
    // -- success variants --------------------------------------------------
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

    // -- error variants ----------------------------------------------------
    ContractAlreadyActive,
    InvalidContractSelection {
        tier_index: usize,
        contract_index: usize,
    },
    NoActiveContract,
    InvalidHandIndex {
        index: usize,
        hand_size: usize,
    },
    InsufficientTokens,
}

// ---------------------------------------------------------------------------
// Serializable state view (for GET /state)
// ---------------------------------------------------------------------------

/// A read-only snapshot of the game state for the `/state` endpoint.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct GameStateView {
    pub seed: u64,
    pub turn_count: u32,
    pub cards: Vec<CardEntry>,
    pub tokens: Vec<TokenAmount>,
    pub active_contract: Option<Contract>,
    pub offered_contracts: Vec<TierContracts>,
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
    turn_count: u32,

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

        let mut cards = create_starter_deck();

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
            turn_count: 0,
            rules,
            action_log: ActionLog::new(),
        };

        // Generate first offered contracts
        state.generate_offered_contracts();

        state
    }

    // -------------------------------------------------------------------
    // State view
    // -------------------------------------------------------------------

    pub fn view(&self) -> GameStateView {
        GameStateView {
            seed: self.seed,
            turn_count: self.turn_count,
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

    pub fn seed(&self) -> u64 {
        self.seed
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
        ActionResult::NewGameStarted { seed: self.seed }
    }

    fn handle_accept_contract(&mut self, tier_index: usize, contract_index: usize) -> ActionResult {
        if self.active_contract.is_some() {
            return ActionResult::ContractAlreadyActive;
        }

        let contract = self
            .offered_contracts
            .get(tier_index)
            .and_then(|tc| tc.contracts.get(contract_index))
            .cloned();

        match contract {
            Some(c) => {
                self.offered_contracts[tier_index]
                    .contracts
                    .remove(contract_index);
                if self.offered_contracts[tier_index].contracts.is_empty() {
                    self.offered_contracts.remove(tier_index);
                }
                self.active_contract = Some(c);
                self.turn_count = 0;
                ActionResult::ContractAccepted
            }
            None => ActionResult::InvalidContractSelection {
                tier_index,
                contract_index,
            },
        }
    }

    fn handle_play_card(&mut self, hand_index: usize) -> ActionResult {
        if self.active_contract.is_none() {
            return ActionResult::NoActiveContract;
        }

        let hand_size = hand_total(&self.cards);
        if hand_index >= hand_size {
            return ActionResult::InvalidHandIndex {
                index: hand_index,
                hand_size,
            };
        }

        let entry_idx = resolve_hand_index(&self.cards, hand_index);
        let card = self.cards[entry_idx].card.clone();

        if !self.can_afford_effects(&card.effects) {
            return ActionResult::InsufficientTokens;
        }

        self.apply_effects(&card.effects);

        self.cards[entry_idx].counts.hand -= 1;
        self.cards[entry_idx].counts.discard += 1;

        draw_from_deck(&mut self.cards, &mut self.rng);
        self.turn_count += 1;

        let contract_completed = self.try_complete_contract();
        ActionResult::CardPlayed { contract_completed }
    }

    fn handle_discard_card(&mut self, hand_index: usize) -> ActionResult {
        if self.active_contract.is_none() {
            return ActionResult::NoActiveContract;
        }

        let hand_size = hand_total(&self.cards);
        if hand_index >= hand_size {
            return ActionResult::InvalidHandIndex {
                index: hand_index,
                hand_size,
            };
        }

        let entry_idx = resolve_hand_index(&self.cards, hand_index);
        self.cards[entry_idx].counts.hand -= 1;
        self.cards[entry_idx].counts.discard += 1;

        let bonus = self.rules.general.discard_production_unit_bonus;
        *self.tokens.entry(TokenType::ProductionUnit).or_insert(0) += bonus;

        draw_from_deck(&mut self.cards, &mut self.rng);
        self.turn_count += 1;

        let contract_completed = self.try_complete_contract();
        ActionResult::CardDiscarded { contract_completed }
    }

    // -------------------------------------------------------------------
    // Token mechanics
    // -------------------------------------------------------------------

    fn can_afford_effects(&self, effects: &[CardEffect]) -> bool {
        let mut required: HashMap<TokenType, u32> = HashMap::new();
        for effect in effects {
            for input in &effect.inputs {
                *required.entry(input.token_type.clone()).or_insert(0) += input.amount;
            }
        }
        for (token_type, needed) in &required {
            let available = self.tokens.get(token_type).copied().unwrap_or(0);
            if available < *needed {
                return false;
            }
        }
        true
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

    fn generate_offered_contracts(&mut self) {
        // Placeholder — Phase 3 will replace this with formula-based generation.
        let min_amount = 5u32;
        let max_amount = 15u32;
        let range = max_amount - min_amount + 1;
        let rolled = min_amount + (self.rng.next_u32() % range);

        let reward_card = PlayerActionCard {
            tags: vec![CardTag::Production],
            effects: vec![CardEffect::new(
                vec![],
                vec![TokenAmount {
                    token_type: TokenType::ProductionUnit,
                    amount: 2,
                }],
            )
            .expect("reward card effect is always valid")],
        };

        let contract = Contract {
            tier: ContractTier(1),
            requirements: vec![ContractRequirementKind::OutputThreshold {
                token_type: TokenType::ProductionUnit,
                min_amount: rolled,
            }],
            reward_card,
        };

        self.offered_contracts = vec![TierContracts {
            tier: ContractTier(1),
            contracts: vec![contract],
        }];
    }

    fn try_complete_contract(&mut self) -> Option<Contract> {
        let contract = self.active_contract.as_ref()?.clone();

        if !self.all_requirements_met(&contract) {
            return None;
        }

        self.subtract_contract_tokens(&contract);
        self.add_tokens(&TokenType::ContractsTierCompleted(contract.tier.0), 1);

        // Add reward card to library and deck
        self.add_reward_card(&contract.reward_card);

        self.active_contract = None;
        self.turn_count = 0;
        self.generate_offered_contracts();

        Some(contract)
    }

    fn add_reward_card(&mut self, card: &PlayerActionCard) {
        // Check if an identical card already exists in the library
        if let Some(entry) = self.cards.iter_mut().find(|e| e.card == *card) {
            entry.counts.library += 1;
            entry.counts.deck += 1;
        } else {
            self.cards.push(CardEntry {
                card: card.clone(),
                counts: CardCounts {
                    library: 1,
                    deck: 1,
                    hand: 0,
                    discard: 0,
                },
            });
        }
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
            ContractRequirementKind::TurnWindow { min_turn, max_turn } => {
                self.turn_count >= *min_turn && self.turn_count <= *max_turn
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

/// Draw one card from deck to hand using weighted random selection.
/// If deck is empty, shuffles discard back into deck first.
fn draw_from_deck(cards: &mut [CardEntry], rng: &mut Pcg64) {
    let deck_total: u32 = cards.iter().map(|e| e.counts.deck).sum();

    if deck_total == 0 {
        // Shuffle discard into deck
        let discard_total: u32 = cards.iter().map(|e| e.counts.discard).sum();
        if discard_total == 0 {
            return;
        }
        for entry in cards.iter_mut() {
            entry.counts.deck += entry.counts.discard;
            entry.counts.discard = 0;
        }
        // Now draw from the freshly reshuffled deck
        let new_deck_total: u32 = cards.iter().map(|e| e.counts.deck).sum();
        draw_weighted(cards, rng, new_deck_total);
    } else {
        draw_weighted(cards, rng, deck_total);
    }
}

/// Pick a random card from the deck (weighted by deck counts) and move one
/// copy from deck to hand.
fn draw_weighted(cards: &mut [CardEntry], rng: &mut Pcg64, deck_total: u32) {
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
