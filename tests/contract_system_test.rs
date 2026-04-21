//! Integration tests for the Phase 3 contract system.
//!
//! Validates formula-based contract generation, market mechanics,
//! reward card previews, and the `/contracts/available` endpoint.

use my_little_factory_manager::rocket_initialize;
use rocket::http::{ContentType, Status};
use rocket::local::blocking::Client;

fn client() -> Client {
    Client::tracked(rocket_initialize()).expect("valid rocket instance")
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

fn get_contracts_available(client: &Client) -> serde_json::Value {
    let response = client.get("/contracts/available").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().expect("response body");
    serde_json::from_str(&body).expect("valid json")
}

fn detail(result: &serde_json::Value) -> &serde_json::Value {
    &result["detail"]
}

fn token_amount(state: &serde_json::Value, token_type_json: &str) -> u64 {
    let expected: serde_json::Value =
        serde_json::from_str(token_type_json).expect("valid token_type json");
    state["tokens"]
        .as_array()
        .expect("tokens array")
        .iter()
        .find(|entry| entry["token_type"] == expected)
        .map(|entry| entry["amount"].as_u64().unwrap_or(0))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Market structure
// ---------------------------------------------------------------------------

#[test]
fn market_has_three_tier0_contracts_on_start() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state = get_state(&client);
    let tiers = state["offered_contracts"]
        .as_array()
        .expect("offered_contracts");
    assert_eq!(tiers.len(), 1, "should have exactly 1 tier group");
    assert_eq!(tiers[0]["tier"], 0);

    let contracts = tiers[0]["contracts"].as_array().expect("contracts array");
    assert_eq!(contracts.len(), 3, "should offer 3 contracts per tier");
}

// ---------------------------------------------------------------------------
// Contract structure validation
// ---------------------------------------------------------------------------

#[test]
fn tier0_contracts_have_valid_requirements() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state = get_state(&client);
    let contracts = state["offered_contracts"][0]["contracts"]
        .as_array()
        .expect("contracts array");

    for (i, contract) in contracts.iter().enumerate() {
        let reqs = contract["requirements"]
            .as_array()
            .expect("requirements array");
        assert_eq!(
            reqs.len(),
            1,
            "tier 0 contract {} should have exactly 1 requirement",
            i
        );

        let req_type = reqs[0]["requirement_type"]
            .as_str()
            .expect("requirement_type");
        // Tier 0 requirement tier ranges from 0 to 1.
        // At tier 0: only PU TokenRequirement(min).
        // At tier 1: PU TokenRequirement(min) or Heat TokenRequirement(max).
        assert_eq!(
            req_type, "TokenRequirement",
            "contract {} should have TokenRequirement",
            i
        );
        if reqs[0]["min"].is_null() {
            // max-only = harmful token limit
            assert_eq!(
                reqs[0]["token_type"], "Heat",
                "contract {} max-only TokenRequirement should target Heat",
                i
            );
            let max_amount = reqs[0]["max"].as_u64().expect("max");
            assert!(max_amount > 0, "contract {} max should be positive", i);
        } else {
            // min-only = production threshold
            assert_eq!(
                reqs[0]["token_type"], "ProductionUnit",
                "contract {} min-only TokenRequirement should require ProductionUnit",
                i
            );
            let min_amount = reqs[0]["min"].as_u64().expect("min");
            assert!(
                (4..=15).contains(&min_amount),
                "contract {} min {} should be in [4, 15]",
                i,
                min_amount
            );
        }
    }
}

#[test]
fn tier0_reward_cards_have_effects() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state = get_state(&client);
    let contracts = state["offered_contracts"][0]["contracts"]
        .as_array()
        .expect("contracts array");

    for (i, contract) in contracts.iter().enumerate() {
        let reqs = contract["requirements"]
            .as_array()
            .expect("requirements array");
        let reward = &contract["reward_card"];
        let effects = reward["effects"].as_array().expect("effects array");

        assert_eq!(
            effects.len(),
            reqs.len(),
            "contract {} reward card should have same number of effects as requirements",
            i
        );

        let tags = reward["tags"].as_array().expect("tags array");
        assert!(
            !tags.is_empty(),
            "contract {} reward card should have at least one tag",
            i
        );

        for (j, effect) in effects.iter().enumerate() {
            let has_inputs = !effect["inputs"].as_array().expect("inputs").is_empty();
            let has_outputs = !effect["outputs"].as_array().expect("outputs").is_empty();
            assert!(
                has_inputs || has_outputs,
                "contract {} effect {} must have at least one input or output",
                i,
                j
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Market mechanics
// ---------------------------------------------------------------------------

#[test]
fn accepting_contract_reduces_market_by_one() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state_before = get_state(&client);
    let count_before = state_before["offered_contracts"][0]["contracts"]
        .as_array()
        .expect("contracts")
        .len();
    assert_eq!(count_before, 3);

    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let state_after = get_state(&client);
    let count_after = state_after["offered_contracts"][0]["contracts"]
        .as_array()
        .expect("contracts")
        .len();
    assert_eq!(
        count_after, 2,
        "market should have 2 contracts after accepting 1"
    );
}

#[test]
fn market_refills_after_contract_completion() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":100}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Complete the contract
    let mut completed = false;
    for _ in 0..100 {
        let result = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
        if detail(&result)["contract_resolution"]["resolution_type"] == "Completed" {
            completed = true;
            break;
        }
    }
    assert!(completed, "contract should complete");

    let state = get_state(&client);
    let contracts = state["offered_contracts"][0]["contracts"]
        .as_array()
        .expect("contracts");
    assert_eq!(
        contracts.len(),
        3,
        "market should refill to 3 contracts after completion"
    );
}

// ---------------------------------------------------------------------------
// Contracts available endpoint
// ---------------------------------------------------------------------------

#[test]
fn contracts_available_matches_state() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state = get_state(&client);
    let available = get_contracts_available(&client);

    assert_eq!(
        state["offered_contracts"], available,
        "GET /contracts/available should match state's offered_contracts"
    );
}

