//! Formula-based contract and reward card generation.
//!
//! Contracts are generated per tier using `TierScalingFormula` parameters.
//! Each formula produces a range `[base_min + tier × per_tier_min,
//! base_max + tier × per_tier_max]` and a value is rolled uniformly
//! within that range using the seeded RNG.
//!
//! Card effect types are auto-generated from `token_definitions.json` using
//! a combinatorial algorithm that produces mains and variations for all
//! 7 tokens, assigned 2 items per tier (0-indexed).

use rand::RngCore;
use rand_pcg::Pcg64;

use crate::adaptive_balance::AdaptiveBalanceTracker;
use crate::config::{
    CardEffectTypeConfig, CardEffectVariation, CardTagConstraintFormulaConfig,
    ContractFormulasConfig, ModifierRange, TierScalingFormula, TokenDefinitionsConfig,
    TurnWindowFormulaConfig, VariationDefaultsConfig,
};
use crate::config_loader::load_token_definitions;
use crate::types::{
    CardEffect, CardTag, Contract, ContractRequirementKind, ContractTier, MainEffectDirection,
    PlayerActionCard, TokenAmount, TokenType, VariationDirection,
};

type RequirementGenerator = Box<dyn Fn(&mut Pcg64) -> ContractRequirementKind>;

// ---------------------------------------------------------------------------
// Core formula helpers
// ---------------------------------------------------------------------------

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

/// Roll a modifier value uniformly within a `ModifierRange`.
fn roll_modifier(range: &ModifierRange, rng: &mut Pcg64) -> f64 {
    if range.min >= range.max {
        return range.min;
    }
    let granularity = 10_000u32;
    let t = (rng.next_u32() % granularity) as f64 / granularity as f64;
    range.min + t * (range.max - range.min)
}

// ---------------------------------------------------------------------------
// Combinatorial effect type generator
// ---------------------------------------------------------------------------

