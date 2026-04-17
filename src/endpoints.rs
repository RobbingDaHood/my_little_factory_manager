//! HTTP endpoints for the game loop.
//!
//! All gameplay is driven through `POST /action`. State inspection
//! via `GET /state` and `GET /actions/history`.

use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;
use std::sync::Mutex;

use crate::action_log::{ActionEntry, PlayerAction};
use crate::game_state::{ActionResult, GameState, GameStateView};
use crate::types::TierContracts;

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
