//! Gameplay statistics tracking and reporting.
//!
//! `MetricsTracker` accumulates live counters during gameplay.
//! `SessionMetrics` is the serializable response for `GET /metrics`.

use std::collections::HashMap;

use rocket::serde::Serialize;
use schemars::JsonSchema;

use crate::types::{CardTag, TokenType};

// ---------------------------------------------------------------------------
// Live tracker (internal, updated during gameplay)
// ---------------------------------------------------------------------------

/// Accumulates gameplay counters during a session. Resets on `NewGame`.
#[derive(Debug, Clone)]
pub struct MetricsTracker {
    // Contract counters
    contracts_completed: u32,
    contracts_completed_per_tier: HashMap<u32, u32>,
    contracts_failed: u32,
    contracts_failed_per_tier: HashMap<u32, u32>,
    contracts_attempted_per_tier: HashMap<u32, u32>,
    contracts_abandoned: u32,
    contracts_abandoned_per_tier: HashMap<u32, u32>,

    // Card counters
    total_cards_played: u32,
    total_cards_discarded: u32,
    cards_played_per_tag: HashMap<CardTag, u32>,

    // Token flow
    tokens_produced: HashMap<TokenType, u32>,
    tokens_consumed: HashMap<TokenType, u32>,

    // Efficiency — per-contract card tracking
    cards_in_current_contract: u32,
    total_cards_across_completed: u32,

    // Streaks
    current_streak: u32,
    best_streak: u32,

    // Deckbuilding
    total_cards_replaced: u32,
}

impl MetricsTracker {
    pub fn new() -> Self {
        Self {
            contracts_completed: 0,
            contracts_completed_per_tier: HashMap::new(),
            contracts_failed: 0,
            contracts_failed_per_tier: HashMap::new(),
            contracts_attempted_per_tier: HashMap::new(),
            contracts_abandoned: 0,
            contracts_abandoned_per_tier: HashMap::new(),
            total_cards_played: 0,
            total_cards_discarded: 0,
            cards_played_per_tag: HashMap::new(),
            tokens_produced: HashMap::new(),
            tokens_consumed: HashMap::new(),
            cards_in_current_contract: 0,
            total_cards_across_completed: 0,
            current_streak: 0,
            best_streak: 0,
            total_cards_replaced: 0,
        }
    }

    /// Record a card play, tracking per-tag counts and token flow.
    pub fn record_card_played(
        &mut self,
        tags: &[CardTag],
        produced: &[(TokenType, u32)],
        consumed: &[(TokenType, u32)],
    ) {
        self.total_cards_played += 1;
        self.cards_in_current_contract += 1;
        for tag in tags {
            *self.cards_played_per_tag.entry(tag.clone()).or_insert(0) += 1;
        }
        for (token_type, amount) in produced {
            *self.tokens_produced.entry(token_type.clone()).or_insert(0) += amount;
        }
        for (token_type, amount) in consumed {
            *self.tokens_consumed.entry(token_type.clone()).or_insert(0) += amount;
        }
    }

    /// Record a card discard (baseline production bonus).
    pub fn record_card_discarded(&mut self, produced: &[(TokenType, u32)]) {
        self.total_cards_discarded += 1;
        self.cards_in_current_contract += 1;
        for (token_type, amount) in produced {
            *self.tokens_produced.entry(token_type.clone()).or_insert(0) += amount;
        }
    }

    /// Record a contract acceptance (increment attempts counter).
    pub fn record_contract_accepted(&mut self, tier: u32) {
        *self.contracts_attempted_per_tier.entry(tier).or_insert(0) += 1;
    }

    /// Record a contract completion.
    pub fn record_contract_completed(&mut self, tier: u32) {
        self.contracts_completed += 1;
        *self.contracts_completed_per_tier.entry(tier).or_insert(0) += 1;
        self.total_cards_across_completed += self.cards_in_current_contract;
        self.cards_in_current_contract = 0;
        self.current_streak += 1;
        if self.current_streak > self.best_streak {
            self.best_streak = self.current_streak;
        }
    }