/// Generate all card effect types and their variations from token definitions.
///
/// Algorithm:
/// 1. Each token gets 1–2 main effects (PU gets 1 producer, others get producer + consumer/remover)
/// 2. Each main gets a self-consuming variation
/// 3. For each pair (earlier, later), cross-token variations go on the earlier token's mains
/// 4. Items are assigned 2 per tier, incrementing tier number
pub fn generate_effect_types(config: &TokenDefinitionsConfig) -> Vec<CardEffectTypeConfig> {
    let defaults = &config.variation_defaults;
    let items_per_tier = defaults.items_per_tier;

    // Queue of (item, is_main) — mains and variations are enqueued in introduction order
    let mut all_items: Vec<GeneratedItem> = Vec::new();

    for (token_idx, token_def) in config.tokens.iter().enumerate() {
        let is_first = token_idx == 0;

        // Producer main
        let producer_name = format!("{:?}Producer", token_def.token_type);
        all_items.push(GeneratedItem::Main {
            name: producer_name,
            tags: token_def.producer_tags.clone(),
            primary_token: token_def.token_type.clone(),
            direction: MainEffectDirection::Producer,
            formula: token_def.primary_formula.clone(),
        });

        // Consumer/remover main (all tokens except first)
        if !is_first {
            let consumer_name = if token_def.is_beneficial {
                format!("{:?}Consumer", token_def.token_type)
            } else {
                format!("{:?}Remover", token_def.token_type)
            };
            all_items.push(GeneratedItem::Main {
                name: consumer_name,
                tags: token_def.consumer_tags.clone(),
                primary_token: token_def.token_type.clone(),
                direction: MainEffectDirection::Consumer,
                formula: token_def.primary_formula.clone(),
            });
        }

        // Self-consuming variation for producer
        all_items.push(GeneratedItem::SelfConsuming {
            main_token: token_def.token_type.clone(),
            main_direction: MainEffectDirection::Producer,
            secondary_token: token_def.token_type.clone(),
            is_beneficial: token_def.is_beneficial,
        });

        // Self-consuming variation for consumer/remover (if exists)
        if !is_first {
            all_items.push(GeneratedItem::SelfConsuming {
                main_token: token_def.token_type.clone(),
                main_direction: MainEffectDirection::Consumer,
                secondary_token: token_def.token_type.clone(),
                is_beneficial: token_def.is_beneficial,
            });
        }

        // Cross-token variations with all EARLIER tokens
        for earlier_def in &config.tokens[..token_idx] {
            // For each main of the earlier token, add input + output variations with current token
            let earlier_mains = if config
                .tokens
                .iter()
                .position(|t| t.token_type == earlier_def.token_type)
                == Some(0)
            {
                vec![MainEffectDirection::Producer]
            } else {
                vec![MainEffectDirection::Producer, MainEffectDirection::Consumer]
            };

            for main_dir in &earlier_mains {
                // Current token as output on earlier main
                all_items.push(GeneratedItem::CrossToken {
                    main_token: earlier_def.token_type.clone(),
                    main_direction: main_dir.clone(),
                    secondary_token: token_def.token_type.clone(),
                    variation_direction: VariationDirection::Output,
                    secondary_is_beneficial: token_def.is_beneficial,
                });

                // Current token as input on earlier main
                all_items.push(GeneratedItem::CrossToken {
                    main_token: earlier_def.token_type.clone(),
                    main_direction: main_dir.clone(),
                    secondary_token: token_def.token_type.clone(),
                    variation_direction: VariationDirection::Input,
                    secondary_is_beneficial: token_def.is_beneficial,
                });
            }
        }
    }

    // Assign tiers: 2 items per tier
    let mut tier_counter = 0u32;
    let mut items_in_current_tier = 0u32;

    // Build effect types from items
    // Mains create new CardEffectTypeConfig entries; variations attach to existing mains
    let mut effect_types: Vec<CardEffectTypeConfig> = Vec::new();

    for item in &all_items {
        let current_tier = tier_counter;

        match item {
            GeneratedItem::Main {
                name,
                tags,
                primary_token,
                direction,
                formula,
            } => {
                effect_types.push(CardEffectTypeConfig {
                    name: name.clone(),
                    tags: tags.clone(),
                    available_at_tier: current_tier,
                    primary_token: primary_token.clone(),
                    main_direction: direction.clone(),
                    primary_formula: formula.clone(),
                    variations: Vec::new(),
                });
            }
            GeneratedItem::SelfConsuming {
                main_token,
                main_direction,
                secondary_token,
                is_beneficial,
            } => {
                let direction_sign = compute_direction_sign(
                    *is_beneficial,
                    &self_consuming_variation_direction(main_direction),
                );

                let variation = CardEffectVariation {
                    name: format!("{:?}SelfConsuming", secondary_token),
                    secondary_token: secondary_token.clone(),
                    direction: self_consuming_variation_direction(main_direction),
                    is_self_consuming: true,
                    direction_sign,
                    unlock_tier: current_tier,
                };

                attach_variation(&mut effect_types, main_token, main_direction, variation);
            }
            GeneratedItem::CrossToken {
                main_token,
                main_direction,
                secondary_token,
                variation_direction,
                secondary_is_beneficial,
            } => {
                let direction_sign =
                    compute_direction_sign(*secondary_is_beneficial, variation_direction);

                let dir_label = match variation_direction {
                    VariationDirection::Input => "Input",
                    VariationDirection::Output => "Output",
                };
                let variation = CardEffectVariation {
                    name: format!("{:?}{}", secondary_token, dir_label),
                    secondary_token: secondary_token.clone(),
                    direction: variation_direction.clone(),
                    is_self_consuming: false,
                    direction_sign,
                    unlock_tier: current_tier,
                };

                attach_variation(&mut effect_types, main_token, main_direction, variation);
            }
        }

        items_in_current_tier += 1;
        if items_in_current_tier >= items_per_tier {
            items_in_current_tier = 0;
            tier_counter += 1;
        }
    }

    effect_types
}

/// For self-consuming variations, the secondary token direction is the opposite
/// of the main: producer self-consuming = input, consumer self-consuming = output.
fn self_consuming_variation_direction(main_dir: &MainEffectDirection) -> VariationDirection {
    match main_dir {
        MainEffectDirection::Producer => VariationDirection::Input,
        MainEffectDirection::Consumer => VariationDirection::Output,
    }
}

/// Compute direction_sign based on the secondary token's beneficial/harmful
/// classification and whether it's an input or output:
///
/// | Beneficial? | Direction | Player impact       | sign |
/// |-------------|-----------|---------------------|------|
/// | harmful     | output    | accepts harm        | +1   |
/// | beneficial  | input     | sacrifices good     | +1   |
/// | harmful     | input     | removes harm        | -1   |
/// | beneficial  | output    | gets extra good     | -1   |
fn compute_direction_sign(is_beneficial: bool, direction: &VariationDirection) -> i8 {
    match (is_beneficial, direction) {
        (false, VariationDirection::Output) => 1, // accepts harm → boosts
        (true, VariationDirection::Input) => 1,   // sacrifices good → boosts
        (false, VariationDirection::Input) => -1, // removes harm → costs
        (true, VariationDirection::Output) => -1, // extra good → costs
    }
}

