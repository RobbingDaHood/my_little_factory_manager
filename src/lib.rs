//! # My Little Factory Manager
//!
//! A deterministic, turn-based deckbuilding game where the player acts as a
//! factory manager fulfilling contracts from an open market. Built as a
//! headless REST API with OpenAPI documentation.

#![allow(clippy::module_name_repetitions)]
#[macro_use]
extern crate rocket;

use rocket_okapi::openapi_get_routes;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};

pub mod action_log;
pub mod config;
pub mod config_loader;
pub mod endpoints;
pub mod game_state;
pub mod starter_cards;
pub mod types;
pub mod version;

use crate::endpoints::{get_actions_history, get_state, post_action};
use crate::endpoints::{
    okapi_add_operation_for_get_actions_history_, okapi_add_operation_for_get_state_,
    okapi_add_operation_for_post_action_,
};
use crate::game_state::GameState;
use crate::version::get_version;
use crate::version::okapi_add_operation_for_get_version_;

/// Initializes and configures the Rocket web server with all routes and
/// OpenAPI documentation.
pub fn rocket_initialize() -> rocket::Rocket<rocket::Build> {
    #[allow(clippy::no_effect_underscore_binding)]
    let _ = env_logger::try_init();

    let game_state = std::sync::Mutex::new(GameState::new(None));

    rocket::build()
        .manage(game_state)
        .mount(
            "/",
            openapi_get_routes![get_version, post_action, get_state, get_actions_history,],
        )
        .mount("/swagger", make_swagger_ui(&swagger_config()))
}

fn swagger_config() -> SwaggerUIConfig {
    SwaggerUIConfig {
        url: "/openapi.json".to_string(),
        ..Default::default()
    }
}
