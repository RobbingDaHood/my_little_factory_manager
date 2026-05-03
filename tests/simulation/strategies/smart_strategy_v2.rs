//! SmartStrategyV2 — success-probability-based contract acceptance.
//!
//! ## Why a V2
//!
//! The V1 SmartStrategy scores contracts by combining a tier weight, a soft
//! infeasibility penalty, an advancement bonus, and a bracket-based
//! `comfort_tier` overreach penalty. In practice the bracket-based comfort
//! metric is too coarse: it jumps in steps of 4 unlock tiers (0 → 4 → 16 → 36)
//! and treats every tier-25 contract as identically risky regardless of how
//! the contract's actual requirements interact with the player's deck and
//! current hand. As issue #115 documents, this either lets the strategy
//! over-stretch into reliable failures (when comfort is high but the specific
//! contract is hard) or refuse a "lucky" easy high-tier contract whose reward
//! card would jump-start a bracket transition.
//!
//! ## Approach
//!
//! V2 replaces the per-contract score with a multiplicative model:
//!
//!     score = success_probability × (BASE_COMPLETION_VALUE + reward_value)
//!
//! `success_probability` is computed per-contract from the actual game state:
//!   - Current token balances (already accumulated progress).
//!   - Hand contribution: sum of net production from cards currently in hand
//!     (cards that are guaranteed playable, not just shuffled deep in the deck).
//!   - Deck mean production rate per token (median-of-top-half, reused from V1).
//!   - Turn budget from the contract's `TurnWindow.max_turn`.
//!   - Harmful-token overflow trajectory.
//!
//! Each requirement contributes a per-token probability via a sigmoid on the
//! ratio of expected progress to required progress. Probabilities are combined
//! multiplicatively across requirements (independence assumption — close
//! enough for ranking).
//!
//! `reward_value` reuses V1's card quality and tag/token diversity bonuses, so
//! a tier-30 contract whose reward card is a strong producer scores higher
//! than a tier-3 contract with a weak reward — but only when the success
//! probability is still meaningful. A tier-30 contract with success prob 0.05
//! is dominated by a tier-3 contract with success prob 0.95.
//!
//! ## Architecture
//!
//! V2 is a thin wrapper around V1. It owns a `SmartStrategy` and delegates all
//! action selection to it, *intercepting only* the case where V1's chosen
//! action is `AcceptContract` — at that point V2 substitutes its own
//! probability-based contract choice. All other behaviour (deckbuilding, play
//! card scoring, discard selection, abandon thresholds, livelock detection,
//! adaptive tier reduction) is reused unchanged from V1.
//!
//! This keeps the V2 diff small and ensures V1 remains usable independently —
//! the two strategies can be benchmarked head-to-head.

use std::collections::HashMap;

use my_little_factory_manager::action_log::PlayerAction;
use my_little_factory_manager::game_state::{PossibleAction, StrategyView, TierContractRange};
use my_little_factory_manager::types::{
    CardEntry, Contract, ContractRequirementKind, PlayerActionCard, TokenType,
};

use crate::game_driver::GameSnapshot;
use crate::strategies::smart_strategy::SmartStrategy;
use crate::strategies::Strategy;

/// Default turns budget for contracts with no explicit `max_turn` deadline.
/// Long enough that the deck cycle dominates, short enough that we don't
/// claim a no-deadline contract is "always feasible" regardless of producer count.
const DEFAULT_TURNS_BUDGET: f64 = 200.0;

/// Sigmoid sharpness for token-min requirements (turns budget vs required turns).
const MIN_REQ_SHARPNESS: f64 = 4.0;

/// Sigmoid sharpness for token-max requirements (cap vs projected accumulation).
/// Sharper than min — overflow is binary in the game (instant fail), so we want
/// the probability to drop quickly once the projected balance approaches the cap.
const MAX_REQ_SHARPNESS: f64 = 6.0;

/// Sigmoid sharpness for tag-cap requirements.
const TAG_CAP_SHARPNESS: f64 = 8.0;

/// Floor on per-requirement probability before multiplication. Prevents one
/// uncertain requirement from collapsing the whole product to zero — we still
/// want to compare contracts where every option is risky.
const PER_REQUIREMENT_FLOOR: f64 = 0.02;

/// Constant value of completing any contract, on top of its reward value.
/// Captures the stall-budget reset and the tier-progress ratchet that
/// `reward_value` doesn't directly model. Tuned so a low-reward contract with
/// high success prob still ranks above a high-reward contract whose success
/// prob is below ~0.2 — which matches the issue #115 preference for reliable
/// progression over volatile peaks.
const BASE_COMPLETION_VALUE: f64 = 50.0;

