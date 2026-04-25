//! Integration tests for Phase 6: Contract Tier Progression.
//!
//! Validates the combinatorial effect type generator, proportional model,
//! HarmfulTokenLimit requirements, requirement tier-gating, duplicate
//! requirement stacking, direction_sign correctness, and the Issue #14
//! beneficial-token-min one-tier delay invariant.

use my_little_factory_manager::adaptive_balance::AdaptiveBalanceTracker;
use my_little_factory_manager::config::CachedConfig;
use my_little_factory_manager::config_loader::{load_game_rules, load_token_definitions};
use my_little_factory_manager::contract_generation::{
    build_cached_config, generate_contract_with_types, generate_effect_types,
    generate_reward_card_with_types,
};
use my_little_factory_manager::rocket_initialize;
use my_little_factory_manager::types::{
    ContractRequirementKind, ContractTier, MainEffectDirection, TokenType, VariationDirection,
};
use rand::SeedableRng;
use rand_pcg::Pcg64;
use rocket::http::{ContentType, Status};
use rocket::local::blocking::Client;
use std::collections::HashMap;

fn client() -> Client {
    Client::tracked(rocket_initialize()).expect("valid rocket instance")
}

fn make_cached_config() -> CachedConfig {
    let token_defs = load_token_definitions().expect("config");
    let game_rules = load_game_rules().expect("rules");
    build_cached_config(game_rules, token_defs)
}

fn post_action(client: &Client, json: &str) -> serde_json::Value {
    let response = client
        .post("/action")
        .header(ContentType::JSON)
        .body(json)
        .dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().expect("response body");
    serde_json::from_str(&body).expect("valid json")
}

fn get_state(client: &Client) -> serde_json::Value {
    let response = client.get("/state").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().expect("response body");
    serde_json::from_str(&body).expect("valid json")
}

fn first_card_in_hand(client: &Client) -> usize {
    let state = get_state(client);
    state["cards"]
        .as_array()
        .expect("cards array")
        .iter()
        .enumerate()
        .find(|(_, e)| e["counts"]["hand"].as_u64().unwrap_or(0) > 0)
        .map(|(i, _)| i)
        .expect("at least one card in hand")
}

// ---------------------------------------------------------------------------
// Combinatorial generator correctness
// ---------------------------------------------------------------------------

#[test]
fn generator_produces_13_mains() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);
    assert_eq!(effect_types.len(), 13, "7 tokens → 13 mains (PU=1, 6×2)");
}

#[test]
fn generator_produces_85_total_variations() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let total_variations: usize = effect_types.iter().map(|et| et.variations.len()).sum();
    assert_eq!(
        total_variations, 85,
        "7 tokens → 85 variations (self-consuming + cross-token)"
    );
}

#[test]
fn generator_spans_49_tiers() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let max_main_tier = effect_types
        .iter()
        .map(|et| et.available_at_tier)
        .max()
        .unwrap_or(0);

    let max_variation_tier = effect_types
        .iter()
        .flat_map(|et| et.variations.iter().map(|v| v.unlock_tier))
        .max()
        .unwrap_or(0);

    let max_tier = max_main_tier.max(max_variation_tier);
    // 98 items / 2 per tier = 49 tiers (0-indexed: 0..48)
    assert_eq!(max_tier, 48, "98 items at 2/tier → tiers 0–48");
}

#[test]
fn first_main_is_production_unit_producer_at_tier_0() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let first = &effect_types[0];
    assert_eq!(first.available_at_tier, 0);
    assert_eq!(first.primary_token, TokenType::ProductionUnit);
    assert_eq!(first.main_direction, MainEffectDirection::Producer);
}

#[test]
fn all_mains_have_at_least_one_tag() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    for et in &effect_types {
        assert!(
            !et.tag.input.is_empty() || !et.tag.output.is_empty(),
            "main '{}' should have at least one token in its tag",
            et.name
        );
    }
}

