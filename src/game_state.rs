//! Core game state: card locations, token balances, contract, and RNG.
//!
//! All game-mutating logic lives here. Endpoints and action dispatch
//! delegate to `GameState` methods, keeping this module the single
//! source of truth for game rules.

use std::collections::HashMap;

use rand::RngCore;
use rand_pcg::Pcg64;

use crate::action_log::{ActionLog, PlayerAction};
use crate::adaptive_balance::AdaptiveBalanceTracker;
use crate::config::{CachedConfig, GameRulesConfig};
use crate::config_loader::{load_game_rules, load_token_definitions};
use crate::contract_generation::{build_cached_config, generate_contract_with_types};
use crate::metrics::{MetricsTracker, SessionMetrics};
use crate::starter_cards::create_starter_deck;
use crate::types::{
    add_card_to_entries, CardEffect, CardEntry, CardLocation, CardTag, Contract,
    ContractFailureReason, ContractRequirementKind, ContractResolution, ContractTier,
    PlayerActionCard, TierContracts, TokenAmount, TokenType,
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
        contract_resolution: Option<ContractResolution>,
    },
    CardDiscarded {
        #[serde(skip_serializing_if = "Option::is_none")]
        contract_resolution: Option<ContractResolution>,
    },
    /// A deck/discard card was replaced with a shelved card; sacrifice destroyed.
    CardReplaced,
    /// The active contract was voluntarily abandoned after the required minimum turns.
    ///
    /// The `contract_resolution` is always a `Failed` resolution with reason `Abandoned`.
    ContractAbandoned {
        contract_resolution: ContractResolution,
    },
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
    InvalidCardIndex {
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
    /// The card's tag is banned or limited by an active CardTagConstraint requirement.
    CardTagBanned {
        tag: CardTag,
    },
    /// AbandonContract was attempted but the required minimum turns have not been played yet.
    AbandonContractNotAllowed {
        turns_played: u32,
        turns_required: u32,
    },
    /// AbandonContract was attempted but there is no active contract.
    NoContractToAbandon,
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
    pub contract_turns_played: u32,
    pub offered_contracts: Vec<TierContracts>,
    /// Cards played per tag during the current contract (empty when no contract is active).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards_played_per_tag_contract: Vec<CardTagCount>,
}

/// Lightweight state view for strategy evaluation (no cloning or serialization).
///
/// Used internally by strategies that need full game state inspection without the
/// overhead of cloning all cards/contracts and serializing to JSON. References are
/// borrowed directly from GameState, avoiding allocation overhead during repeated
/// state introspection (e.g., choosing actions in a 100K+ action simulation).
#[derive(Debug)]
pub struct StrategyView<'a> {
    pub seed: u64,
    pub cards: &'a [CardEntry],
    pub tokens: &'a HashMap<TokenType, u32>,
    pub active_contract: &'a Option<Contract>,
    pub contract_turns_played: u32,
    pub offered_contracts: &'a [TierContracts],
    pub cards_played_per_tag_contract: &'a HashMap<CardTag, u32>,
}

/// Token balances grouped by tag category for the `/player/tokens` endpoint.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct PlayerTokensView {
    pub beneficial: Vec<TokenAmount>,
    pub harmful: Vec<TokenAmount>,
    pub progression: Vec<TokenAmount>,
}

/// A count of how many cards with a specific tag have been played in the current contract.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct CardTagCount {
    pub tag: CardTag,
    pub count: u32,
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
    /// Lists the specific card indices (into the /state cards Vec) that the player
    /// can play given active CardTagConstraint limits and hand availability.
    PlayCard {
        valid_card_indices: Vec<usize>,
    },
    /// Lists the card indices (into the /state cards Vec) that have hand > 0.
    DiscardCard {
        valid_card_indices: Vec<usize>,
    },
    AcceptContract {
        valid_tiers: Vec<TierContractRange>,
    },
    ReplaceCard,
    /// Available when an active contract has been running for at least
    /// `min_turns_before_abandon` turns. Abandoning counts as a failure.
    AbandonContract,
}

