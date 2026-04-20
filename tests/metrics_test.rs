//! Integration tests for the `GET /metrics` endpoint.
//!
//! Verifies that gameplay statistics accumulate correctly through
//! card plays, contract completions, discards, and deckbuilding actions,
//! and that metrics reset on NewGame.

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

fn get_metrics(client: &Client) -> serde_json::Value {
    let response = client.get("/metrics").dispatch();
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

/// Start a game, accept contract at tier 0 index 0, then play cards until
/// contract completes. Returns the number of cards played+discarded.
fn complete_one_contract(client: &Client) -> u64 {
    post_action(
        client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let mut cards_used: u64 = 0;
    loop {
        let state = get_state(client);
        if state["active_contract"].is_null() {
            break;
        }
        let (_, result) = post_action(client, r#"{"action_type":"PlayCard","hand_index":0}"#);
        cards_used += 1;

        // If card play failed (insufficient tokens), discard instead
        if result["outcome"] == "Error" {
            post_action(client, r#"{"action_type":"DiscardCard","hand_index":0}"#);
            // Don't double count — the failed PlayCard didn't actually play
            // but the DiscardCard did use a card
        }

        // Safety: break after too many iterations
        if cards_used > 200 {
            panic!("contract did not complete within 200 card plays");
        }
    }
    cards_used
}

// ---------------------------------------------------------------------------
// Basic endpoint tests
// ---------------------------------------------------------------------------

#[test]
fn metrics_endpoint_returns_ok() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    let metrics = get_metrics(&client);
    assert_eq!(metrics["total_contracts_completed"], 0);
    assert_eq!(metrics["total_cards_played"], 0);
    assert_eq!(metrics["total_cards_discarded"], 0);
    assert_eq!(metrics["current_streak"], 0);
    assert_eq!(metrics["best_streak"], 0);
    assert_eq!(metrics["total_cards_replaced"], 0);
    assert_eq!(metrics["strategy_diversity_score"], 0.0);
    assert!(metrics["dominant_strategy"].is_null());
    assert!(metrics["avg_cards_per_contract"].is_null());
}

#[test]
fn metrics_structure_has_expected_fields() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    let metrics = get_metrics(&client);

    assert!(metrics["total_contracts_completed"].is_u64());
    assert!(metrics["contracts_per_tier"].is_array());
    assert!(metrics["total_cards_played"].is_u64());
    assert!(metrics["total_cards_discarded"].is_u64());
    assert!(metrics["cards_per_tag"].is_array());
    assert!(metrics["token_flow"].is_array());
    assert!(metrics["current_streak"].is_u64());
    assert!(metrics["best_streak"].is_u64());
    assert!(metrics["strategy_diversity_score"].is_f64());
    assert!(metrics["total_cards_replaced"].is_u64());
}

// ---------------------------------------------------------------------------
// Card play tracking
// ---------------------------------------------------------------------------

#[test]
fn metrics_track_cards_played() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let before = get_metrics(&client);
    assert_eq!(before["total_cards_played"].as_u64().unwrap_or(0), 0);

    post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);

    let after = get_metrics(&client);
    assert_eq!(after["total_cards_played"].as_u64().unwrap_or(0), 1);
}

#[test]
fn metrics_track_cards_discarded() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    post_action(&client, r#"{"action_type":"DiscardCard","hand_index":0}"#);

    let metrics = get_metrics(&client);
    assert_eq!(metrics["total_cards_discarded"].as_u64().unwrap_or(0), 1);
    // Discarding should not count as "cards played"
    assert_eq!(metrics["total_cards_played"].as_u64().unwrap_or(0), 0);
}

#[test]
fn metrics_track_per_tag_counts() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Play a few cards to get tag counts
    post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
    post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);

    let metrics = get_metrics(&client);
    let tags = metrics["cards_per_tag"]
        .as_array()
        .expect("cards_per_tag array");
    assert!(!tags.is_empty(), "should have at least one tag count");

    // Verify tag entries have expected structure
    for tag_entry in tags {
        assert!(tag_entry["tag"].is_string());
        assert!(tag_entry["count"].as_u64().unwrap_or(0) > 0);
    }
}

// ---------------------------------------------------------------------------
// Contract completion tracking
// ---------------------------------------------------------------------------

