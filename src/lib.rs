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

pub mod library;
pub mod version;

use crate::version::get_version;
use crate::version::okapi_add_operation_for_get_version_;

/// Initializes and configures the Rocket web server with all routes and
/// OpenAPI documentation.
pub fn rocket_initialize() -> rocket::Rocket<rocket::Build> {
    #[allow(clippy::no_effect_underscore_binding)]
    let _ = env_logger::try_init();

    rocket::build()
        .mount("/", openapi_get_routes![get_version,])
        .mount("/swagger", make_swagger_ui(&swagger_config()))
}

fn swagger_config() -> SwaggerUIConfig {
    SwaggerUIConfig {
        url: "/openapi.json".to_string(),
        ..Default::default()
    }
}