/// Weight applied to the reward card's general quality (V1 metric).
const REWARD_QUALITY_WEIGHT: f64 = 1.0;

/// Weight applied to the V1 tag/token diversity bonuses.
const REWARD_DIVERSITY_WEIGHT: f64 = 0.5;

/// Per-token "option value" bonus when the contract's reward card produces a
/// token the deck currently has no producer for, AND the contract requires
/// that same token (so completing it bootstraps the producer pipeline).
///
/// Critically, this bonus is **additive to the score**, not multiplied by
/// `success_probability`. Empirically, multiplying by success_prob led to
/// permanent stalls because the bootstrap contract's success_prob is near
/// zero (no producer of the missing token yet) — so the bracket-jump value
/// got crushed to a tiny number and trivial low-tier contracts always won.
///
/// Treating it as additive captures the option value of *attempting* a
/// bootstrap contract: even with a 5% chance of completion, the strategy
/// should pay something to take the shot at acquiring its first Energy /
/// QualityPoint / Innovation producer.
const BRACKET_JUMP_OPTION_VALUE: f64 = 35.0;

/// Multiplier on contract value per tier of the contract.
/// At TIER_SCALE = 0.1, tier-30 contracts are worth ~4× tier-3 contracts
/// (per unit of reward) — captures "higher-tier contracts give better
/// reward cards".
const TIER_SCALE: f64 = 0.1;

/// Per-tier discount applied to contracts whose tier is *below* the highest
/// tier available in the current offer pool. The simulation runner stalls
/// after 1000 contracts without a strictly-higher tier completion
/// (game_driver.rs), so completing a contract below the tier-progress
/// frontier does not advance the ratchet — it just consumes stall budget.
/// Without this discount, V2 happily grinds tier-3 contracts forever
/// (verified empirically: avg max_tier 3.6 even with TIER_SCALE applied).
///
/// Geometric discount: a tier-(frontier-2) contract is discounted by 0.2^2
/// = 0.04. Tuned aggressive because empirical tests showed 0.4 was still
/// too gentle for some seeds — when frontier tier contracts had no
/// bracket-jump option value (no missing-token reward), tier-3 contracts
/// dominated and the strategy never advanced past the lowest tier. The
/// strategy still drops below the frontier when frontier contracts have
/// catastrophically low success_prob (the safe choice at 0.95 prob still
/// beats the frontier at 0.1 prob with no option value).
const BELOW_FRONTIER_DISCOUNT: f64 = 0.2;

/// Exponent applied to `success_probability` before multiplying by value.
/// 1.0 = linear (weight prob and value equally). >1 sharpens preference
/// for high-probability contracts; <1 lets value dominate. Tuned 1.0 so
/// that the frontier discount drives tier progression: a tier-3 contract
/// at 0.95 prob shouldn't dominate a tier-4 contract at 0.5 prob purely
/// through probability.
const SUCCESS_PROB_EXPONENT: f64 = 1.0;

pub struct SmartStrategyV2 {
    inner: SmartStrategy,
}

impl SmartStrategyV2 {
    pub fn new() -> Self {
        Self {
            inner: SmartStrategy::new(),
        }
    }

    fn sigmoid(x: f64) -> f64 {
        1.0 / (1.0 + (-x).exp())
    }

    /// Per-requirement success probability for a `TokenRequirement` with a `min` bound.
    /// Models the chain: current balance → hand contribution (immediate) → deck cycling.
    fn min_token_probability(
        token_type: &TokenType,
        min_required: f64,
        current_balance: f64,
        cards: &[CardEntry],
        turns_left: f64,
    ) -> f64 {
        let hand_contrib = SmartStrategy::hand_contribution(cards, token_type);
        let needed_after_hand = min_required - current_balance - hand_contrib;
        if needed_after_hand <= 0.0 {
            return 0.99;
        }
        let deck_mean = SmartStrategy::deck_effective_production(cards, token_type);
        if deck_mean <= 0.0 {
            return PER_REQUIREMENT_FLOOR;
        }
        let turns_needed = needed_after_hand / deck_mean;
        let turns_budget = turns_left.max(0.0);
        if turns_needed <= 0.0 {
            return 0.99;
        }
        let ratio = turns_budget / turns_needed;
        Self::sigmoid((ratio - 1.0) * MIN_REQ_SHARPNESS)
    }

