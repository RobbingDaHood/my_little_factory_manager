use my_little_factory_manager::rocket_initialize;
use rocket::http::Status;
use rocket::local::blocking::Client;

#[test]
fn version_endpoint_returns_ok() {
    let client = Client::tracked(rocket_initialize()).expect("valid rocket instance");
    let response = client.get("/version").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().expect("response body");
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    assert!(
        json.get("version").is_some(),
        "response should contain version field"
    );

    let version = json["version"]
        .as_str()
        .expect("version should be a string");
    assert!(!version.is_empty(), "version should not be empty");
    assert!(
        json.get("config_hash").is_some(),
        "response should contain config_hash field"
    );
    let hash = json["config_hash"]
        .as_str()
        .expect("config_hash should be a string");
    assert_eq!(hash.len(), 16, "config_hash should be 16 hex chars");
}

#[test]
fn openapi_json_returns_ok() {
    let client = Client::tracked(rocket_initialize()).expect("valid rocket instance");
    let response = client.get("/openapi.json").dispatch();
    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().expect("response body");
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid openapi json");
    assert!(
        json.get("openapi").is_some(),
        "should contain openapi field"
    );
    assert!(json.get("paths").is_some(), "should contain paths field");
}

#[test]
fn swagger_ui_is_mounted() {
    let client = Client::tracked(rocket_initialize()).expect("valid rocket instance");
    let response = client.get("/swagger/").dispatch();
    assert!(
        response.status() == Status::Ok || response.status() == Status::SeeOther,
        "expected OK or redirect, got {:?}",
        response.status()
    );
}