#[test]
fn metrics_track_contract_completion() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    complete_one_contract(&client);

    let metrics = get_metrics(&client);
    assert_eq!(
        metrics["total_contracts_completed"].as_u64().unwrap_or(0),
        1
    );

    let tiers = metrics["contracts_per_tier"]
        .as_array()
        .expect("contracts_per_tier");
    assert!(!tiers.is_empty(), "should have tier 0 completion");
    assert_eq!(tiers[0]["tier"], 0);
    assert_eq!(tiers[0]["completed"], 1);
    assert_eq!(tiers[0]["completion_rate"], 1.0);
}

#[test]
fn metrics_avg_cards_per_contract() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    complete_one_contract(&client);

    let metrics = get_metrics(&client);
    let avg = metrics["avg_cards_per_contract"]
        .as_f64()
        .expect("avg_cards_per_contract should be present after completion");
    assert!(avg > 0.0, "should have used at least one card per contract");
}

// ---------------------------------------------------------------------------
// Token flow tracking
// ---------------------------------------------------------------------------

#[test]
fn metrics_track_token_flow() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Play a card to produce tokens
    post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);

    let metrics = get_metrics(&client);
    let flow = metrics["token_flow"].as_array().expect("token_flow array");
    assert!(
        !flow.is_empty(),
        "should have token flow after playing a card"
    );

    // At least ProductionUnit should appear (starter cards produce it)
    let pu_flow = flow
        .iter()
        .find(|f| f["token_type"] == "ProductionUnit")
        .expect("ProductionUnit should be in token flow");
    assert!(
        pu_flow["total_produced"].as_u64().unwrap_or(0) > 0,
        "should have produced production units"
    );
}

// ---------------------------------------------------------------------------
// Streak tracking
// ---------------------------------------------------------------------------

#[test]
fn metrics_track_streaks() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    complete_one_contract(&client);
    let m1 = get_metrics(&client);
    assert_eq!(m1["current_streak"].as_u64().unwrap_or(0), 1);
    assert_eq!(m1["best_streak"].as_u64().unwrap_or(0), 1);

    complete_one_contract(&client);
    let m2 = get_metrics(&client);
    assert_eq!(m2["current_streak"].as_u64().unwrap_or(0), 2);
    assert_eq!(m2["best_streak"].as_u64().unwrap_or(0), 2);
}

// ---------------------------------------------------------------------------
// NewGame reset
// ---------------------------------------------------------------------------

#[test]
fn metrics_reset_on_new_game() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    // Generate some metrics
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );
    post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);

    let before = get_metrics(&client);
    assert!(before["total_cards_played"].as_u64().unwrap_or(0) > 0);

    // Start new game — metrics should reset
    post_action(&client, r#"{"action_type":"NewGame","seed":99}"#);

    let after = get_metrics(&client);
    assert_eq!(after["total_cards_played"].as_u64().unwrap_or(0), 0);
    assert_eq!(after["total_contracts_completed"].as_u64().unwrap_or(0), 0);
    assert_eq!(after["current_streak"].as_u64().unwrap_or(0), 0);
    assert_eq!(after["best_streak"].as_u64().unwrap_or(0), 0);
}

// ---------------------------------------------------------------------------
// Strategy diversity
// ---------------------------------------------------------------------------

#[test]
fn metrics_strategy_diversity_after_plays() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Play several cards so tag counts build up
    for _ in 0..5 {
        let state = get_state(&client);
        if state["active_contract"].is_null() {
            break;
        }
        post_action(&client, r#"{"action_type":"PlayCard","hand_index":0}"#);
    }

    let metrics = get_metrics(&client);
    // After playing cards, dominant_strategy should be populated
    if metrics["total_cards_played"].as_u64().unwrap_or(0) > 0 {
        assert!(
            metrics["dominant_strategy"].is_string(),
            "should have a dominant strategy after playing cards"
        );
    }
}

// ---------------------------------------------------------------------------
// Deterministic metrics
// ---------------------------------------------------------------------------

#[test]
fn metrics_are_deterministic_across_replays() {
    let run = |seed: u64| -> serde_json::Value {
        let client = client();
        post_action(
            &client,
            &format!(r#"{{"action_type":"NewGame","seed":{seed}}}"#),
        );
        complete_one_contract(&client);
        get_metrics(&client)
    };

    let m1 = run(42);
    let m2 = run(42);

    assert_eq!(
        m1["total_contracts_completed"],
        m2["total_contracts_completed"]
    );
    assert_eq!(m1["total_cards_played"], m2["total_cards_played"]);
    assert_eq!(m1["total_cards_discarded"], m2["total_cards_discarded"]);
    assert_eq!(m1["avg_cards_per_contract"], m2["avg_cards_per_contract"]);
    assert_eq!(m1["cards_per_tag"], m2["cards_per_tag"]);
}