fn attach_variation(
    effect_types: &mut [CardEffectTypeConfig],
    main_token: &TokenType,
    main_direction: &MainEffectDirection,
    variation: CardEffectVariation,
) {
    for et in effect_types.iter_mut().rev() {
        if et.primary_token == *main_token && et.main_direction == *main_direction {
            et.variations.push(variation);
            return;
        }
    }
}

/// Intermediate type used during generation.
enum GeneratedItem {
    Main {
        name: String,
        tags: Vec<CardTag>,
        primary_token: TokenType,
        direction: MainEffectDirection,
        formula: TierScalingFormula,
    },
    SelfConsuming {
        main_token: TokenType,
        main_direction: MainEffectDirection,
        secondary_token: TokenType,
        is_beneficial: bool,
    },
    CrossToken {
        main_token: TokenType,
        main_direction: MainEffectDirection,
        secondary_token: TokenType,
        variation_direction: VariationDirection,
        secondary_is_beneficial: bool,
    },
}

// ---------------------------------------------------------------------------
// Contract generation
// ---------------------------------------------------------------------------

/// Returns token types that have at least one card effect unlocked at or before
/// the given tier. Used for requirement tier-gating.
fn unlocked_token_types(tier: u32, effect_types: &[CardEffectTypeConfig]) -> Vec<TokenType> {
    let mut tokens = Vec::new();
    for et in effect_types {
        if et.available_at_tier <= tier && !tokens.contains(&et.primary_token) {
            tokens.push(et.primary_token.clone());
        }
    }
    tokens
}

/// Returns card tags associated with effect types unlocked at or before the given tier.
/// Used to ensure CardTagConstraint only references tags that have actual cards.
fn unlocked_card_tags(tier: u32, effect_types: &[CardEffectTypeConfig]) -> Vec<CardTag> {
    let mut tags = Vec::new();
    for et in effect_types {
        if et.available_at_tier <= tier {
            for tag in &et.tags {
                if !tags.contains(tag) {
                    tags.push(tag.clone());
                }
            }
        }
    }
    tags
}

/// Returns the requirement types available at the given tier, paired with
/// generator functions. Only uses token types with unlocked card effects.
fn available_requirement_generators(
    tier: u32,
    formulas: &ContractFormulasConfig,
    effect_types: &[CardEffectTypeConfig],
) -> Vec<RequirementGenerator> {
    let mut generators: Vec<RequirementGenerator> = Vec::new();
    let available_tokens = unlocked_token_types(tier, effect_types);

    // TokenRequirement generators for each beneficial token that's unlocked
    for token in &available_tokens {
        if token.is_beneficial() {
            let t = token.clone();
            let formula = formulas.output_threshold.clone();
            generators.push(Box::new(move |rng: &mut Pcg64| {
                ContractRequirementKind::TokenRequirement {
                    token_type: t.clone(),
                    min: Some(roll_from_formula(tier, &formula, rng)),
                    max: None,
                }
            }));
        }
    }

    // TokenRequirement generators for each harmful token that's unlocked
    for token in &available_tokens {
        if token.is_harmful() {
            let t = token.clone();
            let formula = formulas.harmful_token_limit.clone();
            generators.push(Box::new(move |rng: &mut Pcg64| {
                ContractRequirementKind::TokenRequirement {
                    token_type: t.clone(),
                    min: None,
                    max: Some(roll_from_formula(tier, &formula, rng)),
                }
            }));
        }
    }

    // TurnWindow generator — unlocks at a specific tier in the Energy→Waste gap
    if let Some(tw_config) = formulas.turn_window.as_ref() {
        if tier >= tw_config.unlock_tier {
            let tw = tw_config.clone();
            generators.push(Box::new(move |rng: &mut Pcg64| {
                generate_turn_window(tier, &tw, rng)
            }));
        }
    }

    // CardTagConstraint generator — unlocks in the Waste→QP gap
    if let Some(ctc_config) = formulas.card_tag_constraint.as_ref() {
        if tier >= ctc_config.unlock_tier {
            let available_tags = unlocked_card_tags(tier, effect_types);
            if !available_tags.is_empty() {
                let ctc = ctc_config.clone();
                generators.push(Box::new(move |rng: &mut Pcg64| {
                    generate_card_tag_constraint(tier, &ctc, &available_tags, rng)
                }));
            }
        }
    }

    generators
}

