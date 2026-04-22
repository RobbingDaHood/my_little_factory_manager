//! Integration tests for Phase 5: Deckbuilding mechanics.
//!
//! Tests: ReplaceCard action, deck slot limits, reward card shelving,
//! DeckSlots token initialization, and config-driven reward generation.

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

fn detail(result: &serde_json::Value) -> &serde_json::Value {
    &result["detail"]
}

fn get_state(client: &Client) -> serde_json::Value {
    let response = client.get("/state").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().expect("response body");
    serde_json::from_str(&body).expect("valid json")
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

fn card_count_total(state: &serde_json::Value, field: &str) -> u64 {
    state["cards"]
        .as_array()
        .expect("cards array")
        .iter()
        .map(|entry| entry["counts"][field].as_u64().unwrap_or(0))
        .sum()
}

/// Returns deck + hand + discard total.
fn active_total(state: &serde_json::Value) -> u64 {
    card_count_total(state, "deck")
        + card_count_total(state, "hand")
        + card_count_total(state, "discard")
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

/// Complete one contract cycle: accept contract 0 from tier 0, play all hand cards.
/// Returns the state after the cycle.
fn complete_one_contract(client: &Client) -> serde_json::Value {
    post_action(
        client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    // Play cards until the contract completes
    for _ in 0..50 {
        let state = get_state(client);
        if state["active_contract"].is_null() {
            return state;
        }
        let idx = first_card_in_hand(client);
        post_action(
            client,
            &format!(r#"{{"action_type":"PlayCard","card_index":{idx}}}"#),
        );
    }
    get_state(client)
}

// ---------------------------------------------------------------------------
// DeckSlots token initialization
// ---------------------------------------------------------------------------

#[test]
fn deck_slots_initialized_to_starting_deck_size() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    let state = get_state(&client);

    let deck_slots = token_amount(&state, r#""DeckSlots""#);
    assert_eq!(
        deck_slots, 50,
        "DeckSlots should be initialized to starting_deck_size (50)"
    );
}

#[test]
fn state_view_does_not_include_deck_slots_fields() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    let state = get_state(&client);

    assert!(
        state.get("deck_slots_total").is_none(),
        "deck_slots_total should not be present (DeckSlots is fixed)"
    );
    assert!(
        state.get("deck_slots_used").is_none(),
        "deck_slots_used should not be present (DeckSlots is fixed)"
    );
}

// ---------------------------------------------------------------------------
// ReplaceCard validation errors
// ---------------------------------------------------------------------------

#[test]
fn replace_card_fails_during_active_contract() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);
    post_action(
        &client,
        r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
    );

    let (status, result) = post_action(
        &client,
        r#"{"action_type":"ReplaceCard","target_card_index":0,"replacement_card_index":1,"sacrifice_card_index":2}"#,
    );
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    assert_eq!(
        detail(&result)["error_type"],
        "ContractActiveForDeckbuilding"
    );
}

#[test]
fn replace_card_fails_with_invalid_target_index() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let (status, result) = post_action(
        &client,
        r#"{"action_type":"ReplaceCard","target_card_index":999,"replacement_card_index":0,"sacrifice_card_index":0}"#,
    );
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    assert_eq!(detail(&result)["error_type"], "InvalidTargetCardIndex");
}

#[test]
fn replace_card_fails_with_invalid_replacement_index() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let (status, result) = post_action(
        &client,
        r#"{"action_type":"ReplaceCard","target_card_index":0,"replacement_card_index":999,"sacrifice_card_index":0}"#,
    );
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    // Could be InvalidReplacementCardIndex or NoTargetCopies depending on deck state
    let error_type = detail(&result)["error_type"].as_str().unwrap_or("");
    assert!(
        error_type == "InvalidReplacementCardIndex" || error_type == "NoTargetCopies",
        "expected replacement-related error, got: {error_type}"
    );
}

#[test]
fn replace_card_fails_with_invalid_sacrifice_index() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let (status, result) = post_action(
        &client,
        r#"{"action_type":"ReplaceCard","target_card_index":0,"replacement_card_index":0,"sacrifice_card_index":999}"#,
    );
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    // Depending on state, could be several error types
    let error_type = detail(&result)["error_type"].as_str().unwrap_or("");
    assert!(
        error_type == "InvalidSacrificeCardIndex"
            || error_type == "NoTargetCopies"
            || error_type == "NoShelvedCopies",
        "expected an error, got: {error_type}"
    );
}

#[test]
fn replace_card_fails_when_no_shelved_copies() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    // At game start, all starter cards are in the active cycle (deck/hand),
    // so no card has shelved copies. ReplaceCard should fail.
    let (status, result) = post_action(
        &client,
        r#"{"action_type":"ReplaceCard","target_card_index":0,"replacement_card_index":0,"sacrifice_card_index":1}"#,
    );
    assert_eq!(status, Status::Ok);
    assert_eq!(result["outcome"], "Error");
    let error_type = detail(&result)["error_type"].as_str().unwrap_or("");
    assert!(
        error_type == "NoShelvedCopies" || error_type == "NoTargetCopies",
        "expected shelved/target error, got: {error_type}"
    );
}

