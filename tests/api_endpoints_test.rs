//! Integration tests for Phase 4 REST API and documentation endpoints.

use my_little_factory_manager::rocket_initialize;
use rocket::http::{ContentType, Status};
use rocket::local::blocking::Client;

fn create_client() -> Client {
    Client::tracked(rocket_initialize()).expect("valid rocket instance")
}

fn start_new_game(client: &Client) {
    client
        .post("/action")
        .header(ContentType::JSON)
        .body(r#"{"action_type": "NewGame", "seed": 42}"#)
        .dispatch();
}

fn accept_first_contract(client: &Client) {
    client
        .post("/action")
        .header(ContentType::JSON)
        .body(r#"{"action_type": "AcceptContract", "tier_index": 0, "contract_index": 0}"#)
        .dispatch();
}

fn first_card_in_hand(client: &Client) -> usize {
    let response = client.get("/state").dispatch();
    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    body["cards"]
        .as_array()
        .expect("cards array")
        .iter()
        .enumerate()
        .find(|(_, e)| e["counts"]["hand"].as_u64().unwrap_or(0) > 0)
        .map(|(i, _)| i)
        .expect("at least one card in hand")
}

// -----------------------------------------------------------------------
// GET /library/cards
// -----------------------------------------------------------------------

#[test]
fn library_cards_returns_all_cards() {
    let client = create_client();
    let response = client.get("/library/cards").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    let cards = body.as_array().expect("array");
    assert!(
        cards.len() >= 3,
        "starter deck should have at least 3 card types"
    );
}

#[test]
fn library_cards_filter_by_production_tag() {
    let client = create_client();
    let response = client.get("/library/cards?tag=Production").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    let cards = body.as_array().expect("array");
    assert!(cards.len() >= 3, "all starter cards are Production tagged");

    for card in cards {
        let tags = card["card"]["tags"].as_array().expect("tags array");
        assert!(
            tags.iter().any(|t| t.as_str() == Some("Production")),
            "filtered cards must have Production tag"
        );
    }
}

#[test]
fn library_cards_filter_by_unknown_tag_returns_empty() {
    let client = create_client();
    let response = client.get("/library/cards?tag=Nonexistent").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    let cards = body.as_array().expect("array");
    assert!(cards.is_empty(), "unknown tag should return empty result");
}

// -----------------------------------------------------------------------
// GET /player/tokens
// -----------------------------------------------------------------------

#[test]
fn player_tokens_returns_grouped_balances() {
    let client = create_client();
    let response = client.get("/player/tokens").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    assert!(
        body.get("beneficial").is_some(),
        "should have beneficial field"
    );
    assert!(body.get("harmful").is_some(), "should have harmful field");
    assert!(
        body.get("progression").is_some(),
        "should have progression field"
    );
}

#[test]
fn player_tokens_after_playing_card_shows_production() {
    let client = create_client();
    start_new_game(&client);
    accept_first_contract(&client);

    let idx = first_card_in_hand(&client);
    client
        .post("/action")
        .header(ContentType::JSON)
        .body(format!(
            r#"{{"action_type": "PlayCard", "card_index": {idx}}}"#
        ))
        .dispatch();

    let response = client.get("/player/tokens").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    let beneficial = body["beneficial"].as_array().expect("beneficial array");
    assert!(
        !beneficial.is_empty(),
        "should have beneficial tokens after playing a production card"
    );
}

// -----------------------------------------------------------------------
// GET /contracts/active
// -----------------------------------------------------------------------

#[test]
fn contracts_active_returns_null_when_no_contract() {
    let client = create_client();
    let response = client.get("/contracts/active").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().expect("body");
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    assert!(json.is_null(), "should be null when no active contract");
}

#[test]
fn contracts_active_returns_contract_when_accepted() {
    let client = create_client();
    start_new_game(&client);
    accept_first_contract(&client);

    let response = client.get("/contracts/active").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    assert!(body.is_object(), "should return contract object");
    assert!(body.get("tier").is_some(), "contract should have tier");
    assert!(
        body.get("requirements").is_some(),
        "contract should have requirements"
    );
    assert!(
        body.get("reward_card").is_some(),
        "contract should have reward_card"
    );
}

// -----------------------------------------------------------------------
// GET /actions/possible
// -----------------------------------------------------------------------

#[test]
fn actions_possible_without_contract_shows_accept() {
    let client = create_client();
    start_new_game(&client);

    let response = client.get("/actions/possible").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    let actions = body.as_array().expect("array");

    let has_new_game = actions
        .iter()
        .any(|a| a["action_type"].as_str() == Some("NewGame"));
    assert!(has_new_game, "NewGame should always be possible");

    let has_accept = actions
        .iter()
        .any(|a| a["action_type"].as_str() == Some("AcceptContract"));
    assert!(
        has_accept,
        "AcceptContract should be possible when no active contract"
    );

    let has_play = actions
        .iter()
        .any(|a| a["action_type"].as_str() == Some("PlayCard"));
    assert!(
        !has_play,
        "PlayCard should not be possible without active contract"
    );
}

#[test]
fn actions_possible_with_contract_shows_play_and_discard() {
    let client = create_client();
    start_new_game(&client);
    accept_first_contract(&client);

    let response = client.get("/actions/possible").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    let actions = body.as_array().expect("array");

    let has_play = actions
        .iter()
        .any(|a| a["action_type"].as_str() == Some("PlayCard"));
    assert!(has_play, "PlayCard should be possible with active contract");

    let has_discard = actions
        .iter()
        .any(|a| a["action_type"].as_str() == Some("DiscardCard"));
    assert!(
        has_discard,
        "DiscardCard should be possible with active contract"
    );

    let has_accept = actions
        .iter()
        .any(|a| a["action_type"].as_str() == Some("AcceptContract"));
    assert!(
        !has_accept,
        "AcceptContract should not be possible with active contract"
    );
}

// -----------------------------------------------------------------------
// GET /docs/tutorial
// -----------------------------------------------------------------------

#[test]
fn docs_tutorial_returns_structured_response() {
    let client = create_client();
    let response = client.get("/docs/tutorial").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    assert!(body.get("title").is_some(), "should have title");
    assert!(
        body.get("introduction").is_some(),
        "should have introduction"
    );
    assert!(
        body.get("core_concepts").is_some(),
        "should have core_concepts"
    );
    assert!(body.get("steps").is_some(), "should have steps");
    assert!(body.get("next_steps").is_some(), "should have next_steps");

    let steps = body["steps"].as_array().expect("steps array");
    assert!(steps.len() >= 5, "tutorial should have at least 5 steps");
}

// -----------------------------------------------------------------------
// GET /docs/hints
// -----------------------------------------------------------------------

#[test]
fn docs_hints_returns_structured_response() {
    let client = create_client();
    let response = client.get("/docs/hints").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    assert!(body.get("title").is_some(), "should have title");
    assert!(
        body.get("general_tips").is_some(),
        "should have general_tips"
    );
    assert!(body.get("tiers").is_some(), "should have tiers");

    let tiers = body["tiers"].as_array().expect("tiers array");
    assert!(!tiers.is_empty(), "should have at least 1 tier hints");
}

// -----------------------------------------------------------------------
// GET /docs/designer
// -----------------------------------------------------------------------

#[test]
fn docs_designer_returns_structured_response() {
    let client = create_client();
    let response = client.get("/docs/designer").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    assert!(body.get("title").is_some(), "should have title");
    assert!(
        body.get("introduction").is_some(),
        "should have introduction"
    );
    assert!(body.get("sections").is_some(), "should have sections");

    let sections = body["sections"].as_array().expect("sections array");
    assert!(
        sections.len() >= 5,
        "designer guide should have at least 5 sections"
    );
}

// -----------------------------------------------------------------------
// OpenAPI includes new endpoints
// -----------------------------------------------------------------------

#[test]
fn openapi_includes_new_endpoints() {
    let client = create_client();
    let response = client.get("/openapi.json").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body: serde_json::Value =
        serde_json::from_str(&response.into_string().expect("body")).expect("valid json");
    let paths = body["paths"].as_object().expect("paths object");

    let expected_paths = [
        "/library/cards",
        "/player/tokens",
        "/contracts/active",
        "/actions/possible",
        "/docs/tutorial",
        "/docs/hints",
        "/docs/designer",
    ];

    for path in &expected_paths {
        assert!(paths.contains_key(*path), "OpenAPI should include {path}");
    }
}
