use crate::config_loader::config_hash;
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
    /// Deterministic fingerprint of embedded config files (FNV-1a hex).
    /// Changes whenever game_rules.json or token_definitions.json change.
    pub config_hash: String,
}

/// Current game version.
///
/// Returns the game version (semver) and a deterministic config fingerprint.
/// Use this endpoint to verify the server is running and to detect config
/// changes between deployments.
#[openapi]
#[get("/version")]
pub fn get_version() -> Json<VersionInfo> {
    Json(VersionInfo {
        version: GAME_VERSION.to_string(),
        config_hash: config_hash(),
    })
}
