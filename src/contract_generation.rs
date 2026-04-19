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

use crate::config::{
    CardEffectTypeConfig, CardEffectVariation, ContractFormulasConfig, EffectFormula,
    ModifierRange, TierScalingFormula,
};
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

    {
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

/// Roll the tier for a single requirement. Each requirement's tier is
/// rolled independently from `max(1, contract_tier − 1)` to `contract_tier + 1`.
fn roll_requirement_tier(contract_tier: u32, rng: &mut Pcg64) -> u32 {
    let min_tier = contract_tier.saturating_sub(1);
    let max_tier = contract_tier + 1;
    let range = max_tier - min_tier + 1;
    min_tier + (rng.next_u32() % range)
}

/// Generate a single contract for the given tier.
pub fn generate_contract(
    tier: ContractTier,
    rng: &mut Pcg64,
    formulas: &ContractFormulasConfig,
) -> Contract {
    let generators_at_contract_tier = available_requirement_generators(tier.0, formulas);
    let req_count = roll_requirement_count(tier.0, generators_at_contract_tier.len(), rng);

    let mut requirements = Vec::with_capacity(req_count);
    for _ in 0..req_count {
        let req_tier = roll_requirement_tier(tier.0, rng);
        let generators = available_requirement_generators(req_tier, formulas);
        if generators.is_empty() {
            continue;
        }
        let gen_idx = rng.next_u32() as usize % generators.len();
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
    let choices = build_effect_choices(tier.0, effect_types);

    let mut all_tags: Vec<CardTag> = Vec::new();
    let effects: Vec<CardEffect> = (0..num_effects)
        .map(|_| {
            let selected = weighted_select(&choices, rng);

            for tag_str in &selected.root.tags {
                if let Some(tag) = parse_card_tag(tag_str) {
                    if !all_tags.contains(&tag) {
                        all_tags.push(tag);
                    }
                }
            }

            match selected.variation {
                Some(v) => roll_variation_effect(tier.0, selected.root, v, rng),
                None => roll_base_effect(tier.0, selected.root, rng),
            }
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

/// A flattened entry: either a root's base effect or one of its variations.
struct EffectChoice<'a> {
    root: &'a CardEffectTypeConfig,
    variation: Option<&'a CardEffectVariation>,
    unlocked_at_tier: u32,
}

/// Build a flat list of all available effect choices for the given tier.
/// Each root's base effect and each unlocked variation are separate entries.
fn build_effect_choices(tier: u32, effect_types: &[CardEffectTypeConfig]) -> Vec<EffectChoice<'_>> {
    let mut choices = Vec::new();
    for root in effect_types {
        if root.unlocked_at_tier <= tier {
            choices.push(EffectChoice {
                root,
                variation: None,
                unlocked_at_tier: root.unlocked_at_tier,
            });
            for variation in &root.variations {
                if variation.unlocked_at_tier <= tier {
                    choices.push(EffectChoice {
                        root,
                        variation: Some(variation),
                        unlocked_at_tier: variation.unlocked_at_tier,
                    });
                }
            }
        }
    }
    choices
}

/// Weighted random selection: lower `unlocked_at_tier` gets higher weight.
/// Weight = current_max_tier - unlocked_at_tier + 1.
fn weighted_select<'a>(choices: &'a [EffectChoice<'a>], rng: &mut Pcg64) -> &'a EffectChoice<'a> {
    let max_tier = choices
        .iter()
        .map(|c| c.unlocked_at_tier)
        .max()
        .unwrap_or(1);

    let weights: Vec<u32> = choices
        .iter()
        .map(|c| max_tier - c.unlocked_at_tier + 1)
        .collect();
    let total_weight: u32 = weights.iter().sum();

    let mut roll = rng.next_u32() % total_weight;
    for (choice, &w) in choices.iter().zip(&weights) {
        if roll < w {
            return choice;
        }
        roll -= w;
    }
    choices.last().expect("choices must not be empty")
}

/// Roll the base (unmodified) root effect.
pub(crate) fn roll_base_effect(
    tier: u32,
    root: &CardEffectTypeConfig,
    rng: &mut Pcg64,
) -> CardEffect {
    let inputs = roll_effect_formulas(tier, &root.inputs, rng);
    let outputs = roll_effect_formulas(tier, &root.outputs, rng);
    CardEffect::new(inputs, outputs).expect("config-driven effect should be valid")
}

/// Roll a variation effect: roll root primary output, apply modifier, then
/// add the variation's extra token exchanges.
fn roll_variation_effect(
    tier: u32,
    root: &CardEffectTypeConfig,
    variation: &CardEffectVariation,
    rng: &mut Pcg64,
) -> CardEffect {
    let mut outputs = roll_effect_formulas(tier, &root.outputs, rng);

    // Apply modifier to the primary (first) output
    if let Some(primary) = outputs.first_mut() {
        let modifier = roll_modifier(&variation.modifier_range, rng);
        primary.amount = (primary.amount as f64 * modifier).round() as u32;
        if primary.amount == 0 {
            primary.amount = 1;
        }
    }

    let mut inputs = roll_effect_formulas(tier, &root.inputs, rng);

    // Append variation's extra exchanges
    inputs.extend(roll_effect_formulas(tier, &variation.extra_inputs, rng));
    outputs.extend(roll_effect_formulas(tier, &variation.extra_outputs, rng));

    CardEffect::new(inputs, outputs).expect("variation effect should be valid")
}

/// Roll a modifier value uniformly within a `ModifierRange`.
fn roll_modifier(range: &ModifierRange, rng: &mut Pcg64) -> f64 {
    if range.min >= range.max {
        return range.min;
    }
    let granularity = 10_000u32;
    let t = (rng.next_u32() % granularity) as f64 / granularity as f64;
    range.min + t * (range.max - range.min)
}

/// Roll concrete token amounts from a list of effect formulas.
fn roll_effect_formulas(
    tier: u32,
    formulas: &[EffectFormula],
    rng: &mut Pcg64,
) -> Vec<TokenAmount> {
    formulas
        .iter()
        .filter_map(|ef| {
            let token = parse_token_type(&ef.token_type)?;
            let amount = roll_from_formula(tier, &ef.formula, rng);
            Some(TokenAmount {
                token_type: token,
                amount,
            })
        })
        .collect()
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
pub(crate) fn parse_card_tag(s: &str) -> Option<CardTag> {
    match s {
        "Production" => Some(CardTag::Production),
        "QualityControl" => Some(CardTag::QualityControl),
        "Transformation" => Some(CardTag::Transformation),
        "SystemAdjustment" => Some(CardTag::SystemAdjustment),
        _ => None,
    }
}
