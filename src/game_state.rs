//! Core game state: card locations, token balances, contract, and RNG.
//!
//! All game-mutating logic lives here. Endpoints and action dispatch
//! delegate to `GameState` methods, keeping this module the single
//! source of truth for game rules.

use std::collections::HashMap;

use rand::seq::SliceRandom;
use rand::RngCore;
use rand_pcg::Pcg64;

use crate::action_log::{ActionLog, PlayerAction};
use crate::config::GameRulesConfig;
use crate::config_loader::load_game_rules;
use crate::starter_cards::create_starter_deck;
use crate::types::{
    CardEffect, CardTag, Contract, ContractRequirementKind, ContractTier, PlayerActionCard,
    TokenAmount, TokenType,
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
    NoContractOffered,
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
    pub contracts_completed: u32,
    pub hand: Vec<PlayerActionCard>,
    pub deck_size: usize,
    pub discard_size: usize,
    pub tokens: HashMap<TokenType, u32>,
    pub active_contract: Option<Contract>,
    pub offered_contract: Option<Contract>,
}

// ---------------------------------------------------------------------------
// GameState
// ---------------------------------------------------------------------------

pub struct GameState {
    // Card management
    card_library: Vec<PlayerActionCard>,
    deck: Vec<usize>,
    hand: Vec<usize>,
    discard: Vec<usize>,

    // Token balances
    tokens: HashMap<TokenType, u32>,

    // Contract state
    active_contract: Option<Contract>,
    offered_contract: Option<Contract>,

    // RNG and metadata
    rng: Pcg64,
    seed: u64,
    turn_count: u32,
    contracts_completed: u32,

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

        let (library, mut deck_indices) = create_starter_deck();

        // Shuffle the deck
        deck_indices.shuffle(&mut rng);

        // Deal starting hand
        let hand_size = rules.general.starting_hand_size as usize;
        let hand: Vec<usize> = deck_indices
            .drain(deck_indices.len().saturating_sub(hand_size)..)
            .collect();

        let mut state = Self {
            card_library: library,
            deck: deck_indices,
            hand,
            discard: Vec::new(),
            tokens: HashMap::new(),
            active_contract: None,
            offered_contract: None,
            rng,
            seed: actual_seed,
            turn_count: 0,
            contracts_completed: 0,
            rules,
            action_log: ActionLog::new(),
        };

        // Generate first offered contract
        state.generate_offered_contract();

        state
    }

    // -------------------------------------------------------------------
    // State view
    // -------------------------------------------------------------------

    pub fn view(&self) -> GameStateView {
        GameStateView {
            seed: self.seed,
            turn_count: self.turn_count,
            contracts_completed: self.contracts_completed,
            hand: self
                .hand
                .iter()
                .map(|&i| self.card_library[i].clone())
                .collect(),
            deck_size: self.deck.len(),
            discard_size: self.discard.len(),
            tokens: self.tokens.clone(),
            active_contract: self.active_contract.clone(),
            offered_contract: self.offered_contract.clone(),
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
            PlayerAction::AcceptContract => self.handle_accept_contract(),
            PlayerAction::PlayCard { hand_index } => self.handle_play_card(hand_index),
            PlayerAction::DiscardCard { hand_index } => self.handle_discard_card(hand_index),
        }
    }

    // -------------------------------------------------------------------
    // Action handlers
    // -------------------------------------------------------------------

    fn handle_new_game(&mut self, seed: Option<u64>) -> ActionResult {
        let new_state = Self::new_with_rules(seed, self.rules.clone());
        // Preserve the action log entry we just appended
        let log = self.action_log.clone();
        *self = new_state;
        self.action_log = log;
        ActionResult::NewGameStarted { seed: self.seed }
    }

    fn handle_accept_contract(&mut self) -> ActionResult {
        if self.active_contract.is_some() {
            return ActionResult::ContractAlreadyActive;
        }

        match self.offered_contract.take() {
            Some(contract) => {
                self.active_contract = Some(contract);
                self.turn_count = 0;
                ActionResult::ContractAccepted
            }
            None => ActionResult::NoContractOffered,
        }
    }

    fn handle_play_card(&mut self, hand_index: usize) -> ActionResult {
        if self.active_contract.is_none() {
            return ActionResult::NoActiveContract;
        }

        if hand_index >= self.hand.len() {
            return ActionResult::InvalidHandIndex {
                index: hand_index,
                hand_size: self.hand.len(),
            };
        }

        let card_idx = self.hand[hand_index];
        let card = &self.card_library[card_idx];

        if !self.can_afford_effects(&card.effects) {
            return ActionResult::InsufficientTokens;
        }

        let card_clone = card.clone();
        self.apply_effects(&card_clone.effects);

        self.hand.remove(hand_index);
        self.discard.push(card_idx);

        self.draw_card();
        self.turn_count += 1;

        let contract_completed = self.try_complete_contract();
        ActionResult::CardPlayed { contract_completed }
    }

    fn handle_discard_card(&mut self, hand_index: usize) -> ActionResult {
        if self.active_contract.is_none() {
            return ActionResult::NoActiveContract;
        }

        if hand_index >= self.hand.len() {
            return ActionResult::InvalidHandIndex {
                index: hand_index,
                hand_size: self.hand.len(),
            };
        }

        let card_idx = self.hand.remove(hand_index);
        self.discard.push(card_idx);

        let bonus = self.rules.general.discard_production_unit_bonus;
        *self.tokens.entry(TokenType::ProductionUnit).or_insert(0) += bonus;

        self.draw_card();
        self.turn_count += 1;

        let contract_completed = self.try_complete_contract();
        ActionResult::CardDiscarded { contract_completed }
    }

    // -------------------------------------------------------------------
    // Card mechanics
    // -------------------------------------------------------------------

    /// Draw one card from the deck into the hand. If the deck is empty,
    /// shuffles the discard pile back into the deck first.
    fn draw_card(&mut self) {
        if self.deck.is_empty() && !self.discard.is_empty() {
            self.shuffle_discard_into_deck();
        }
        if let Some(card_idx) = self.deck.pop() {
            self.hand.push(card_idx);
        }
    }

    fn shuffle_discard_into_deck(&mut self) {
        self.deck.append(&mut self.discard);
        self.deck.shuffle(&mut self.rng);
    }

    // -------------------------------------------------------------------
    // Token mechanics
    // -------------------------------------------------------------------

    fn can_afford_effects(&self, effects: &[CardEffect]) -> bool {
        // Accumulate all required inputs
        let mut required: HashMap<TokenType, u32> = HashMap::new();
        for effect in effects {
            for input in &effect.inputs {
                *required.entry(input.token_type.clone()).or_insert(0) += input.amount;
            }
        }
        // Check availability
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

    fn generate_offered_contract(&mut self) {
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

        self.offered_contract = Some(contract);
    }

    fn try_complete_contract(&mut self) -> Option<Contract> {
        let contract = self.active_contract.as_ref()?.clone();

        if !self.all_requirements_met(&contract) {
            return None;
        }

        self.subtract_contract_tokens(&contract);
        self.contracts_completed += 1;
        self.active_contract = None;
        self.turn_count = 0;
        self.generate_offered_contract();

        Some(contract)
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