fn generate_turn_window(tier: u32, config: &TurnWindowFormulaConfig, rng: &mut Pcg64) -> ContractRequirementKind {
    let min_low = config.min_turns_base + tier * config.min_turns_per_tier;
    let min_turn = min_low + (rng.next_u32() % (tier + 1));
    let window_size = config.window_size_base + tier * config.window_size_per_tier;
    let max_turn = min_turn + window_size;
    ContractRequirementKind::TurnWindow { min_turn, max_turn }
}

fn generate_card_tag_constraint(
    tier: u32,
    config: &CardTagConstraintFormulaConfig,
    available_tags: &[CardTag],
    rng: &mut Pcg64,
) -> ContractRequirementKind {
    let tag_idx = rng.next_u32() as usize % available_tags.len();
    let tag = available_tags[tag_idx].clone();
    let max_count = config.base_count + tier * config.per_tier_count;
    // Alternate between ban (max=0), must-play (min), and limit (max>0) based on RNG
    let variant = rng.next_u32() % 3;
    match variant {
        0 => ContractRequirementKind::CardTagConstraint { tag, min: None, max: Some(0) },
        1 => ContractRequirementKind::CardTagConstraint { tag, min: Some(max_count.max(1)), max: None },
        _ => ContractRequirementKind::CardTagConstraint { tag, min: None, max: Some(max_count.max(1)) },
    }
}

/// Determine the number of requirements for a contract at the given tier.
/// Tier X → max(X−1, 1) to X+1 requirements, capped by available types.
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

/// Roll the tier for a single requirement.
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
    adaptive_tracker: &AdaptiveBalanceTracker,
) -> Contract {
    let token_defs = load_token_definitions().expect("embedded token definitions must parse");
    let effect_types = generate_effect_types(&token_defs);
    generate_contract_with_types(
        tier,
        rng,
        formulas,
        &token_defs,
        &effect_types,
        adaptive_tracker,
    )
}

/// Generate a contract using explicit effect types (for testing).
pub fn generate_contract_with_types(
    tier: ContractTier,
    rng: &mut Pcg64,
    formulas: &ContractFormulasConfig,
    token_defs: &TokenDefinitionsConfig,
    effect_types: &[CardEffectTypeConfig],
    adaptive_tracker: &AdaptiveBalanceTracker,
) -> Contract {
    let generators_at_contract_tier =
        available_requirement_generators(tier.0, formulas, effect_types);
    let req_count = roll_requirement_count(tier.0, generators_at_contract_tier.len(), rng);

    let mut requirements = Vec::with_capacity(req_count);
    for _ in 0..req_count {
        let req_tier = roll_requirement_tier(tier.0, rng);
        let generators = available_requirement_generators(req_tier, formulas, effect_types);
        if generators.is_empty() {
            continue;
        }
        let gen_idx = rng.next_u32() as usize % generators.len();
        requirements.push(generators[gen_idx](rng));
    }

    // Apply adaptive balance overlay after base requirements are rolled
    let adaptive_adjustments = adaptive_tracker.apply_overlay(&mut requirements);

    let reward_card =
        generate_reward_card_with_types(tier, requirements.len(), rng, token_defs, effect_types);

    Contract {
        tier,
        requirements,
        reward_card,
        adaptive_adjustments,
    }
}

// ---------------------------------------------------------------------------
// Reward card generation (proportional model)
// ---------------------------------------------------------------------------