// ---------------------------------------------------------------------------
// GameState
// ---------------------------------------------------------------------------

/// Core game state for My Little Factory Manager.
///
/// **Simulation tests only**: `GameState` and its methods (`possible_actions`,
/// `view`, `dispatch`) are called directly by `tests/simulation/game_driver`
/// to avoid HTTP overhead in the hot loop.  All other integration tests must
/// interact with the game exclusively through the HTTP API so that the actual
/// endpoints remain exercised.
pub struct GameState {
    // Card management (count-based)
    cards: Vec<CardEntry>,

    // Token balances
    tokens: HashMap<TokenType, u32>,

    // Contract state
    active_contract: Option<Contract>,
    offered_contracts: Vec<TierContracts>,
    contract_turns_played: u32,
    cards_played_per_tag_contract: HashMap<CardTag, u32>,

    // RNG and metadata
    rng: Pcg64,
    seed: u64,

    // All config-derived data, pre-computed once at game creation
    cached_config: CachedConfig,

    // Action log
    action_log: ActionLog,

    // Gameplay statistics
    metrics_tracker: MetricsTracker,

    // Adaptive balance
    adaptive_tracker: AdaptiveBalanceTracker,
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

        let token_defs = load_token_definitions().expect("embedded token definitions must parse");
        let cached_config = build_cached_config(rules, token_defs);

        let mut cards = create_starter_deck(
            cached_config.rules.general.starting_deck_size,
            &mut rng,
            &cached_config.effect_types,
        );

        // Deal starting hand
        let hand_size = cached_config.rules.general.starting_hand_size;
        for _ in 0..hand_size {
            draw_from_deck(&mut cards, &mut rng);
        }

        let starting_deck_size = cached_config.rules.general.starting_deck_size;
        let adaptive_tracker =
            AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

        let mut state = Self {
            cards,
            tokens: HashMap::new(),
            active_contract: None,
            offered_contracts: Vec::new(),
            contract_turns_played: 0,
            cards_played_per_tag_contract: HashMap::new(),
            rng,
            seed: actual_seed,
            cached_config,
            action_log: ActionLog::new(),
            metrics_tracker: MetricsTracker::new(),
            adaptive_tracker,
        };

