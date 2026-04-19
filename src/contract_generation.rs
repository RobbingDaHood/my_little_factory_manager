//! Formula-based contract and reward card generation.
//!
//! Contracts are generated per tier using `TierScalingFormula` parameters.
//! Each formula produces a range `[base_min + tier × per_tier_min,
//! base_max + tier × per_tier_max]` and a value is rolled uniformly
//! within that range using the seeded RNG.
//!
//! Reward card effects are driven by `CardEffectTypeConfig` definitions
//! loaded from `configurations/card_effects/effect_types.json`.

use rand::RngCore;
use rand_pcg::Pcg64;

use crate::config::{CardEffectTypeConfig, ContractFormulasConfig, TierScalingFormula};
use crate::config_loader::load_effect_types;
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
/// Phase 6 will add more types gated by `unlocked_at_tier`.
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
/// Uses config-driven effect type selection from `effect_types.json`.
fn generate_reward_card(
    tier: ContractTier,
    num_effects: usize,
    rng: &mut Pcg64,
    formulas: &ContractFormulasConfig,
) -> PlayerActionCard {
    let all_effect_types = load_effect_types().expect("embedded effect types must parse");
    generate_reward_card_with_types(tier, num_effects, rng, formulas, &all_effect_types)
}

/// Generate a reward card using explicit effect type definitions (for testing).
pub fn generate_reward_card_with_types(
    tier: ContractTier,
    num_effects: usize,
    rng: &mut Pcg64,
    _formulas: &ContractFormulasConfig,
    effect_types: &[CardEffectTypeConfig],
) -> PlayerActionCard {
    let available: Vec<&CardEffectTypeConfig> = effect_types
        .iter()
        .filter(|et| et.unlocked_at_tier <= tier.0)
        .collect();

    // Fallback to hardcoded pure production if no config types available
    if available.is_empty() {
        return generate_fallback_reward_card(tier, num_effects, rng);
    }

    let mut all_tags: Vec<CardTag> = Vec::new();
    let effects: Vec<CardEffect> = (0..num_effects)
        .map(|_| {
            let selected = available[rng.next_u32() as usize % available.len()];

            // Collect tags from selected effect type
            for tag_str in &selected.tags {
                if let Some(tag) = parse_card_tag(tag_str) {
                    if !all_tags.contains(&tag) {
                        all_tags.push(tag);
                    }
                }
            }

            let inputs: Vec<TokenAmount> = selected
                .inputs
                .iter()
                .filter_map(|ef| {
                    let token = parse_token_type(&ef.token_type)?;
                    let amount = roll_from_formula(tier.0, &ef.formula, rng);
                    Some(TokenAmount {
                        token_type: token,
                        amount,
                    })
                })
                .collect();

            let outputs: Vec<TokenAmount> = selected
                .outputs
                .iter()
                .filter_map(|ef| {
                    let token = parse_token_type(&ef.token_type)?;
                    let amount = roll_from_formula(tier.0, &ef.formula, rng);
                    Some(TokenAmount {
                        token_type: token,
                        amount,
                    })
                })
                .collect();

            CardEffect::new(inputs, outputs).expect("config-driven effect should be valid")
        })
        .collect();

    if all_tags.is_empty() {
        all_tags.push(CardTag::Production);
    }

    PlayerActionCard {
        tags: all_tags,
        effects,
    }
}

/// Fallback reward card when no effect types are configured for the tier.
fn generate_fallback_reward_card(
    tier: ContractTier,
    num_effects: usize,
    rng: &mut Pcg64,
) -> PlayerActionCard {
    let formula = TierScalingFormula {
        min_tier: 1,
        base_min: 0,
        base_max: 1,
        per_tier_min: 1,
        per_tier_max: 2,
    };
    let effects: Vec<CardEffect> = (0..num_effects)
        .map(|_| {
            let amount = roll_from_formula(tier.0, &formula, rng);
            CardEffect::new(
                vec![],
                vec![TokenAmount {
                    token_type: TokenType::ProductionUnit,
                    amount,
                }],
            )
            .expect("fallback effect is always valid")
        })
        .collect();

    PlayerActionCard {
        tags: vec![CardTag::Production],
        effects,
    }
}

/// Parse a config string into a `TokenType`.
fn parse_token_type(s: &str) -> Option<TokenType> {
    match s {
        "ProductionUnit" => Some(TokenType::ProductionUnit),
        "Heat" => Some(TokenType::Heat),
        "Energy" => Some(TokenType::Energy),
        "Waste" => Some(TokenType::Waste),
        "DeckSlots" => Some(TokenType::DeckSlots),
        _ => None,
    }
}

/// Parse a config string into a `CardTag`.
fn parse_card_tag(s: &str) -> Option<CardTag> {
    match s {
        "Production" => Some(CardTag::Production),
        "QualityControl" => Some(CardTag::QualityControl),
        "Transformation" => Some(CardTag::Transformation),
        "SystemAdjustment" => Some(CardTag::SystemAdjustment),
        _ => None,
    }
}