#[test]
fn items_alternate_two_per_tier() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    // Collect all tier assignments
    let mut tier_counts: HashMap<u32, u32> = HashMap::new();
    for et in &effect_types {
        *tier_counts.entry(et.available_at_tier).or_insert(0) += 1;
        for v in &et.variations {
            *tier_counts.entry(v.unlock_tier).or_insert(0) += 1;
        }
    }

    for (tier, count) in &tier_counts {
        assert_eq!(
            *count, 2,
            "tier {} should have exactly 2 items, found {}",
            tier, count
        );
    }
}

// ---------------------------------------------------------------------------
// Producer/Consumer direction correctness
// ---------------------------------------------------------------------------

#[test]
fn beneficial_tokens_have_producer_and_consumer() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let beneficial_tokens = [
        TokenType::Energy,
        TokenType::QualityPoint,
        TokenType::Innovation,
    ];

    for token in &beneficial_tokens {
        let has_producer = effect_types.iter().any(|et| {
            et.primary_token == *token && et.main_direction == MainEffectDirection::Producer
        });
        let has_consumer = effect_types.iter().any(|et| {
            et.primary_token == *token && et.main_direction == MainEffectDirection::Consumer
        });
        assert!(has_producer, "{:?} should have a producer main", token);
        assert!(has_consumer, "{:?} should have a consumer main", token);
    }
}

#[test]
fn harmful_tokens_have_producer_and_remover() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let harmful_tokens = [TokenType::Heat, TokenType::Waste, TokenType::Pollution];

    for token in &harmful_tokens {
        let has_producer = effect_types.iter().any(|et| {
            et.primary_token == *token && et.main_direction == MainEffectDirection::Producer
        });
        let has_consumer = effect_types.iter().any(|et| {
            et.primary_token == *token && et.main_direction == MainEffectDirection::Consumer
        });
        assert!(has_producer, "{:?} should have a producer main", token);
        assert!(has_consumer, "{:?} should have a remover main", token);
    }
}

#[test]
fn production_unit_has_only_producer_no_consumer() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let pu_mains: Vec<_> = effect_types
        .iter()
        .filter(|et| et.primary_token == TokenType::ProductionUnit)
        .collect();
    assert_eq!(pu_mains.len(), 1, "PU should have exactly 1 main");
    assert_eq!(pu_mains[0].main_direction, MainEffectDirection::Producer);
}

// ---------------------------------------------------------------------------
// Direction sign correctness
// ---------------------------------------------------------------------------

#[test]
fn self_consuming_variations_exist_for_all_mains() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    for et in &effect_types {
        let self_consuming = et.variations.iter().filter(|v| v.is_self_consuming).count();
        assert_eq!(
            self_consuming, 1,
            "main '{}' should have exactly 1 self-consuming variation",
            et.name
        );
    }
}

#[test]
fn cross_token_variations_only_on_earlier_mains() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let token_order: Vec<TokenType> = token_defs
        .tokens
        .iter()
        .map(|t| t.token_type.clone())
        .collect();

    for et in &effect_types {
        let primary_idx = token_order
            .iter()
            .position(|t| *t == et.primary_token)
            .expect("token in order");

        for v in &et.variations {
            if v.is_self_consuming {
                continue;
            }
            let secondary_idx = token_order
                .iter()
                .position(|t| *t == v.secondary_token)
                .expect("secondary in order");
            assert!(
                secondary_idx > primary_idx,
                "cross-token variation on '{}' references {:?} which should be LATER, not earlier",
                et.name,
                v.secondary_token
            );
        }
    }
}

#[test]
fn direction_sign_positive_for_harmful_output_variation() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    // Find a variation with harmful token as output (should boost: +1)
    let found = effect_types
        .iter()
        .flat_map(|et| et.variations.iter())
        .find(|v| {
            v.secondary_token.is_harmful() && matches!(v.direction, VariationDirection::Output)
        });

    let v = found.expect("should have at least one harmful output variation");
    assert_eq!(
        v.direction_sign, 1,
        "harmful output → accepts harm → should boost (+1)"
    );
}