        // Initialize deck slots to starting deck size
        state.add_tokens(&TokenType::DeckSlots, starting_deck_size);

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
            contract_turns_played: self.contract_turns_played,
            offered_contracts: self.offered_contracts.clone(),
            cards_played_per_tag_contract: {
                let mut v: Vec<_> = self
                    .cards_played_per_tag_contract
                    .iter()
                    .map(|(tag, &count)| CardTagCount {
                        tag: tag.clone(),
                        count,
                    })
                    .collect();
                v.sort_by(|a, b| format!("{:?}", a.tag).cmp(&format!("{:?}", b.tag)));
                v
            },
        }
    }

    /// Fast-path state view for strategy evaluation (zero allocations, borrowed references).
    ///
    /// Used by strategies that need to inspect game state for decision-making without
    /// the overhead of cloning all state data and serializing to JSON. Returns borrowed
    /// references directly from GameState's internal storage.
    ///
    /// This is suitable for internal strategy evaluation during simulations. For HTTP API
    /// responses, use `view()` which returns the full JSON-serializable snapshot.
    pub fn view_for_scoring(&self) -> StrategyView<'_> {
        StrategyView {
            seed: self.seed,
            cards: &self.cards,
            tokens: &self.tokens,
            active_contract: &self.active_contract,
            contract_turns_played: self.contract_turns_played,
            offered_contracts: &self.offered_contracts,
            cards_played_per_tag_contract: &self.cards_played_per_tag_contract,
        }
    }

    pub fn action_log(&self) -> &ActionLog {
        &self.action_log
    }

    pub fn session_metrics(&self) -> SessionMetrics {
        let mut metrics = self.metrics_tracker.compute_session_metrics();
        metrics.adaptive_pressure = self.adaptive_tracker.pressure_snapshot();
        metrics
    }

    pub fn adaptive_pressure(&self) -> Vec<crate::adaptive_balance::TokenPressure> {
        self.adaptive_tracker.pressure_snapshot()
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
            if self.cards.iter().any(|e| e.counts.hand > 0) {
                let valid_play_indices = self.playable_card_indices();
                if !valid_play_indices.is_empty() {
                    actions.push(PossibleAction::PlayCard {
                        valid_card_indices: valid_play_indices,
                    });
                }
                let discard_indices: Vec<usize> = self
                    .cards
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.counts.hand > 0)
                    .map(|(i, _)| i)
                    .collect();
                if !discard_indices.is_empty() {
                    actions.push(PossibleAction::DiscardCard {
                        valid_card_indices: discard_indices,
                    });
                }
            }

            // AbandonContract becomes available after the minimum turns threshold
            if self.contract_turns_played
                >= self.cached_config.rules.general.min_turns_before_abandon
            {
                actions.push(PossibleAction::AbandonContract);
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

    /// Adds ReplaceCard action if it's possible.
    fn add_replace_card_action(&self, actions: &mut Vec<PossibleAction>) {
        // ReplaceCard is possible if there are cards with shelved copies (for replacement and sacrifice)
        // and cards with copies in deck or discard (for target).
        let has_shelved = self.cards.iter().any(|e| e.counts.has_shelved());
        let has_target = self
            .cards
            .iter()
            .any(|e| e.counts.deck > 0 || e.counts.discard > 0);

        if has_shelved && has_target {
            actions.push(PossibleAction::ReplaceCard);
        }
    }

    /// Returns the set of card indices (into the cards Vec) that may currently be played.
    /// A card is playable if it has hand > 0 and its tags are not banned or over the
    /// active CardTagConstraint max limit.
    fn playable_card_indices(&self) -> Vec<usize> {
        let contract = match &self.active_contract {
            Some(c) => c,
            None => {
                return self
                    .cards
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.counts.hand > 0)
                    .map(|(i, _)| i)
                    .collect();
            }
        };

        // Collect tag constraints (max-bound only) for fast lookup
        let mut banned_or_limited: HashMap<CardTag, u32> = HashMap::new();
        for req in &contract.requirements {
            if let ContractRequirementKind::CardTagConstraint {
                tag,
                max: Some(max),
                ..
            } = req
            {
                // Keep the tightest limit per tag
                let entry = banned_or_limited.entry(tag.clone()).or_insert(u32::MAX);
                *entry = (*entry).min(*max);
            }
        }

        self.cards
            .iter()
            .enumerate()
            .filter(|(_, e)| e.counts.hand > 0)
            .filter(|(_, e)| {
                for tag in &e.card.tags {
                    if let Some(&limit) = banned_or_limited.get(tag) {
                        let played = self
                            .cards_played_per_tag_contract
                            .get(tag)
                            .copied()
                            .unwrap_or(0);
                        if played >= limit {
                            return false;
                        }
                    }
                }
                true
            })
            .map(|(i, _)| i)
            .collect()
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
            PlayerAction::PlayCard { card_index } => self.handle_play_card(card_index),
            PlayerAction::DiscardCard { card_index } => self.handle_discard_card(card_index),
            PlayerAction::ReplaceCard {
                target_card_index,
                replacement_card_index,
                sacrifice_card_index,
            } => self.handle_replace_card(
                target_card_index,
                replacement_card_index,
                sacrifice_card_index,
            ),
            PlayerAction::AbandonContract => self.handle_abandon_contract(),
        }
    }

    // -------------------------------------------------------------------
    // Action handlers
    // -------------------------------------------------------------------

    fn handle_new_game(&mut self, seed: Option<u64>) -> ActionResult {
        let new_state = Self::new_with_rules(seed, self.cached_config.rules.clone());
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
        self.metrics_tracker
            .record_contract_accepted(contract.tier.0);
        self.contract_turns_played = 0;
        self.cards_played_per_tag_contract.clear();
        self.active_contract = Some(contract);
        ActionResult::Success(ActionSuccess::ContractAccepted)
    }

    fn handle_play_card(&mut self, card_index: usize) -> ActionResult {
        if self.active_contract.is_none() {
            return ActionResult::Error(ActionError::NoActiveContract);
        }

        if card_index >= self.cards.len() || self.cards[card_index].counts.hand == 0 {
            return ActionResult::Error(ActionError::InvalidCardIndex { index: card_index });
        }

        let card = self.cards[card_index].card.clone();

        // Check CardTagConstraint bans / limits before applying the card
        if let Some(contract) = &self.active_contract {
            for tag in &card.tags {
                for req in &contract.requirements {
                    if let ContractRequirementKind::CardTagConstraint {
                        tag: req_tag,
                        max: Some(limit),
                        ..
                    } = req
                    {
                        if req_tag == tag {
                            let played = self
                                .cards_played_per_tag_contract
                                .get(tag)
                                .copied()
                                .unwrap_or(0);
                            if played >= *limit {
                                return ActionResult::Error(ActionError::CardTagBanned {
                                    tag: tag.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        let missing = self.get_missing_tokens_for_effects(&card.effects);
        if !missing.is_empty() {
            return ActionResult::Error(ActionError::InsufficientTokens { missing });
        }

        self.apply_effects(&card.effects);

        // Track per-tag plays for CardTagConstraint enforcement
        for tag in &card.tags {
            *self
                .cards_played_per_tag_contract
                .entry(tag.clone())
                .or_insert(0) += 1;
        }

        // Record metrics: tag counts and token flow from effects
        let (produced, consumed) = collect_effect_token_flow(&card.effects);
        self.metrics_tracker
            .record_card_played(&card.tags, &produced, &consumed);

        // Record gross production for adaptive balance
        for (token_type, amount) in &produced {
            self.adaptive_tracker
                .record_token_produced(token_type, *amount);
        }

        self.cards[card_index].counts.hand -= 1;
        self.cards[card_index].counts.discard += 1;

        self.contract_turns_played += 1;

        draw_from_deck(&mut self.cards, &mut self.rng);

        let contract_resolution = self.try_resolve_contract();
        ActionResult::Success(ActionSuccess::CardPlayed {
            contract_resolution,
        })
    }

    fn handle_discard_card(&mut self, card_index: usize) -> ActionResult {
        if self.active_contract.is_none() {
            return ActionResult::Error(ActionError::NoActiveContract);
        }

        if card_index >= self.cards.len() || self.cards[card_index].counts.hand == 0 {
            return ActionResult::Error(ActionError::InvalidCardIndex { index: card_index });
        }

        self.cards[card_index].counts.hand -= 1;
        self.cards[card_index].counts.discard += 1;

        let bonus = self
            .cached_config
            .rules
            .general
            .discard_production_unit_bonus;
        *self.tokens.entry(TokenType::ProductionUnit).or_insert(0) += bonus;

        self.metrics_tracker
            .record_card_discarded(&[(TokenType::ProductionUnit, bonus)]);

        // Record gross production for adaptive balance
        self.adaptive_tracker
            .record_token_produced(&TokenType::ProductionUnit, bonus);

        self.contract_turns_played += 1;

        draw_from_deck(&mut self.cards, &mut self.rng);

        let contract_resolution = self.try_resolve_contract();
        ActionResult::Success(ActionSuccess::CardDiscarded {
            contract_resolution,
        })
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

        self.metrics_tracker.record_card_replaced();

        ActionResult::Success(ActionSuccess::CardReplaced)
    }

    fn handle_abandon_contract(&mut self) -> ActionResult {
        let contract = match self.active_contract.as_ref() {
            Some(c) => c.clone(),
            None => return ActionResult::Error(ActionError::NoContractToAbandon),
        };

        let turns_required = self.cached_config.rules.general.min_turns_before_abandon;
        if self.contract_turns_played < turns_required {
            return ActionResult::Error(ActionError::AbandonContractNotAllowed {
                turns_played: self.contract_turns_played,
                turns_required,
            });
        }

        let turns_played = self.contract_turns_played;
        self.metrics_tracker
            .record_contract_abandoned(contract.tier.0);
        self.adaptive_tracker.on_contract_failed();
        self.active_contract = None;
        self.contract_turns_played = 0;
        self.cards_played_per_tag_contract.clear();
        self.refill_contract_market();

        let reason = ContractFailureReason::Abandoned { turns_played };
        let contract_resolution = ContractResolution::Failed { contract, reason };
        ActionResult::Success(ActionSuccess::ContractAbandoned {
            contract_resolution,
        })
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
        let target = self
            .cached_config
            .rules
            .general
            .contract_market_size_per_tier;

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
                .map(|_| {
                    generate_contract_with_types(
                        tier,
                        &mut self.rng,
                        &self.cached_config,
                        &self.adaptive_tracker,
                    )
                })
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
        let threshold = self
            .cached_config
            .rules
            .general
            .contracts_per_tier_to_advance;
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

    /// Check for contract failure (first) then completion. Failure takes precedence.
    fn try_resolve_contract(&mut self) -> Option<ContractResolution> {
        let contract = self.active_contract.as_ref()?.clone();

        // 1. Check failure conditions (failure-first ordering)
        if let Some(reason) = self.check_contract_failure(&contract) {
            self.metrics_tracker.record_contract_failed(contract.tier.0);
            self.adaptive_tracker.on_contract_failed();
            self.active_contract = None;
            self.contract_turns_played = 0;
            self.cards_played_per_tag_contract.clear();
            self.refill_contract_market();
            return Some(ContractResolution::Failed { contract, reason });
        }

        // 2. Check completion
        if !self.all_requirements_met(&contract) {
            return None;
        }

        self.subtract_contract_tokens(&contract);
        self.add_tokens(&TokenType::ContractsTierCompleted(contract.tier.0), 1);
        self.metrics_tracker
            .record_contract_completed(contract.tier.0);
        self.adaptive_tracker.on_contract_completed();

        self.add_reward_card(&contract.reward_card);

        self.active_contract = None;
        self.contract_turns_played = 0;
        self.cards_played_per_tag_contract.clear();
        self.refill_contract_market();

        Some(ContractResolution::Completed { contract })
    }

    /// Check all failure conditions on the active contract. Returns the first
    /// violation found (deterministic order: harmful limits by token sort order,
    /// then turn window).
    fn check_contract_failure(&self, contract: &Contract) -> Option<ContractFailureReason> {
        // Check HarmfulTokenLimit violations (sorted by TokenType for determinism)
        let (_, harmful_limits) = Self::aggregate_requirements(&contract.requirements);
        let mut sorted_limits: Vec<_> = harmful_limits.into_iter().collect();
        sorted_limits.sort_by(|a, b| a.0.cmp(&b.0));

        for (token_type, max_amount) in sorted_limits {
            let current = self.tokens.get(&token_type).copied().unwrap_or(0);
            if current > max_amount {
                return Some(ContractFailureReason::HarmfulTokenLimitExceeded {
                    token_type,
                    max_amount,
                    current_amount: current,
                });
            }
        }

        // Check TurnWindow max_turn
        for req in &contract.requirements {
            if let ContractRequirementKind::TurnWindow {
                max_turn: Some(max),
                ..
            } = req
            {
                if self.contract_turns_played > *max {
                    return Some(ContractFailureReason::TurnWindowExceeded {
                        max_turn: *max,
                        current_turn: self.contract_turns_played,
                    });
                }
            }
        }

        None
    }

    fn add_reward_card(&mut self, card: &PlayerActionCard) {
        add_card_to_entries(&mut self.cards, card, CardLocation::Shelved);
    }

    fn all_requirements_met(&self, contract: &Contract) -> bool {
        let (output_thresholds, harmful_limits) =
            Self::aggregate_requirements(&contract.requirements);

        for (token_type, total_min) in &output_thresholds {
            if self.tokens.get(token_type).copied().unwrap_or(0) < *total_min {
                return false;
            }
        }

        for (token_type, tightest_max) in &harmful_limits {
            if self.tokens.get(token_type).copied().unwrap_or(0) > *tightest_max {
                return false;
            }
        }

        // TurnWindow min_turn: prevent premature completion
        for req in &contract.requirements {
            if let ContractRequirementKind::TurnWindow {
                min_turn: Some(min),
                ..
            } = req
            {
                if self.contract_turns_played < *min {
                    return false;
                }
            }
        }

        // CardTagConstraint min: must have played enough cards of this tag
        for req in &contract.requirements {
            if let ContractRequirementKind::CardTagConstraint {
                tag,
                min: Some(required),
                ..
            } = req
            {
                let played = self
                    .cards_played_per_tag_contract
                    .get(tag)
                    .copied()
                    .unwrap_or(0);
                if played < *required {
                    return false;
                }
            }
        }

        true
    }

    fn aggregate_requirements(
        requirements: &[ContractRequirementKind],
    ) -> (
        std::collections::HashMap<TokenType, u32>,
        std::collections::HashMap<TokenType, u32>,
    ) {
        let mut output_thresholds: std::collections::HashMap<TokenType, u32> =
            std::collections::HashMap::new();
        let mut harmful_limits: std::collections::HashMap<TokenType, u32> =
            std::collections::HashMap::new();

        for req in requirements {
            match req {
                ContractRequirementKind::TokenRequirement {
                    token_type,
                    min,
                    max,
                } => {
                    if let Some(min_amount) = min {
                        *output_thresholds.entry(token_type.clone()).or_insert(0) += min_amount;
                    }
                    if let Some(max_amount) = max {
                        let entry = harmful_limits.entry(token_type.clone()).or_insert(u32::MAX);
                        *entry = (*entry).min(*max_amount);
                    }
                }
                ContractRequirementKind::CardTagConstraint { .. }
                | ContractRequirementKind::TurnWindow { .. } => {}
            }
        }

        (output_thresholds, harmful_limits)
    }

    fn subtract_contract_tokens(&mut self, contract: &Contract) {
        let (output_thresholds, _) = Self::aggregate_requirements(&contract.requirements);
        for (token_type, total_min) in &output_thresholds {
            self.remove_tokens(token_type, *total_min);
        }
    }
}

// ---------------------------------------------------------------------------
// Card helper functions (free functions operating on Vec<CardEntry>)
// ---------------------------------------------------------------------------

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

/// Token flow pairs: (token_type, amount) for each produced/consumed token.
type TokenFlowPairs = (Vec<(TokenType, u32)>, Vec<(TokenType, u32)>);

/// Extract token inputs and outputs from a list of card effects
/// as flat `(TokenType, amount)` pairs for metrics tracking.
fn collect_effect_token_flow(effects: &[CardEffect]) -> TokenFlowPairs {
    let mut produced = Vec::new();
    let mut consumed = Vec::new();
    for effect in effects {
        for output in &effect.outputs {
            produced.push((output.token_type.clone(), output.amount));
        }
        for input in &effect.inputs {
            consumed.push((input.token_type.clone(), input.amount));
        }
    }
    (produced, consumed)
}