    /// Per-requirement success probability for a `TokenRequirement` with a `max` bound.
    /// Models harmful-token overflow risk over the contract's turn budget.
    fn max_token_probability(
        token_type: &TokenType,
        max_allowed: f64,
        current_balance: f64,
        cards: &[CardEntry],
        turns_left: f64,
    ) -> f64 {
        if current_balance > max_allowed {
            return 0.0;
        }
        let deck_mean = SmartStrategy::deck_effective_production(cards, token_type);
        if deck_mean <= 0.0 {
            return 0.99;
        }
        let projected = current_balance + deck_mean * turns_left;
        if projected <= max_allowed {
            return 0.95;
        }
        let ratio = max_allowed / projected.max(1.0);
        Self::sigmoid((ratio - 1.0) * MAX_REQ_SHARPNESS)
    }

    /// Per-requirement success probability for a `CardTagConstraint` with a `max` bound.
    /// Hard to model precisely — depends on play-time card selection. Approximate using
    /// the fraction of the deck that carries the tag.
    fn tag_max_probability(
        tag: &my_little_factory_manager::types::CardTag,
        max_allowed: u32,
        cards: &[CardEntry],
    ) -> f64 {
        let cycle = SmartStrategy::deck_cycle_size(cards);
        if cycle <= 0.0 {
            return 0.5;
        }
        let tagged = SmartStrategy::deck_tag_count(cards, tag);
        let tagged_frac = tagged / cycle;
        // Heuristic allowed_frac: assume the strategy plays ~30 cards per contract,
        // so max_allowed cards tagged corresponds to allowed_frac ≈ max_allowed / 30.
        let allowed_frac = (max_allowed as f64 / 30.0).clamp(0.0, 1.0);
        if tagged_frac <= allowed_frac {
            return 0.9;
        }
        Self::sigmoid((allowed_frac - tagged_frac) * TAG_CAP_SHARPNESS)
    }

    /// Combine per-requirement probabilities multiplicatively (independence assumption).
    /// Each requirement is floored at `PER_REQUIREMENT_FLOOR` to avoid a single uncertain
    /// requirement zeroing out the entire score — we still need to rank "all bad" menus.
    fn success_probability(
        contract: &Contract,
        cards: &[CardEntry],
        token_balances: &HashMap<TokenType, i64>,
    ) -> f64 {
        let turns_left: f64 = contract
            .requirements
            .iter()
            .find_map(|req| {
                if let ContractRequirementKind::TurnWindow {
                    max_turn: Some(m), ..
                } = req
                {
                    Some(*m as f64)
                } else {
                    None
                }
            })
            .unwrap_or(DEFAULT_TURNS_BUDGET);

        let mut prob = 1.0;
        for req in &contract.requirements {
            let p = match req {
                ContractRequirementKind::TokenRequirement {
                    token_type,
                    min,
                    max,
                } => {
                    let current = *token_balances.get(token_type).unwrap_or(&0) as f64;
                    let mut combined = 1.0;
                    if let Some(min_v) = min {
                        combined *= Self::min_token_probability(
                            token_type,
                            *min_v as f64,
                            current,
                            cards,
                            turns_left,
                        );
                    }
                    if let Some(max_v) = max {
                        combined *= Self::max_token_probability(
                            token_type,
                            *max_v as f64,
                            current,
                            cards,
                            turns_left,
                        );
                    }
                    combined
                }
                ContractRequirementKind::CardTagConstraint { tag, max, .. } => {
                    if let Some(max_v) = max {
                        Self::tag_max_probability(tag, *max_v, cards)
                    } else {
                        1.0
                    }
                }
                ContractRequirementKind::TurnWindow { .. } => 1.0,
            };
            prob *= p.max(PER_REQUIREMENT_FLOOR);
        }
        prob.clamp(0.0, 1.0)
    }

    /// Reward value of a contract — what we get if we complete it. Reuses V1's
    /// `card_general_quality` and diversity bonuses. Multiplied by success_prob
    /// in `score_contract`, so this represents conditional-on-completion value.
    fn reward_value(reward_card: &PlayerActionCard, cards: &[CardEntry]) -> f64 {
        let quality = SmartStrategy::card_general_quality(reward_card);
        let tag_bonus = SmartStrategy::tag_diversity_bonus(reward_card, cards);
        let token_bonus = SmartStrategy::token_diversity_bonus(reward_card, cards);
        quality * REWARD_QUALITY_WEIGHT + (tag_bonus + token_bonus) * REWARD_DIVERSITY_WEIGHT
    }