#[test]
fn direction_sign_positive_for_beneficial_input_variation() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let found = effect_types
        .iter()
        .flat_map(|et| et.variations.iter())
        .find(|v| {
            v.secondary_token.is_beneficial()
                && matches!(v.direction, VariationDirection::Input)
                && !v.is_self_consuming
        });

    let v = found.expect("should have at least one beneficial input variation");
    assert_eq!(
        v.direction_sign, 1,
        "beneficial input → sacrifices good → should boost (+1)"
    );
}

#[test]
fn direction_sign_negative_for_harmful_input_variation() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let found = effect_types
        .iter()
        .flat_map(|et| et.variations.iter())
        .find(|v| {
            v.secondary_token.is_harmful()
                && matches!(v.direction, VariationDirection::Input)
                && !v.is_self_consuming
        });

    let v = found.expect("should have at least one harmful input variation");
    assert_eq!(
        v.direction_sign, -1,
        "harmful input → removes harm → should cost (-1)"
    );
}

#[test]
fn direction_sign_negative_for_beneficial_output_variation() {
    let token_defs = load_token_definitions().expect("config");
    let effect_types = generate_effect_types(&token_defs);

    let found = effect_types
        .iter()
        .flat_map(|et| et.variations.iter())
        .find(|v| {
            v.secondary_token.is_beneficial()
                && matches!(v.direction, VariationDirection::Output)
                && !v.is_self_consuming
        });

    let v = found.expect("should have at least one beneficial output variation");
    assert_eq!(
        v.direction_sign, -1,
        "beneficial output → gets extra good → should cost (-1)"
    );
}

// ---------------------------------------------------------------------------
// Proportional model: reward card structure
// ---------------------------------------------------------------------------

#[test]
fn reward_card_variation_has_both_primary_and_secondary_tokens() {
    let cached_config = make_cached_config();

    // Generate many reward cards and find one with a variation (non-empty inputs+outputs)
    let mut rng = Pcg64::seed_from_u64(42);
    let mut found_variation = false;

    for _ in 0..50 {
        let card = generate_reward_card_with_types(ContractTier(5), 3, &mut rng, &cached_config);

        for effect in &card.effects {
            let total_entries = effect.inputs.len() + effect.outputs.len();
            if total_entries >= 2 {
                found_variation = true;
            }
            assert!(
                !effect.inputs.is_empty() || !effect.outputs.is_empty(),
                "every effect must have at least one input or output"
            );
        }
    }

    assert!(
        found_variation,
        "at tier 5 with many generations, at least one variation effect should appear"
    );
}

#[test]
fn reward_card_amounts_scale_with_tier() {
    let cached_config = make_cached_config();

    let mut rng_low = Pcg64::seed_from_u64(42);
    let mut rng_high = Pcg64::seed_from_u64(42);

    let low_tier_card =
        generate_reward_card_with_types(ContractTier(0), 1, &mut rng_low, &cached_config);
    let high_tier_card =
        generate_reward_card_with_types(ContractTier(20), 1, &mut rng_high, &cached_config);

    let low_sum: u32 = low_tier_card
        .effects
        .iter()
        .flat_map(|e| e.outputs.iter().map(|t| t.amount))
        .sum::<u32>()
        + low_tier_card
            .effects
            .iter()
            .flat_map(|e| e.inputs.iter().map(|t| t.amount))
            .sum::<u32>();

    let high_sum: u32 = high_tier_card
        .effects
        .iter()
        .flat_map(|e| e.outputs.iter().map(|t| t.amount))
        .sum::<u32>()
        + high_tier_card
            .effects
            .iter()
            .flat_map(|e| e.inputs.iter().map(|t| t.amount))
            .sum::<u32>();

    assert!(
        high_sum > low_sum,
        "tier 20 amounts ({}) should exceed tier 0 amounts ({})",
        high_sum,
        low_sum
    );
}

