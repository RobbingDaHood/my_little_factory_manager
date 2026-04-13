use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::openapi;
use schemars::JsonSchema;

const GAME_VERSION: &str = "0.0.1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(crate = "rocket::serde")]
pub struct VersionInfo {
    /// Full version string (e.g. "0.0.1").
    pub version: String,
}

/// Current game version.
///
/// Returns the game version (semver). Use this endpoint to verify the server
/// is running and to check which version of the game is deployed.
#[openapi]
#[get("/version")]
pub fn get_version() -> Json<VersionInfo> {
    Json(VersionInfo {
        version: GAME_VERSION.to_string(),
    })
}
