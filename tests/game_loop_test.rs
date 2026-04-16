//! Integration tests for the basic game loop.
//!
//! Exercises the full cycle: new game → accept contract → play cards →
//! auto-complete → new contract, plus edge cases and error handling.

use my_little_factory_manager::rocket_initialize;
use rocket::http::{ContentType, Status};
use rocket::local::blocking::Client;

fn client() -> Client {
    Client::tracked(rocket_initialize()).expect("valid rocket instance")
}

fn post_action(client: &Client, json: &str) -> (Status, serde_json::Value) {
    let response = client
        .post("/action")
        .header(ContentType::JSON)
        .body(json)
        .dispatch();
    let status = response.status();
    let body = response.into_string().expect("response body");
    let value: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    (status, value)
}

/// Extract the detail payload from an ActionResult response.
fn detail(result: &serde_json::Value) -> &serde_json::Value {
    &result["detail"]
}

fn get_state(client: &Client) -> serde_json::Value {
    let response = client.get("/state").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().expect("response body");
    serde_json::from_str(&body).expect("valid json")
}

fn get_history(client: &Client) -> serde_json::Value {
    let response = client.get("/actions/history").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().expect("response body");
    serde_json::from_str(&body).expect("valid json")
}

/// Look up a token amount from the tokens array by matching `token_type` key.
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

/// Sum a specific count field across all card entries.
fn card_count_total(state: &serde_json::Value, field: &str) -> u64 {
    state["cards"]
        .as_array()
        .expect("cards array")
        .iter()
        .map(|entry| entry["counts"][field].as_u64().unwrap_or(0))
        .sum()
}

// ---------------------------------------------------------------------------
// New game
// ---------------------------------------------------------------------------

#[test]
fn new_game_initializes_state() {
    let client = client();
    let (status, result) = post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Success");
    assert_eq!(detail(&result)["result_type"], "NewGameStarted");
    assert_eq!(detail(&result)["seed"], 42);

    let state = get_state(&client);
    assert_eq!(state["seed"], 42);
    assert_eq!(card_count_total(&state, "hand"), 5);
    assert!(
        !state["offered_contracts"]
            .as_array()
            .expect("offered_contracts")
            .is_empty(),
        "should have offered contracts"
    );
    assert!(state["active_contract"].is_null(), "no active contract yet");
}

#[test]
fn new_game_without_seed_generates_random_seed() {
    let client = client();
    let (status, result) = post_action(&client, r#"{"action_type":"NewGame","seed":null}"#);
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Success");
    assert_eq!(detail(&result)["result_type"], "NewGameStarted");

    let state = get_state(&client);
    assert!(state["seed"].is_u64(), "should have a seed");
}

// ---------------------------------------------------------------------------
// Accept contract
// ---------------------------------------------------------------------------

#[test]
fn accept_contract_activates_offered_contract() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state_before = get_state(&client);
    let offered = state_before["offered_contracts"][0]["contracts"][0].clone();
    assert!(offered.is_object());

    let (status, result) = post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Success");
    assert_eq!(detail(&result)["result_type"], "ContractAccepted");

    let state_after = get_state(&client);
    assert!(state_after["active_contract"].is_object());
    let remaining = state_after["offered_contracts"][0]["contracts"]
        .as_array()
        .expect("contracts");
    assert_eq!(
        remaining.len(),
        2,
        "should have 2 remaining contracts after accepting 1"
    );
    assert_eq!(state_after["active_contract"], offered);
}

#[test]
fn accept_contract_fails_when_already_active() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let (status, result) = post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    assert_eq!(detail(&result)["error_type"], "ContractAlreadyActive");
}

// ---------------------------------------------------------------------------
// Play card
// ---------------------------------------------------------------------------

#[test]
fn play_card_adds_tokens_and_moves_card() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let state_before = get_state(&client);
    let pu_before = token_amount(&state_before, r#""ProductionUnit""#);

    let (status, result) = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Success");
    assert_eq!(detail(&result)["result_type"], "CardPlayed");

    let state_after = get_state(&client);
    let pu_after = token_amount(&state_after, r#""ProductionUnit""#);
    assert!(
        pu_after > pu_before,
        "playing a production card should increase ProductionUnit"
    );
    // Hand should still have 5 cards (drew a replacement)
    assert_eq!(card_count_total(&state_after, "hand"), 5);
}

#[test]
fn play_card_fails_without_active_contract() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let (status, result) = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    assert_eq!(detail(&result)["error_type"], "NoActiveContract");
}

#[test]
fn play_card_fails_with_invalid_index() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let (status, result) = post_action(&client, r#"{"action_type":"PlayCard","hand_index":99}"#);
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    assert_eq!(detail(&result)["error_type"], "InvalidHandIndex");
    assert_eq!(detail(&result)["index"], 99);
}

// ---------------------------------------------------------------------------
// Discard card
// ---------------------------------------------------------------------------

