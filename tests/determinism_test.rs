//! Integration tests for deterministic game replay.
//!
//! Verifies that the same seed + same actions = identical game state,
//! and that replaying an action log reproduces the same outcome.

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

fn get_history(client: &Client) -> Vec<serde_json::Value> {
    let response = client.get("/actions/history").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().expect("response body");
    let entries: Vec<serde_json::Value> = serde_json::from_str(&body).expect("valid json");
    entries
}

/// Run a fixed sequence of actions and return the final state.
fn run_game_sequence(client: &Client, seed: u64) -> serde_json::Value {
    post_action(
        client,
        &format!(r#"{{"action_type":"NewGame","seed":{seed}}}"#),
    );
    post_action(
        client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Play several cards
    for _ in 0..5 {
        post_action(client, r#"{"action_type":"PlayCard","hand_index":0}"#);
    }

    // Discard one card
    post_action(client, r#"{"action_type":"DiscardCard","hand_index":0}"#);

    get_state(client)
}

// ---------------------------------------------------------------------------
// Same seed produces identical state
// ---------------------------------------------------------------------------

#[test]
fn same_seed_same_actions_produce_identical_state() {
    let client = client();

    let state1 = run_game_sequence(&client, 12345);
    let state2 = run_game_sequence(&client, 12345);

    assert_eq!(
        state1, state2,
        "same seed + same actions must produce identical state"
    );
}

#[test]
fn different_seeds_produce_different_state() {
    let client = client();

    let state1 = run_game_sequence(&client, 111);
    let state2 = run_game_sequence(&client, 222);

    // The states should differ (different seeds → different shuffle → different hands)
    // We check tokens or hand as the distinguishing factor
    let hand1 = &state1["hand"];
    let hand2 = &state2["hand"];
    let tokens1 = &state1["tokens"];
    let tokens2 = &state2["tokens"];

    assert!(
        hand1 != hand2 || tokens1 != tokens2,
        "different seeds should produce different game states"
    );
}

// ---------------------------------------------------------------------------
// Replay from action log
// ---------------------------------------------------------------------------

#[test]
fn replay_action_log_reproduces_state() {
    let client = client();

    // Play a game
    let state_original = run_game_sequence(&client, 99999);
    let history = get_history(&client);

    // Start a fresh game and replay all recorded actions
    // The first action in history should be a NewGame with the same seed
    for entry in &history {
        let action = &entry["action"];
        let action_json = serde_json::to_string(action).expect("serialize action");
        post_action(&client, &action_json);
    }

    let state_replayed = get_state(&client);

    assert_eq!(
        state_original, state_replayed,
        "replaying the action log must reproduce the identical state"
    );
}

// ---------------------------------------------------------------------------
// Multiple games with reset
// ---------------------------------------------------------------------------

#[test]
fn new_game_fully_resets_state() {
    let client = client();

    // Play some actions
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );
    post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);

    let state_mid = get_state(&client);
    // Verify game has progressed — turn_count > 0
    let progressed = state_mid["turn_count"].as_u64().expect("turn_count") > 0;
    assert!(progressed, "game should have progressed");

    // Start a completely new game
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state_fresh = get_state(&client);
    assert_eq!(state_fresh["turn_count"], 0);
    assert_eq!(state_fresh["seed"], 42);
}

// ---------------------------------------------------------------------------
// Deterministic contract generation
// ---------------------------------------------------------------------------

#[test]
fn same_seed_generates_same_contracts() {
    let client = client();

    // First run
    post_action(&client, r#"{"action_type":"NewGame","seed":777}"#);
    let state1 = get_state(&client);
    let offered1 = state1["offered_contracts"].clone();

    // Second run with same seed
    post_action(&client, r#"{"action_type":"NewGame","seed":777}"#);
    let state2 = get_state(&client);
    let offered2 = state2["offered_contracts"].clone();

    assert_eq!(
        offered1, offered2,
        "same seed should generate the same first contract"
    );
}

// ---------------------------------------------------------------------------
// Deterministic deck shuffle
// ---------------------------------------------------------------------------

#[test]
fn same_seed_deals_same_hand() {
    let client = client();

    post_action(&client, r#"{"action_type":"NewGame","seed":555}"#);
    let state1 = get_state(&client);
    let hand1 = state1["hand"].clone();

    post_action(&client, r#"{"action_type":"NewGame","seed":555}"#);
    let state2 = get_state(&client);
    let hand2 = state2["hand"].clone();

    assert_eq!(hand1, hand2, "same seed should deal the same starting hand");
}