    /// Record a contract failure.
    pub fn record_contract_failed(&mut self, tier: u32) {
        self.contracts_failed += 1;
        *self.contracts_failed_per_tier.entry(tier).or_insert(0) += 1;
        self.cards_in_current_contract = 0;
        self.current_streak = 0;
    }

    /// Record a contract abandonment.
    ///
    /// Abandonment counts as a failure in all failure metrics (calls
    /// `record_contract_failed` internally) and also increments its own
    /// dedicated counter so callers can distinguish voluntary abandons from
    /// mechanical failures.
    pub fn record_contract_abandoned(&mut self, tier: u32) {
        self.record_contract_failed(tier);
        self.contracts_abandoned += 1;
        *self.contracts_abandoned_per_tier.entry(tier).or_insert(0) += 1;
    }

    pub fn record_card_replaced(&mut self) {
        self.total_cards_replaced += 1;
    }

    /// Build the serializable metrics response.
    pub fn compute_session_metrics(&self) -> SessionMetrics {
        let contracts_per_tier = self.build_tier_metrics();
        let cards_per_tag = self.build_tag_counts();
        let token_flow = self.build_token_flow();

        let avg_cards_per_contract = if self.contracts_completed > 0 {
            Some(self.total_cards_across_completed as f64 / self.contracts_completed as f64)
        } else {
            None
        };

        let (dominant_strategy, strategy_diversity_score) =
            compute_strategy_analysis(&self.cards_played_per_tag);

        SessionMetrics {
            total_contracts_completed: self.contracts_completed,
            total_contracts_failed: self.contracts_failed,
            total_contracts_abandoned: self.contracts_abandoned,
            contracts_per_tier,
            total_cards_played: self.total_cards_played,
            total_cards_discarded: self.total_cards_discarded,
            cards_per_tag,
            avg_cards_per_contract,
            token_flow,
            current_streak: self.current_streak,
            best_streak: self.best_streak,
            dominant_strategy,
            strategy_diversity_score,
            total_cards_replaced: self.total_cards_replaced,
            adaptive_pressure: Vec::new(),
        }
    }

    fn build_tier_metrics(&self) -> Vec<TierCompletionMetrics> {
        let mut all_tiers: Vec<u32> = self
            .contracts_attempted_per_tier
            .keys()
            .chain(self.contracts_completed_per_tier.keys())
            .chain(self.contracts_failed_per_tier.keys())
            .copied()
            .collect();
        all_tiers.sort();
        all_tiers.dedup();

        all_tiers
            .into_iter()
            .map(|tier| {
                let completed = self
                    .contracts_completed_per_tier
                    .get(&tier)
                    .copied()
                    .unwrap_or(0);
                let failed = self
                    .contracts_failed_per_tier
                    .get(&tier)
                    .copied()
                    .unwrap_or(0);
                let attempted = self
                    .contracts_attempted_per_tier
                    .get(&tier)
                    .copied()
                    .unwrap_or(0);
                let completion_rate = if attempted > 0 {
                    completed as f64 / attempted as f64
                } else {
                    0.0
                };
                TierCompletionMetrics {
                    tier,
                    completed,
                    failed,
                    attempted,
                    completion_rate,
                }
            })
            .collect()
    }

    fn build_tag_counts(&self) -> Vec<TagPlayCount> {
        let mut counts: Vec<TagPlayCount> = self
            .cards_played_per_tag
            .iter()
            .map(|(tag, &count)| TagPlayCount {
                tag: tag.clone(),
                count,
            })
            .collect();
        counts.sort_by_key(|a| std::cmp::Reverse(a.count));
        counts
    }

    fn build_token_flow(&self) -> Vec<TokenFlowMetrics> {
        let mut all_types: Vec<&TokenType> = self
            .tokens_produced
            .keys()
            .chain(self.tokens_consumed.keys())
            .collect();
        all_types.sort();
        all_types.dedup();

        all_types
            .into_iter()
            .map(|token_type| {
                let produced = self.tokens_produced.get(token_type).copied().unwrap_or(0);
                let consumed = self.tokens_consumed.get(token_type).copied().unwrap_or(0);
                TokenFlowMetrics {
                    token_type: token_type.clone(),
                    total_produced: produced,
                    total_consumed: consumed,
                    net: produced as i64 - consumed as i64,
                }
            })
            .collect()
    }
}

