//! HTTP endpoints for the game loop.
//!
//! All gameplay is driven through `POST /action`. State inspection
//! via `GET /state`, `GET /actions/history`, and additional query endpoints.

use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;
use std::sync::Mutex;

use crate::action_log::{ActionEntry, PlayerAction};
use crate::game_state::{ActionResult, GameState, GameStateView, PlayerTokensView, PossibleAction};
use crate::metrics::SessionMetrics;
use crate::types::{CardEntry, CardTag, Contract, TierContracts};

/// Dispatch a player action.
///
/// Accepts a JSON payload describing the action (NewGame, AcceptContract,
/// PlayCard, DiscardCard) and returns a typed result variant matching the
/// action on success, or a specific error variant on failure.
#[openapi]
#[post("/action", format = "json", data = "<action>")]
pub fn post_action(
    action: Json<PlayerAction>,
    game_state: &State<Mutex<GameState>>,
) -> Json<ActionResult> {
    let mut gs = game_state.lock().expect("game state lock poisoned");
    let result = gs.dispatch(action.into_inner());
    Json(result)
}

/// Current game state.
///
/// Returns a snapshot of the game including hand contents, token balances,
/// active/offered contracts, deck and discard pile sizes, and metadata.
#[openapi]
#[get("/state")]
pub fn get_state(game_state: &State<Mutex<GameState>>) -> Json<GameStateView> {
    let gs = game_state.lock().expect("game state lock poisoned");
    Json(gs.view())
}

/// Action history for replay and save/load.
///
/// Returns the ordered list of all player actions taken in this game.
/// Combined with the game seed (available in `/state`), replaying these
/// actions on a fresh game with the same seed reproduces the exact same
/// state — this serves as the save/load mechanism.
#[openapi]
#[get("/actions/history")]
pub fn get_actions_history(game_state: &State<Mutex<GameState>>) -> Json<Vec<ActionEntry>> {
    let gs = game_state.lock().expect("game state lock poisoned");
    Json(gs.action_log().entries().to_vec())
}

/// Available contracts in the market.
///
/// Returns the currently offered contracts grouped by tier, including
/// each contract's requirements and reward card preview. The player can
/// inspect these before accepting one via `POST /action`.
#[openapi]
#[get("/contracts/available")]
pub fn get_contracts_available(game_state: &State<Mutex<GameState>>) -> Json<Vec<TierContracts>> {
    let gs = game_state.lock().expect("game state lock poisoned");
    Json(gs.offered_contracts().to_vec())
}

/// Card catalogue with optional tag filter.
///
/// Returns all player action cards in the game with their per-location copy
/// counts (shelved, deck, hand, discard). Use the optional `?tag=` query
/// parameter to filter by card tag (e.g., `Production`, `Transformation`,
/// `QualityControl`, `SystemAdjustment`).
#[openapi]
#[get("/library/cards?<tag>")]
pub fn get_library_cards(
    tag: Option<String>,
    game_state: &State<Mutex<GameState>>,
) -> Json<Vec<CardEntry>> {
    let gs = game_state.lock().expect("game state lock poisoned");

    let tag_filter = match tag {
        Some(t) => match serde_json::from_value::<CardTag>(serde_json::Value::String(t)) {
            Ok(parsed) => Some(parsed),
            Err(_) => return Json(Vec::new()),
        },
        None => None,
    };

    let cards: Vec<CardEntry> = gs
        .cards()
        .iter()
        .filter(|entry| match &tag_filter {
            Some(filter_tag) => entry.card.tags.contains(filter_tag),
            None => true,
        })
        .cloned()
        .collect();
    Json(cards)
}

/// Player token balances grouped by category.
///
/// Returns all non-zero token balances organized into three categories:
/// **beneficial** (ProductionUnit, Energy, QualityPoint, Innovation), **harmful** (Heat,
/// Waste, Pollution), and **progression** (ContractsTierCompleted).
/// Use this to monitor resource levels and plan card plays.
#[openapi]
#[get("/player/tokens")]
pub fn get_player_tokens(game_state: &State<Mutex<GameState>>) -> Json<PlayerTokensView> {
    let gs = game_state.lock().expect("game state lock poisoned");
    Json(gs.tokens_view())
}

/// Currently active contract.
///
/// Returns the contract the player is currently working on, or `null` if
/// no contract is active. When a contract is active, the player can play
/// cards or discard to make progress toward its requirements.
#[openapi]
#[get("/contracts/active")]
pub fn get_contracts_active(game_state: &State<Mutex<GameState>>) -> Json<Option<Contract>> {
    let gs = game_state.lock().expect("game state lock poisoned");
    Json(gs.active_contract().cloned())
}

/// Actions available in the current game state.
///
/// Returns the list of player actions that are valid right now, along with
/// a human-readable description for each. This is the recommended way for
/// clients to discover what they can do:
/// - **No active contract**: `AcceptContract` actions for each offered contract
/// - **Active contract**: `PlayCard` and `DiscardCard` for each hand position
/// - `NewGame` is always available
#[openapi]
#[get("/actions/possible")]
pub fn get_actions_possible(game_state: &State<Mutex<GameState>>) -> Json<Vec<PossibleAction>> {
    let gs = game_state.lock().expect("game state lock poisoned");
    Json(gs.possible_actions())
}

/// Gameplay statistics and metrics.
///
/// Returns session-level statistics computed from live gameplay data:
/// contract completion counts per tier, card usage breakdown by tag,
/// token production/consumption flow, efficiency metrics (average cards
/// per contract), streaks, and strategy diversity analysis.
/// All metrics reset when a new game starts.
#[openapi]
#[get("/metrics")]
pub fn get_metrics(game_state: &State<Mutex<GameState>>) -> Json<SessionMetrics> {
    let gs = game_state.lock().expect("game state lock poisoned");
    Json(gs.session_metrics())
}