/// Generate a reward card using the combinatorial effect types and proportional model.
pub fn generate_reward_card_with_types(
    tier: ContractTier,
    num_effects: usize,
    rng: &mut Pcg64,
    token_defs: &TokenDefinitionsConfig,
    effect_types: &[CardEffectTypeConfig],
) -> PlayerActionCard {
    let choices = build_effect_choices(tier.0, effect_types);

    let mut all_tags: Vec<CardTag> = Vec::new();
    let effects: Vec<CardEffect> = (0..num_effects)
        .map(|_| {
            let selected = weighted_select(&choices, rng);

            for tag in &selected.root.tags {
                if !all_tags.contains(tag) {
                    all_tags.push(tag.clone());
                }
            }

            match selected.variation {
                Some(v) => roll_variation_effect(
                    tier.0,
                    selected.root,
                    v,
                    &token_defs.variation_defaults,
                    rng,
                ),
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
fn build_effect_choices(tier: u32, effect_types: &[CardEffectTypeConfig]) -> Vec<EffectChoice<'_>> {
    let mut choices = Vec::new();
    for root in effect_types {
        if root.available_at_tier <= tier {
            choices.push(EffectChoice {
                root,
                variation: None,
                unlocked_at_tier: root.available_at_tier,
            });
            for variation in &root.variations {
                if variation.unlock_tier <= tier {
                    choices.push(EffectChoice {
                        root,
                        variation: Some(variation),
                        unlocked_at_tier: variation.unlock_tier,
                    });
                }
            }
        }
    }
    choices
}

/// Weighted random selection: lower `unlocked_at_tier` gets higher weight.
fn weighted_select<'a>(choices: &'a [EffectChoice<'a>], rng: &mut Pcg64) -> &'a EffectChoice<'a> {
    let max_tier = choices
        .iter()
        .map(|c| c.unlocked_at_tier)
        .max()
        .unwrap_or(0);

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

/// Roll the base (unmodified) main effect — no variation applied.
pub(crate) fn roll_base_effect(
    tier: u32,
    root: &CardEffectTypeConfig,
    rng: &mut Pcg64,
) -> CardEffect {
    let primary_amount = roll_from_formula(tier, &root.primary_formula, rng);

    let (inputs, outputs) = match root.main_direction {
        MainEffectDirection::Producer => (
            vec![],
            vec![TokenAmount {
                token_type: root.primary_token.clone(),
                amount: primary_amount,
            }],
        ),
        MainEffectDirection::Consumer => (
            vec![TokenAmount {
                token_type: root.primary_token.clone(),
                amount: primary_amount,
            }],
            vec![],
        ),
    };

    CardEffect::new(inputs, outputs).expect("config-driven effect should be valid")
}

/// Roll a variation effect using the proportional model.
///
/// 1. Roll the unmodified primary amount
/// 2. Roll a ratio from the variation defaults
/// 3. Compute secondary amount = round(unmodified_primary × effective_ratio)
/// 4. Compute modified primary = round(unmodified_primary × modifier)
///    where modifier = 1.0 + ratio × direction_sign × boost_factor
fn roll_variation_effect(
    tier: u32,
    root: &CardEffectTypeConfig,
    variation: &CardEffectVariation,
    defaults: &VariationDefaultsConfig,
    rng: &mut Pcg64,
) -> CardEffect {
    let unmodified_primary = roll_from_formula(tier, &root.primary_formula, rng);

    let rolled_ratio = roll_modifier(&defaults.ratio_range, rng);

    // Effective ratio decreases per tier (higher tiers more efficient)
    let tier_diff = tier.saturating_sub(variation.unlock_tier);
    let effective_ratio =
        (rolled_ratio - tier_diff as f64 * defaults.efficiency_per_tier).max(0.01);

    // Secondary amount from unmodified primary
    let secondary_amount = (unmodified_primary as f64 * effective_ratio)
        .round()
        .max(1.0) as u32;

    // Modified primary
    let modifier = 1.0 + rolled_ratio * variation.direction_sign as f64 * defaults.boost_factor;
    let modified_primary = (unmodified_primary as f64 * modifier).round().max(1.0) as u32;

    // Build inputs/outputs based on main direction and variation direction
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    // Primary token
    match root.main_direction {
        MainEffectDirection::Producer => {
            outputs.push(TokenAmount {
                token_type: root.primary_token.clone(),
                amount: modified_primary,
            });
        }
        MainEffectDirection::Consumer => {
            inputs.push(TokenAmount {
                token_type: root.primary_token.clone(),
                amount: modified_primary,
            });
        }
    }

    // Secondary token
    match variation.direction {
        VariationDirection::Input => {
            inputs.push(TokenAmount {
                token_type: variation.secondary_token.clone(),
                amount: secondary_amount,
            });
        }
        VariationDirection::Output => {
            outputs.push(TokenAmount {
                token_type: variation.secondary_token.clone(),
                amount: secondary_amount,
            });
        }
    }

    CardEffect::new(inputs, outputs).expect("variation effect should be valid")
}