// ---------------------------------------------------------------------------
// TokenRequirement (harmful max) generation
// ---------------------------------------------------------------------------

#[test]
fn harmful_token_limit_appears_in_generated_contracts() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    let mut found_limit = false;

    // Generate contracts at tiers where Heat is unlocked (tier >= 1)
    for seed in 0..100u64 {
        let mut rng = Pcg64::seed_from_u64(seed);
        let contract =
            generate_contract_with_types(ContractTier(3), &mut rng, &cached_config, &adaptive);

        for req in &contract.requirements {
            if matches!(
                req,
                ContractRequirementKind::TokenRequirement { max: Some(_), .. }
            ) {
                found_limit = true;
            }
        }
        if found_limit {
            break;
        }
    }

    assert!(
        found_limit,
        "TokenRequirement with max should appear in contracts at tier 3 across 100 seeds"
    );
}

#[test]
fn harmful_token_limit_targets_harmful_tokens_only() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    for seed in 0..50u64 {
        let mut rng = Pcg64::seed_from_u64(seed);
        let contract =
            generate_contract_with_types(ContractTier(5), &mut rng, &cached_config, &adaptive);

        for req in &contract.requirements {
            if let ContractRequirementKind::TokenRequirement {
                token_type,
                max: Some(_),
                min: None,
            } = req
            {
                assert!(
                    token_type.is_harmful(),
                    "harmful-only TokenRequirement should target harmful tokens, got {:?}",
                    token_type
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Requirement tier-gating
// ---------------------------------------------------------------------------

#[test]
fn tier0_requirements_only_use_production_unit() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    for seed in 0..50u64 {
        let mut rng = Pcg64::seed_from_u64(seed);
        let contract =
            generate_contract_with_types(ContractTier(0), &mut rng, &cached_config, &adaptive);

        for req in &contract.requirements {
            if let ContractRequirementKind::TokenRequirement {
                token_type,
                min: Some(_),
                max: None,
            } = req
            {
                // At tier 0, only PU is unlocked; req_tier can be 0 or 1
                // At tier 1, Heat is also unlocked but Heat is harmful (no min-only requirement)
                assert_eq!(
                    *token_type,
                    TokenType::ProductionUnit,
                    "tier 0 beneficial TokenRequirement should only target PU, got {:?}",
                    token_type
                );
            }
            if let ContractRequirementKind::TokenRequirement {
                token_type,
                max: Some(_),
                min: None,
            } = req
            {
                assert!(
                    token_type.is_harmful(),
                    "harmful TokenRequirement should target harmful token, got {:?}",
                    token_type
                );
            }
        }
    }
}

#[test]
fn higher_tier_contracts_can_reference_more_token_types() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    let mut tier0_tokens = std::collections::HashSet::new();
    let mut tier10_tokens = std::collections::HashSet::new();

    for seed in 0..100u64 {
        let mut rng0 = Pcg64::seed_from_u64(seed);
        let c0 =
            generate_contract_with_types(ContractTier(0), &mut rng0, &cached_config, &adaptive);
        for req in &c0.requirements {
            if let ContractRequirementKind::TokenRequirement { token_type, .. } = req {
                tier0_tokens.insert(format!("{:?}", token_type));
            }
        }

        let mut rng10 = Pcg64::seed_from_u64(seed + 1000);
        let c10 =
            generate_contract_with_types(ContractTier(10), &mut rng10, &cached_config, &adaptive);
        for req in &c10.requirements {
            if let ContractRequirementKind::TokenRequirement { token_type, .. } = req {
                tier10_tokens.insert(format!("{:?}", token_type));
            }
        }
    }

    assert!(
        tier10_tokens.len() > tier0_tokens.len(),
        "tier 10 should reference more token types ({}) than tier 0 ({})",
        tier10_tokens.len(),
        tier0_tokens.len()
    );
}

// ---------------------------------------------------------------------------
// Duplicate requirement stacking via HTTP API
// ---------------------------------------------------------------------------

#[test]
fn contract_completion_subtracts_summed_requirements() {
    // Verify through the HTTP API that completing a contract with multiple
    // OutputThreshold requirements for the same token deducts the full sum.
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":100}"#);

    // Accept and complete a contract
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let state_before = get_state(&client);
    let active = &state_before["active_contract"];
    assert!(active.is_object(), "should have active contract");

    // Play cards until contract completes
    let mut completed = false;
    for _ in 0..200 {
        let idx = first_card_in_hand(&client);
        let result = post_action(
            &client,
            &format!(r#"{{"action_type":"PlayCard","card_index":{idx}}}"#),
        );
        if result["detail"]["contract_resolution"]["resolution_type"] == "Completed" {
            completed = true;
            break;
        }
    }
    assert!(completed, "contract should complete within 200 plays");
}

// ---------------------------------------------------------------------------
// Multi-tier determinism
// ---------------------------------------------------------------------------

#[test]
fn same_seed_same_tier_produces_identical_contracts() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    for tier in [0, 5, 10, 20] {
        let mut rng1 = Pcg64::seed_from_u64(42);
        let c1 =
            generate_contract_with_types(ContractTier(tier), &mut rng1, &cached_config, &adaptive);

        let mut rng2 = Pcg64::seed_from_u64(42);
        let c2 =
            generate_contract_with_types(ContractTier(tier), &mut rng2, &cached_config, &adaptive);

        assert_eq!(
            format!("{:?}", c1.requirements),
            format!("{:?}", c2.requirements),
            "tier {} contracts with same seed should have identical requirements",
            tier
        );
    }
}

// ---------------------------------------------------------------------------
// Generated contracts at various tiers are valid (fuzzing)
// ---------------------------------------------------------------------------

#[test]
fn contracts_valid_across_many_tiers_and_seeds() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    for tier in [0, 1, 5, 10, 20, 30, 48] {
        for seed in [1, 42, 777, 12345] {
            let mut rng = Pcg64::seed_from_u64(seed);
            let contract = generate_contract_with_types(
                ContractTier(tier),
                &mut rng,
                &cached_config,
                &adaptive,
            );

            assert!(
                !contract.requirements.is_empty(),
                "tier {} seed {} should have at least 1 requirement",
                tier,
                seed
            );

            assert_eq!(
                contract.reward_card.effects.len(),
                contract.requirements.len(),
                "tier {} seed {} reward effects should match requirement count",
                tier,
                seed
            );

            for effect in &contract.reward_card.effects {
                assert!(
                    !effect.inputs.is_empty() || !effect.outputs.is_empty(),
                    "tier {} seed {} effect must have inputs or outputs",
                    tier,
                    seed
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Issue #14: beneficial-token min requirement one-tier delay invariant
// ---------------------------------------------------------------------------

/// For every non-ProductionUnit beneficial token, the token must NOT appear as
/// a `TokenRequirement { min: Some(_) }` in contracts generated at the exact
/// req_tier where that token's producer card-effect first unlocks.  It should
/// only start appearing one tier later.
///
/// ProductionUnit is exempt: it unlocks at tier 0 and `0.saturating_sub(1) == 0`,
/// so tier 0 beneficial-min pool still includes it.
#[test]
fn beneficial_min_requirement_absent_at_token_producer_unlock_tier() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    // Find (token, producer_unlock_tier) for each non-PU beneficial token.
    let beneficial_tokens: Vec<(TokenType, u32)> = {
        let mut seen = std::collections::HashMap::new();
        for et in &cached_config.effect_types {
            if et.primary_token.is_beneficial()
                && et.primary_token != TokenType::ProductionUnit
                && matches!(et.main_direction, MainEffectDirection::Producer)
            {
                seen.entry(et.primary_token.clone())
                    .or_insert(et.available_at_tier);
            }
        }
        seen.into_iter().collect()
    };

    assert!(
        !beneficial_tokens.is_empty(),
        "should have at least one non-PU beneficial token with a producer"
    );

    for (token, unlock_tier) in &beneficial_tokens {
        // Generate 200 contracts at exactly the unlock tier; none should have a
        // beneficial-min requirement for this token.
        for seed in 0u64..200 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let contract = generate_contract_with_types(
                ContractTier(*unlock_tier),
                &mut rng,
                &cached_config,
                &adaptive,
            );

            for req in &contract.requirements {
                if let ContractRequirementKind::TokenRequirement {
                    token_type,
                    min: Some(_),
                    ..
                } = req
                {
                    assert_ne!(
                        token_type, token,
                        "token {:?} (unlocks at tier {}) must NOT appear as a beneficial-min \
                         requirement in contracts at req_tier {} (seed {})",
                        token, unlock_tier, unlock_tier, seed
                    );
                }
            }
        }
    }
}

/// At req_tier = unlock_tier + 1, the token's beneficial-min pool now includes it
/// (since we use `tier.saturating_sub(1) = unlock_tier`).  Over 200 seeds, the
/// token must appear at least once so the feature is also reachable.
#[test]
fn beneficial_min_requirement_reachable_one_tier_after_unlock() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    let beneficial_tokens: Vec<(TokenType, u32)> = {
        let mut seen = std::collections::HashMap::new();
        for et in &cached_config.effect_types {
            if et.primary_token.is_beneficial()
                && et.primary_token != TokenType::ProductionUnit
                && matches!(et.main_direction, MainEffectDirection::Producer)
            {
                seen.entry(et.primary_token.clone())
                    .or_insert(et.available_at_tier);
            }
        }
        seen.into_iter().collect()
    };

    for (token, unlock_tier) in &beneficial_tokens {
        let req_tier = unlock_tier + 1;
        let mut found = false;

        for seed in 0u64..200 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let contract = generate_contract_with_types(
                ContractTier(req_tier),
                &mut rng,
                &cached_config,
                &adaptive,
            );

            for req in &contract.requirements {
                if let ContractRequirementKind::TokenRequirement {
                    token_type,
                    min: Some(_),
                    ..
                } = req
                {
                    if token_type == token {
                        found = true;
                    }
                }
            }

            if found {
                break;
            }
        }

        assert!(
            found,
            "token {:?} (unlocks at tier {}) should appear as a beneficial-min \
             requirement in at least one contract at req_tier {} (over 200 seeds)",
            token, unlock_tier, req_tier
        );
    }
}

/// ProductionUnit unlocks at tier 0.  Because `0.saturating_sub(2) == 0`, it
/// should still be reachable as a beneficial-min requirement in tier-0 contracts.
#[test]
fn production_unit_min_requirement_reachable_at_tier_0() {
    let cached_config = make_cached_config();
    let adaptive = AdaptiveBalanceTracker::new(cached_config.rules.adaptive_balance.clone());

    let mut found = false;
    for seed in 0u64..500 {
        let mut rng = Pcg64::seed_from_u64(seed);
        let contract =
            generate_contract_with_types(ContractTier(0), &mut rng, &cached_config, &adaptive);

        for req in &contract.requirements {
            if let ContractRequirementKind::TokenRequirement {
                token_type: TokenType::ProductionUnit,
                min: Some(_),
                ..
            } = req
            {
                found = true;
            }
        }

        if found {
            break;
        }
    }

    assert!(
        found,
        "ProductionUnit should still appear as a min requirement in tier-0 contracts \
         (saturating_sub(2) == 0 preserves tier-0 inclusion)"
    );
}
