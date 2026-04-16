//! Formula-based contract and reward card generation.
//!
//! Contracts are generated per tier using `TierScalingFormula` parameters.
//! Each formula produces a range `[base_min + tier × per_tier_min,
//! base_max + tier × per_tier_max]` and a value is rolled uniformly
//! within that range using the seeded RNG.

use rand::RngCore;
use rand_pcg::Pcg64;

use crate::config::{ContractFormulasConfig, TierScalingFormula};
use crate::types::{
    CardEffect, CardTag, Contract, ContractRequirementKind, ContractTier, PlayerActionCard,
    TokenAmount, TokenType,
};

type RequirementGenerator = Box<dyn Fn(&mut Pcg64) -> ContractRequirementKind>;

/// Roll a value from a tier-scaling formula for the given tier.
fn roll_from_formula(tier: u32, formula: &TierScalingFormula, rng: &mut Pcg64) -> u32 {
    let min = formula.base_min + tier * formula.per_tier_min;
    let max = formula.base_max + tier * formula.per_tier_max;
    if min >= max {
        return min;
    }
    let range = max - min + 1;
    min + (rng.next_u32() % range)
}

/// Returns the requirement types available at the given tier, paired with
/// a generator function. Currently only OutputThreshold is implemented;
/// Phase 6 will add more types gated by `min_tier`.
fn available_requirement_generators(
    tier: u32,
    formulas: &ContractFormulasConfig,
) -> Vec<RequirementGenerator> {
    let mut generators: Vec<RequirementGenerator> = Vec::new();

    if tier >= formulas.output_threshold.min_tier {
        let formula = formulas.output_threshold.clone();
        generators.push(Box::new(move |rng: &mut Pcg64| {
            ContractRequirementKind::OutputThreshold {
                token_type: TokenType::ProductionUnit,
                min_amount: roll_from_formula(tier, &formula, rng),
            }
        }));
    }

    generators
}

/// Determine the number of requirements for a contract at the given tier.
/// Vision rule: tier X → max(X−1, 1) to X+1 requirements, capped by
/// the number of available requirement types.
fn roll_requirement_count(tier: u32, available_types: usize, rng: &mut Pcg64) -> usize {
    let min_reqs = (tier.saturating_sub(1)).max(1) as usize;
    let max_reqs = (tier + 1) as usize;
    let max_reqs = max_reqs.min(available_types);
    let min_reqs = min_reqs.min(max_reqs);
    if min_reqs == max_reqs {
        return min_reqs;
    }
    let range = max_reqs - min_reqs + 1;
    min_reqs + (rng.next_u32() as usize % range)
}

/// Generate a single contract for the given tier.
pub fn generate_contract(
    tier: ContractTier,
    rng: &mut Pcg64,
    formulas: &ContractFormulasConfig,
) -> Contract {
    let generators = available_requirement_generators(tier.0, formulas);
    let req_count = roll_requirement_count(tier.0, generators.len(), rng);

    let mut requirements = Vec::with_capacity(req_count);
    for i in 0..req_count {
        let gen_idx = i % generators.len();
        requirements.push(generators[gen_idx](rng));
    }

    let reward_card = generate_reward_card(tier, requirements.len(), rng, formulas);

    Contract {
        tier,
        requirements,
        reward_card,
    }
}

/// Generate a reward card with the given number of effects at the given tier.
fn generate_reward_card(
    tier: ContractTier,
    num_effects: usize,
    rng: &mut Pcg64,
    formulas: &ContractFormulasConfig,
) -> PlayerActionCard {
    let effects: Vec<CardEffect> = (0..num_effects)
        .map(|_| {
            let amount = roll_from_formula(tier.0, &formulas.reward_production, rng);
            CardEffect::new(
                vec![],
                vec![TokenAmount {
                    token_type: TokenType::ProductionUnit,
                    amount,
                }],
            )
            .expect("reward card effect with production output is always valid")
        })
        .collect();

    PlayerActionCard {
        tags: vec![CardTag::Production],
        effects,
    }
}
