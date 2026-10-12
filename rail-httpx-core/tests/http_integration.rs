mod common;

use common::*;
use httpmock::prelude::*;
use rail_httpx_core::http::{Body, HttpRequest};

#[tokio::test]
async fn end_to_end_post_json_roundtrip() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/users")
            .header("Authorization", "Bearer tok-test")
            .json_body(serde_json::json!({"name":"ana"}));
        then.status(201)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({"id": 7, "name":"ana"}));
    });

    let env = test_env();
    let client = test_client();
    let req = HttpRequest::post(
        format!("{}/users", server.base_url()),
        Body::Json(serde_json::json!({"name":"ana"})),
    )
        .header("Authorization", "Bearer {{token}}");

    let resp = client.send(&req, &env).await.unwrap();
    assert_eq!(resp.status, 201);
    assert_eq!(resp.body_json().unwrap()["id"], 7);
    m.assert();
}

#[tokio::test]
async fn does_not_follow_redirect_by_default() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/start");
        then.status(302).header("location", "/dest");
    });
    server.mock(|when, then| {
        when.method(GET).path("/dest");
        then.status(200).body("chegou");
    });

    let env = test_env();
    let client = test_client();
    let req = HttpRequest::get(format!("{}/start", server.base_url()));

    let resp = client.send(&req, &env).await.unwrap();
    assert_eq!(resp.status, 302);
    assert_eq!(resp.headers.get("location").map(String::as_str), Some("/dest"));
}

#[tokio::test]
async fn timeout_is_respected() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/slow");
        then.status(200).delay(std::time::Duration::from_millis(800)).body("tarde");
    });

    let env = test_env();
    let client = test_client();
    let mut req = HttpRequest::get(format!("{}/slow", server.base_url()));
    req.timeout_ms = Some(100);

    let err = client.send(&req, &env).await.unwrap_err();
    // reqwest embrulha timeout como Error::Http
    assert!(matches!(err, rail_httpx_core::Error::Http(_)));
}