impl Default for MetricsTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Strategy analysis
// ---------------------------------------------------------------------------

/// Compute dominant strategy tag and normalized Shannon entropy diversity score.
///
/// Diversity score: 0.0 = only one tag used, 1.0 = perfectly even distribution.
/// Returns (None, 0.0) if no cards have been played.
fn compute_strategy_analysis(tag_counts: &HashMap<CardTag, u32>) -> (Option<String>, f64) {
    if tag_counts.is_empty() {
        return (None, 0.0);
    }

    let dominant = tag_counts
        .iter()
        .max_by_key(|(_, &count)| count)
        .map(|(tag, _)| format!("{tag:?}"));

    let total: f64 = tag_counts.values().map(|&c| c as f64).sum();
    if total == 0.0 {
        return (dominant, 0.0);
    }

    let n = tag_counts.len() as f64;
    if n <= 1.0 {
        return (dominant, 0.0);
    }

    let entropy: f64 = tag_counts
        .values()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / total;
            -p * p.ln()
        })
        .sum();

    let max_entropy = n.ln();
    let diversity = if max_entropy > 0.0 {
        entropy / max_entropy
    } else {
        0.0
    };

    (dominant, diversity)
}

// ---------------------------------------------------------------------------
// Serializable response types
// ---------------------------------------------------------------------------

/// Comprehensive session-level gameplay statistics.
///
/// Computed from live gameplay counters. Resets when a new game starts.
/// Tracks contract completions, card usage patterns, token flow,
/// efficiency metrics, streaks, and strategy analysis.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct SessionMetrics {
    /// Total contracts completed across all tiers.
    pub total_contracts_completed: u32,
    /// Total contracts failed across all tiers (includes abandoned contracts).
    pub total_contracts_failed: u32,
    /// Total contracts abandoned via `AbandonContract`.
    /// These are a subset of `total_contracts_failed`.
    pub total_contracts_abandoned: u32,
    /// Per-tier completion statistics.
    pub contracts_per_tier: Vec<TierCompletionMetrics>,

    /// Total cards played from hand (not including discards).
    pub total_cards_played: u32,
    /// Total cards discarded for baseline bonus.
    pub total_cards_discarded: u32,
    /// Cards played broken down by card tag.
    pub cards_per_tag: Vec<TagPlayCount>,

    /// Average number of cards (played + discarded) per completed contract.
    /// `None` if no contracts have been completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_cards_per_contract: Option<f64>,

    /// Token production and consumption totals per type.
    pub token_flow: Vec<TokenFlowMetrics>,

    /// Current consecutive contract completion streak.
    pub current_streak: u32,
    /// Best consecutive contract completion streak this session.
    pub best_streak: u32,

    /// The most-played card tag (e.g., "Production").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dominant_strategy: Option<String>,
    /// Normalized Shannon entropy of tag play distribution.
    /// 0.0 = single tag only, 1.0 = perfectly even across all tags.
    pub strategy_diversity_score: f64,

    /// Total cards replaced via deckbuilding.
    pub total_cards_replaced: u32,

    /// Current adaptive balance pressures per token type.
    /// Shows how heavily the player relies on each token,
    /// which influences how future contract requirements are adjusted.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub adaptive_pressure: Vec<crate::adaptive_balance::TokenPressure>,
}

/// Completion statistics for a single contract tier.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TierCompletionMetrics {
    pub tier: u32,
    pub completed: u32,
    pub failed: u32,
    pub attempted: u32,
    /// Completion rate (completed / attempted). 0.0 if no attempts.
    pub completion_rate: f64,
}

/// Number of cards played with a specific tag.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TagPlayCount {
    pub tag: CardTag,
    pub count: u32,
}

/// Token production/consumption flow for a single token type.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TokenFlowMetrics {
    pub token_type: TokenType,
    pub total_produced: u32,
    pub total_consumed: u32,
    /// Net flow: produced − consumed (can be negative).
    pub net: i64,
}