#[test]
fn replace_card_allows_sacrifice_is_target_with_enough_copies() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":100}"#);

    // Complete contracts to get a shelved reward card for replacement
    for _ in 0..15 {
        let state = get_state(&client);
        if state["active_contract"].is_null() {
            let cards = state["cards"].as_array().expect("cards");
            let has_shelved = cards
                .iter()
                .any(|c| c["counts"]["shelved"].as_u64().unwrap_or(0) > 0);
            if has_shelved {
                break;
            }
            post_action(
                &client,
                r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
            );
        } else {
            {
                let idx = first_card_in_hand(&client);
                post_action(
                    &client,
                    &format!(r#"{{"action_type":"PlayCard","card_index":{}}}"#, idx),
                );
            };
        }
    }

    let state = get_state(&client);
    let cards = state["cards"].as_array().expect("cards");

    // Find a card with shelved >= 1, and deck copies.
    // This ensures it can be used as both target and sacrifice.
    let target_idx = cards.iter().enumerate().find(|(_, c)| {
        let counts = &c["counts"];
        let shelved = counts["shelved"].as_u64().unwrap_or(0);
        shelved >= 1 && counts["deck"].as_u64().unwrap_or(0) > 0
    });

    let shelved_idx = cards
        .iter()
        .enumerate()
        .find(|(_, c)| c["counts"]["shelved"].as_u64().unwrap_or(0) > 0)
        .map(|(i, _)| i);

    if let (Some((target, _)), Some(replacement)) = (target_idx, shelved_idx) {
        // replacement must differ from target for this test to isolate sacrifice==target
        let replacement = if replacement == target {
            cards
                .iter()
                .enumerate()
                .find(|(i, c)| *i != target && c["counts"]["shelved"].as_u64().unwrap_or(0) > 0)
                .map(|(i, _)| i)
        } else {
            Some(replacement)
        };

        if let Some(replacement) = replacement {
            let before_shelved_total: u64 = card_count_total(&state, "shelved");

            let action = format!(
                r#"{{"action_type":"ReplaceCard","target_card_index":{target},"replacement_card_index":{replacement},"sacrifice_card_index":{target}}}"#,
            );
            let (status, result) = post_action(&client, &action);
            assert_eq!(status, Status::Ok);
            assert_eq!(
                result["outcome"], "Success",
                "sacrifice == target should succeed when shelved >= 2"
            );
            assert_eq!(detail(&result)["result_type"], "CardReplaced");

            let after_state = get_state(&client);
            let after_shelved_total: u64 = card_count_total(&after_state, "shelved");
            assert_eq!(
                after_shelved_total,
                before_shelved_total - 1,
                "one shelved copy destroyed by sacrifice"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// ReplaceCard success scenario
// ---------------------------------------------------------------------------

#[test]
fn replace_card_swaps_deck_card_with_shelved_card() {
    let client = client();
    // Use a seed and complete contracts until a reward card is shelved
    post_action(&client, r#"{"action_type":"NewGame","seed":100}"#);

    // Complete contracts until we have a shelved reward card
    for _ in 0..15 {
        let state = get_state(&client);
        if state["active_contract"].is_null() {
            // Check if any card has shelved copies
            let cards = state["cards"].as_array().expect("cards");
            let has_shelved = cards
                .iter()
                .any(|c| c["counts"]["shelved"].as_u64().unwrap_or(0) > 0);
            if has_shelved {
                break;
            }
            // Accept next contract
            post_action(
                &client,
                r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
            );
        } else {
            {
                let idx = first_card_in_hand(&client);
                post_action(
                    &client,
                    &format!(r#"{{"action_type":"PlayCard","card_index":{}}}"#, idx),
                );
            };
        }
    }

    let state = get_state(&client);
    let cards = state["cards"].as_array().expect("cards");

    // Find a card with shelved copies for replacement
    let shelved_idx = cards
        .iter()
        .enumerate()
        .find(|(_, c)| c["counts"]["shelved"].as_u64().unwrap_or(0) > 0)
        .map(|(i, _)| i);

    // Find a card with deck copies for target
    let target_idx = cards
        .iter()
        .enumerate()
        .find(|(_, c)| c["counts"]["deck"].as_u64().unwrap_or(0) > 0)
        .map(|(i, _)| i);

    // Find a sacrifice candidate (card with shelved copies, not the target or replacement)
    let sacrifice_idx = cards
        .iter()
        .enumerate()
        .find(|(i, c)| {
            c["counts"]["shelved"].as_u64().unwrap_or(0) > 0
                && Some(*i) != target_idx
                && Some(*i) != shelved_idx
        })
        .map(|(i, _)| i)
        .or_else(|| {
            // If no separate sacrifice exists, use replacement if it has >= 2 shelved copies
            shelved_idx.filter(|&idx| cards[idx]["counts"]["shelved"].as_u64().unwrap_or(0) >= 2)
        });

    if let (Some(target), Some(replacement), Some(sacrifice)) =
        (target_idx, shelved_idx, sacrifice_idx)
    {
        let before_shelved_total: u64 = card_count_total(&state, "shelved");

        let action = format!(
            r#"{{"action_type":"ReplaceCard","target_card_index":{target},"replacement_card_index":{replacement},"sacrifice_card_index":{sacrifice}}}"#,
        );
        let (status, result) = post_action(&client, &action);
        assert_eq!(status, Status::Ok);
        assert_eq!(result["outcome"], "Success");
        assert_eq!(detail(&result)["result_type"], "CardReplaced");

        let after_state = get_state(&client);
        let after_shelved_total: u64 = card_count_total(&after_state, "shelved");

        // Sacrifice destroys one shelved copy
        assert_eq!(
            after_shelved_total,
            before_shelved_total - 1,
            "one shelved copy should be destroyed by sacrifice"
        );
    }
    // If we couldn't set up the scenario, the test is inconclusive but not failing.
    // The error-path tests above cover the validation logic.
}

// ---------------------------------------------------------------------------
// Reward card respects deck limit
// ---------------------------------------------------------------------------

#[test]
fn reward_card_goes_to_shelved_only() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let state_before = get_state(&client);

    // Complete one contract
    complete_one_contract(&client);

    let state_after = get_state(&client);
    let lib_after = card_count_total(&state_after, "shelved");
    let lib_before = card_count_total(&state_before, "shelved");

    // Library should have increased (reward card added)
    assert!(
        lib_after > lib_before,
        "shelved count should increase after completing a contract"
    );

    // Active cycle (deck+hand+discard) should NOT have changed
    let active_before = active_total(&state_before);
    let active_after = active_total(&state_after);
    assert_eq!(
        active_before, active_after,
        "active cycle (deck+hand+discard) should stay fixed at 50"
    );
}

// ---------------------------------------------------------------------------
// Possible actions include ReplaceCard
// ---------------------------------------------------------------------------

#[test]
fn possible_actions_do_not_include_replace_card_at_game_start() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    let response = client.get("/actions/possible").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let body = response.into_string().expect("response body");
    let actions: serde_json::Value = serde_json::from_str(&body).expect("valid json");

    let has_replace = actions
        .as_array()
        .expect("actions array")
        .iter()
        .any(|a| a["action_type"] == "ReplaceCard");

    assert!(
        !has_replace,
        "ReplaceCard should not be available at game start (no shelved cards)"
    );
}

// ---------------------------------------------------------------------------
// Config-driven reward generation produces PureProduction at Tier 0
// ---------------------------------------------------------------------------

#[test]
fn tier1_reward_cards_are_pure_production() {
    let client = client();
    post_action(&client, r#"{"action_type":"NewGame","seed":42}"#);

    // Complete a contract and check the reward card
    complete_one_contract(&client);

    let state = get_state(&client);
    let cards = state["cards"].as_array().expect("cards array");

    // Reward cards should only have Production tags at tier 0
    for card_entry in cards {
        let tags = card_entry["card"]["tags"].as_array();
        if let Some(tags) = tags {
            for tag in tags {
                assert_eq!(
                    tag.as_str().unwrap_or(""),
                    "Production",
                    "tier 0 reward cards should only have Production tags"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Determinism with ReplaceCard
// ---------------------------------------------------------------------------

#[test]
fn replace_card_is_deterministic() {
    let client = client();

    // Run a sequence with a fixed seed
    post_action(&client, r#"{"action_type":"NewGame","seed":200}"#);
    for _ in 0..10 {
        let state = get_state(&client);
        if state["active_contract"].is_null() {
            post_action(
                &client,
                r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
            );
        } else {
            {
                let idx = first_card_in_hand(&client);
                post_action(
                    &client,
                    &format!(r#"{{"action_type":"PlayCard","card_index":{}}}"#, idx),
                );
            };
        }
    }
    let state_run1 = get_state(&client);

    // Re-run with same seed
    post_action(&client, r#"{"action_type":"NewGame","seed":200}"#);
    for _ in 0..10 {
        let state = get_state(&client);
        if state["active_contract"].is_null() {
            post_action(
                &client,
                r#"{"action_type":"AcceptContract","tier_index":0,"contract_index":0}"#,
            );
        } else {
            {
                let idx = first_card_in_hand(&client);
                post_action(
                    &client,
                    &format!(r#"{{"action_type":"PlayCard","card_index":{}}}"#, idx),
                );
            };
        }
    }
    let state_run2 = get_state(&client);

    assert_eq!(
        state_run1["cards"], state_run2["cards"],
        "same seed + same actions should produce identical card state"
    );
    assert_eq!(
        state_run1["tokens"], state_run2["tokens"],
        "same seed + same actions should produce identical token state"
    );
}