    /// Additive option-value bonus for contracts that could bootstrap a missing
    /// producer. Returns BRACKET_JUMP_OPTION_VALUE for each beneficial token where
    /// the contract's reward card produces it AND the deck currently has no
    /// producer — i.e., completing this contract would acquire the player's first
    /// Energy/QualityPoint/Innovation producer. Independent of success_prob so the
    /// strategy will take longshot bootstrap contracts when they're the only path
    /// to the next bracket.
    fn bracket_jump_option_value(reward_card: &PlayerActionCard, cards: &[CardEntry]) -> f64 {
        [
            TokenType::ProductionUnit,
            TokenType::Energy,
            TokenType::QualityPoint,
            TokenType::Innovation,
        ]
        .iter()
        .map(|t| {
            let card_makes = SmartStrategy::card_net_production(reward_card, t);
            if card_makes <= 0.0 {
                return 0.0;
            }
            let deck_makes = SmartStrategy::deck_effective_production(cards, t);
            if deck_makes > 0.0 {
                0.0
            } else {
                BRACKET_JUMP_OPTION_VALUE
            }
        })
        .sum()
    }

    fn score_contract(
        contract: &Contract,
        cards: &[CardEntry],
        token_balances: &HashMap<TokenType, i64>,
        offer_frontier_tier: u32,
    ) -> f64 {
        let success_prob = Self::success_probability(contract, cards, token_balances);
        let reward = Self::reward_value(&contract.reward_card, cards);
        let tier_factor = 1.0 + contract.tier.0 as f64 * TIER_SCALE;
        let below_frontier = offer_frontier_tier.saturating_sub(contract.tier.0) as f64;
        let frontier_factor = BELOW_FRONTIER_DISCOUNT.powf(below_frontier);
        let value = (BASE_COMPLETION_VALUE + reward) * tier_factor * frontier_factor;
        let realised_value = success_prob.powf(SUCCESS_PROB_EXPONENT) * value;

        // Option value of a bootstrap contract — only applied at the offer frontier
        // (bracket-jumping below the frontier doesn't help advance the tier ratchet).
        let option_value = if contract.tier.0 >= offer_frontier_tier {
            Self::bracket_jump_option_value(&contract.reward_card, cards)
        } else {
            0.0
        };

        realised_value + option_value
    }

    fn choose_accept_contract(
        valid_tiers: &[TierContractRange],
        state: &StrategyView,
        token_balances: &HashMap<TokenType, i64>,
    ) -> Option<PlayerAction> {
        let offered = state.offered_contracts;

        // Frontier tier = highest contract tier currently being offered. Used as a
        // proxy for "the tier we should be working at to ratchet the stall counter".
        let frontier_tier: u32 = valid_tiers
            .iter()
            .filter_map(|r| offered.get(r.tier_index).map(|tc| tc.tier.0))
            .max()
            .unwrap_or(0);

        let mut best: Option<(usize, usize, f64)> = None;
        for tier_range in valid_tiers {
            let tier_idx = tier_range.tier_index;
            let min_c = tier_range.valid_contract_index_range.min;
            let max_c = tier_range.valid_contract_index_range.max;
            if let Some(tier_contracts) = offered.get(tier_idx) {
                for c_idx in min_c..=max_c {
                    if let Some(contract) = tier_contracts.contracts.get(c_idx) {
                        let score = Self::score_contract(
                            contract,
                            state.cards,
                            token_balances,
                            frontier_tier,
                        );
                        if best.is_none_or(|(_, _, prev)| score > prev) {
                            best = Some((tier_idx, c_idx, score));
                        }
                    }
                }
            }
        }

        best.map(|(t, c, _)| PlayerAction::AcceptContract {
            tier_index: t,
            contract_index: c,
        })
    }
}

impl Default for SmartStrategyV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for SmartStrategyV2 {
    fn name(&self) -> &str {
        "smart_v2"
    }

    fn needs_state(&self) -> bool {
        true
    }

    fn choose_action(
        &self,
        possible_actions: &[PossibleAction],
        snapshot: &GameSnapshot,
    ) -> PlayerAction {
        // Delegate to V1 for everything (deckbuild, play, discard, abandon, livelock,
        // adaptive bookkeeping). V1's internal state mutations (consecutive_discards,
        // outcome window, etc.) stay consistent because we either accept V1's action
        // verbatim or substitute a *different* `AcceptContract` for V1's chosen
        // `AcceptContract`. V1 has already done the "we're accepting a contract"
        // bookkeeping (consecutive_discards reset) by the time it returns.
        let v1_action = self.inner.choose_action(possible_actions, snapshot);
        if !matches!(v1_action, PlayerAction::AcceptContract { .. }) {
            return v1_action;
        }
        let state = snapshot
            .state
            .as_ref()
            .expect("SmartStrategyV2 requires state");
        let token_balances = SmartStrategy::token_balances(state);
        let valid_tiers = match possible_actions.iter().find_map(|a| match a {
            PossibleAction::AcceptContract { valid_tiers } => Some(valid_tiers.as_slice()),
            _ => None,
        }) {
            Some(t) => t,
            None => return v1_action,
        };
        Self::choose_accept_contract(valid_tiers, state, &token_balances).unwrap_or(v1_action)
    }
}
