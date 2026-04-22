//! Adaptive balance system: adjusts contract difficulty based on player behaviour.
//!
//! Tracks per-token "pressure" from gross production. After each contract
//! resolution the pressures are updated:
//! - Tokens used heavily → pressure rises → contracts tighten in that area.
//! - Tokens not used → pressure decays toward zero.
//! - Contract failure → all pressures relax (multiplied by `failure_relaxation`).
//!
//! The overlay is applied to newly generated contracts *after* base requirements
//! are rolled, so the underlying generation logic stays untouched.

use std::collections::HashMap;

use rocket::serde::Serialize;
use schemars::JsonSchema;

use crate::config::AdaptiveBalanceConfig;
use crate::types::{AdaptiveAdjustment, ContractRequirementKind, TokenType};

// ---------------------------------------------------------------------------
// Tracker
// ---------------------------------------------------------------------------

/// Accumulates per-token pressure and per-contract gross token flow.
#[derive(Debug, Clone)]
pub struct AdaptiveBalanceTracker {
    /// Long-running exponential moving average of gross production per token.
    token_pressure: HashMap<TokenType, f64>,

    /// Gross tokens produced during the current contract (reset each contract).
    contract_gross_produced: HashMap<TokenType, u32>,

    /// Config snapshot.
    config: AdaptiveBalanceConfig,
}

impl AdaptiveBalanceTracker {
    pub fn new(config: AdaptiveBalanceConfig) -> Self {
        Self {
            token_pressure: HashMap::new(),
            contract_gross_produced: HashMap::new(),
            config,
        }
    }

    /// Record token production from a card play or discard.
    pub fn record_token_produced(&mut self, token_type: &TokenType, amount: u32) {
        *self
            .contract_gross_produced
            .entry(token_type.clone())
            .or_insert(0) += amount;
    }

    /// Called when a contract is completed — updates pressures and resets
    /// per-contract accumulators.
    pub fn on_contract_completed(&mut self) {
        self.update_pressures();
        self.contract_gross_produced.clear();
    }

    /// Called when a contract fails — relaxes all pressures and resets
    /// per-contract accumulators.
    pub fn on_contract_failed(&mut self) {
        self.update_pressures();
        let factor = self.config.failure_relaxation;
        for pressure in self.token_pressure.values_mut() {
            *pressure *= factor;
        }
        self.contract_gross_produced.clear();
    }

    /// Apply adaptive overlay to a set of requirements that were already
    /// base-rolled. Returns the adjustments made (may be empty).
    pub fn apply_overlay(
        &self,
        requirements: &mut [ContractRequirementKind],
    ) -> Vec<AdaptiveAdjustment> {
        let mut adjustments = Vec::new();

        for (idx, req) in requirements.iter_mut().enumerate() {
            match req {
                ContractRequirementKind::TokenRequirement {
                    token_type,
                    min,
                    max,
                } => {
                    let pressure = self.token_pressure.get(token_type).copied().unwrap_or(0.0);
                    if pressure.abs() < f64::EPSILON {
                        continue;
                    }

                    // Tighten max (harmful bound)
                    if let Some(max_amount) = max {
                        let ratio = (pressure / self.config.normalization_factor)
                            .clamp(0.0, self.config.max_tightening_pct);
                        let original = *max_amount;
                        let reduction = (original as f64 * ratio).round() as u32;
                        let adjusted = original.saturating_sub(reduction).max(1);
                        if adjusted != original {
                            let pct = -((original as f64 - adjusted as f64) / original as f64
                                * 100.0)
                                .round() as i32;
                            adjustments.push(AdaptiveAdjustment {
                                requirement_index: idx,
                                original_value: original,
                                adjusted_value: adjusted,
                                adjustment_percent: pct,
                            });
                            *max_amount = adjusted;
                        }
                    }

                    // Raise min (beneficial bound)
                    if let Some(min_amount) = min {
                        let ratio = (pressure / self.config.normalization_factor)
                            .clamp(0.0, self.config.max_increase_pct);
                        let original = *min_amount;
                        let increase = (original as f64 * ratio).round() as u32;
                        let adjusted = original + increase;
                        if adjusted != original {
                            let pct = ((adjusted as f64 - original as f64) / original as f64
                                * 100.0)
                                .round() as i32;
                            adjustments.push(AdaptiveAdjustment {
                                requirement_index: idx,
                                original_value: original,
                                adjusted_value: adjusted,
                                adjustment_percent: pct,
                            });
                            *min_amount = adjusted;
                        }
                    }
                }
                ContractRequirementKind::CardTagConstraint { .. }
                | ContractRequirementKind::TurnWindow { .. } => {}
            }
        }

        adjustments
    }

    /// Current pressure values — exposed for transparency endpoints.
    pub fn pressures(&self) -> &HashMap<TokenType, f64> {
        &self.token_pressure
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn update_pressures(&mut self) {
        let alpha = self.config.alpha;
        let decay = self.config.decay_rate;

        // Collect all token types we've ever seen pressure for OR produced this contract.
        let mut all_tokens: Vec<TokenType> = self.token_pressure.keys().cloned().collect();
        for t in self.contract_gross_produced.keys() {
            if !all_tokens.contains(t) {
                all_tokens.push(t.clone());
            }
        }

        for token_type in &all_tokens {
            let produced = self
                .contract_gross_produced
                .get(token_type)
                .copied()
                .unwrap_or(0) as f64;
            let current = self.token_pressure.get(token_type).copied().unwrap_or(0.0);

            let new_pressure = if produced > 0.0 {
                alpha * produced + (1.0 - alpha) * current
            } else {
                current * decay
            };

            self.token_pressure.insert(token_type.clone(), new_pressure);
        }
    }
}

// ---------------------------------------------------------------------------
// Serializable pressure snapshot (for transparency)
// ---------------------------------------------------------------------------

/// Per-token adaptive pressure for the `/metrics` response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct TokenPressure {
    pub token_type: TokenType,
    pub pressure: f64,
}

impl AdaptiveBalanceTracker {
    /// Build a sorted snapshot of current pressures for API responses.
    pub fn pressure_snapshot(&self) -> Vec<TokenPressure> {
        let mut out: Vec<TokenPressure> = self
            .token_pressure
            .iter()
            .filter(|(_, &p)| p.abs() > f64::EPSILON)
            .map(|(tt, &p)| TokenPressure {
                token_type: tt.clone(),
                pressure: (p * 100.0).round() / 100.0,
            })
            .collect();
        out.sort_by(|a, b| a.token_type.cmp(&b.token_type));
        out
    }
}
