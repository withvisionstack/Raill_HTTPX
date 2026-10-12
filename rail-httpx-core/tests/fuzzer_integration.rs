mod common;

use common::*;
use httpmock::prelude::*;
use rail_httpx_core::http::HttpRequest;
use rail_httpx_core::security::Fuzzer;

#[tokio::test]
async fn fuzz_replaces_marker_in_url_and_collects_all_results() {
    let server = MockServer::start();

    // Um mock por caminho — sem regex, sem pegadinha de trait bound.
    for path in ["/admin", "/user", "/test"] {
        server.mock(|when, then| {
            when.method(GET).path(path);
            then.status(200).body("ok");
        });
    }

    let client = test_client();
    let env = test_env();

    let template = HttpRequest::get(format!("{}/FUZZ", server.base_url()));
    let payloads = vec!["admin".to_string(), "user".to_string(), "test".to_string()];

    let fuzzer = Fuzzer::new(client, 8);
    let results = fuzzer.fuzz(&template, &env, "FUZZ", &payloads).await.unwrap();

    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r.status == 200));
    let mut got: Vec<_> = results.into_iter().map(|r| r.payload).collect();
    got.sort();
    assert_eq!(got, vec!["admin", "test", "user"]);
}

#[tokio::test]
async fn fuzz_marks_network_errors_as_status_zero() {
    let client = test_client();
    let env = test_env();

    let template = HttpRequest::get("http://127.0.0.1:1/FUZZ");
    let payloads = vec!["a".to_string()];

    let fuzzer = Fuzzer::new(client, 2);
    let results = fuzzer.fuzz(&template, &env, "FUZZ", &payloads).await.unwrap();
    assert_eq!(results[0].status, 0);
    assert!(results[0].response.is_none());
}