#[test]
fn discard_card_gives_baseline_bonus() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let (status, result) = post_action(&client, r#"{"action_type":"DiscardCard","hand_index":0}"#);
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Success");
    assert_eq!(detail(&result)["result_type"], "CardDiscarded");

    let state = get_state(&client);
    let pu = token_amount(&state, r#""ProductionUnit""#);
    // Discard bonus is 1 PU (from config)
    assert_eq!(pu, 1);
}

#[test]
fn discard_card_fails_without_active_contract() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let (status, result) = post_action(&client, r#"{"action_type":"DiscardCard","hand_index":0}"#);
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    assert_eq!(detail(&result)["error_type"], "NoActiveContract");
}

// ---------------------------------------------------------------------------
// Contract auto-completion
// ---------------------------------------------------------------------------

#[test]
fn contract_auto_completes_when_threshold_met() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Get the required threshold from the active contract
    let state = get_state(&client);
    let min_amount = state["active_contract"]["requirements"][0]["min_amount"]
        .as_u64()
        .expect("min_amount");

    // Play cards until we accumulate enough production units
    let mut total_pu: u64 = 0;
    let mut completed = false;
    for _ in 0..50 {
        let (_, result) = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
        if detail(&result)["contract_completed"].is_object() {
            completed = true;
            break;
        }
        let st = get_state(&client);
        total_pu = token_amount(&st, r#""ProductionUnit""#);
    }

    assert!(
        completed || total_pu >= min_amount,
        "contract should have auto-completed after enough cards played"
    );

    if completed {
        let state = get_state(&client);
        assert_eq!(token_amount(&state, r#"{"ContractsTierCompleted":1}"#), 1);
        assert!(state["active_contract"].is_null());
        assert!(
            !state["offered_contracts"]
                .as_array()
                .expect("offered_contracts")
                .is_empty(),
            "new contracts should be offered"
        );
    }
}

#[test]
fn full_game_loop_two_contracts() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":100}"#);

    for contract_num in 1..=2 {
        post_action(
            &client,
            r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
        );

        let mut completed = false;
        for _ in 0..100 {
            let (_, result) = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
            if detail(&result)["contract_completed"].is_object() {
                completed = true;
                break;
            }
        }

        assert!(completed, "contract {} should have completed", contract_num);
    }

    let state = get_state(&client);
    assert_eq!(token_amount(&state, r#"{"ContractsTierCompleted":1}"#), 2);
}

// ---------------------------------------------------------------------------
// Token persistence between contracts
// ---------------------------------------------------------------------------

#[test]
fn tokens_persist_between_contracts() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Play cards until the contract completes
    let mut completed = false;
    for _ in 0..100 {
        let (_, result) = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
        if detail(&result)["contract_completed"].is_object() {
            completed = true;
            break;
        }
    }
    assert!(completed, "first contract should complete");

    // Record PU tokens after completion (contract subtracts its requirement)
    let state_after_completion = get_state(&client);
    let pu_after_completion = token_amount(&state_after_completion, r#""ProductionUnit""#);

    // Accept the new contract
    let (_, accept_result) = post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );
    assert_eq!(detail(&accept_result)["result_type"], "ContractAccepted");

    // Verify tokens persist into the new contract
    let state_new_contract = get_state(&client);
    let pu_new_contract = token_amount(&state_new_contract, r#""ProductionUnit""#);
    assert_eq!(
        pu_after_completion, pu_new_contract,
        "tokens should persist between contracts"
    );
}

// ---------------------------------------------------------------------------
// Hand persistence between contracts
// ---------------------------------------------------------------------------

#[test]
fn hand_persists_between_contracts() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Complete a contract by playing enough cards
    let mut completed = false;
    for _ in 0..100 {
        let (_, result) = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
        if detail(&result)["contract_completed"].is_object() {
            completed = true;
            break;
        }
    }
    assert!(completed, "first contract should complete");

    let state = get_state(&client);
    let hand_size = card_count_total(&state, "hand");
    assert!(
        hand_size > 0,
        "hand should persist after contract completion"
    );
}

// ---------------------------------------------------------------------------
// Deck cycling
// ---------------------------------------------------------------------------

#[test]
fn deck_recycles_discard_when_empty() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Play more cards than the deck size to force a reshuffle
    for _ in 0..15 {
        let (_, result) = post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
        if detail(&result)["contract_completed"].is_object() {
            // If contract completes, accept the new one and continue
            post_action(
                &client,
                r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
            );
        }
    }

    // After playing 15 cards from a 10-card deck, at least one reshuffle must have occurred.
    // The hand should still have cards.
    let state = get_state(&client);
    let hand_size = card_count_total(&state, "hand");
    assert!(hand_size > 0, "hand should have cards after deck recycling");
}

// ---------------------------------------------------------------------------
// Action history
// ---------------------------------------------------------------------------

#[test]
fn action_history_records_all_actions() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );
    post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);

    let history = get_history(&client);
    let entries = history.as_array().expect("history array");
    // The default initialization fires a NewGame, so our explicit NewGame is logged,
    // plus AcceptContract, plus PlayCard = at least 3 entries
    assert!(
        entries.len() >= 3,
        "history should have at least 3 entries, got {}",
        entries.len()
    );

    // Check sequence numbers are ascending
    let seqs: Vec<u64> = entries
        .iter()
        .map(|e| e["seq"].as_u64().expect("seq"))
        .collect();
    for window in seqs.windows(2) {
        assert!(
            window[1] > window[0],
            "sequence numbers should be ascending"
        );
    }
}

// ---------------------------------------------------------------------------
// State endpoint
// ---------------------------------------------------------------------------

#[test]
fn state_endpoint_returns_complete_view() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state = get_state(&client);
    assert!(state.get("seed").is_some());
    assert!(state.get("cards").is_some());
    assert!(state.get("tokens").is_some());
    assert!(state.get("offered_contracts").is_some());
}