#[test]
fn contracts_available_updates_after_accept() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let available = get_contracts_available(&client);
    let contracts = available[0]["contracts"].as_array().expect("contracts");
    assert_eq!(
        contracts.len(),
        2,
        "available should reflect accepted contract"
    );
}

// ---------------------------------------------------------------------------
// Deterministic generation
// ---------------------------------------------------------------------------

#[test]
fn same_seed_generates_identical_contracts() {
    let client = client();

    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    let contracts1 = get_contracts_available(&client);

    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    let contracts2 = get_contracts_available(&client);

    assert_eq!(
        contracts1, contracts2,
        "same seed should generate identical contracts"
    );
}

#[test]
fn different_seeds_generate_different_contracts() {
    let client = client();

    post_action(&client, r#"{"action_type":"NewGame","seed":111}"#);
    let contracts1 = get_contracts_available(&client);

    post_action(&client, r#"{"action_type":"NewGame","seed":222}"#);
    let contracts2 = get_contracts_available(&client);

    assert_ne!(
        contracts1, contracts2,
        "different seeds should generate different contracts"
    );
}

// ---------------------------------------------------------------------------
// Multi-seed validation (fuzz across seeds)
// ---------------------------------------------------------------------------

#[test]
fn all_generated_contracts_are_valid_across_seeds() {
    let client = client();

    for seed in [1, 42, 100, 999, 12345, 99999] {
        post_action(
            &client,
            &format!(r#"{{"action_type":"NewGame","seed":{seed}}}"#),
        );

        let state = get_state(&client);
        let tiers = state["offered_contracts"]
            .as_array()
            .expect("offered_contracts");

        assert!(!tiers.is_empty(), "seed {} should have tier groups", seed);

        for tier_group in tiers {
            let contracts = tier_group["contracts"].as_array().expect("contracts array");
            assert_eq!(
                contracts.len(),
                3,
                "seed {} should have 3 contracts per tier",
                seed
            );

            for contract in contracts {
                let reqs = contract["requirements"].as_array().expect("requirements");
                assert!(
                    !reqs.is_empty(),
                    "seed {} contract should have requirements",
                    seed
                );

                let reward = &contract["reward_card"];
                let effects = reward["effects"].as_array().expect("effects");
                assert_eq!(
                    effects.len(),
                    reqs.len(),
                    "seed {} reward effects should match requirement count",
                    seed
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reward card is awarded on completion
// ---------------------------------------------------------------------------

#[test]
fn reward_card_added_to_shelved_on_completion() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":100}"#);

    let state_before = get_state(&client);
    let cards_before: usize = state_before["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|e| e["counts"]["shelved"].as_u64().unwrap_or(0) as usize)
        .sum();

    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let mut completed = false;
    for _ in 0..100 {
        let result = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
        if detail(&result)["contract_resolution"]["resolution_type"] == "Completed" {
            completed = true;
            break;
        }
    }
    assert!(completed, "contract should complete");

    let state_after = get_state(&client);
    let cards_after: usize = state_after["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|e| e["counts"]["shelved"].as_u64().unwrap_or(0) as usize)
        .sum();

    assert_eq!(
        cards_after,
        cards_before + 1,
        "completing a contract should add 1 reward card to shelved"
    );

    assert_eq!(
        token_amount(&state_after, r#"{"ContractsTierCompleted":0}"#),
        1
    );
}

// ---------------------------------------------------------------------------
// Config validation: token_definitions.json must produce at least one tier-0 effect
// ---------------------------------------------------------------------------

#[test]
fn generated_effect_types_have_tier0_entry() {
    let token_defs = my_little_factory_manager::config_loader::load_token_definitions()
        .expect("config must parse");
    let effect_types =
        my_little_factory_manager::contract_generation::generate_effect_types(&token_defs);

    let has_tier0 = effect_types.iter().any(|et| et.available_at_tier == 0);
    assert!(
        has_tier0,
        "generated effect types must contain at least one entry with available_at_tier == 0"
    );
}
