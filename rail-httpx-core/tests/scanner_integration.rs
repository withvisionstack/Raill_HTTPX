mod common;

use common::*;
use httpmock::prelude::*;
use rail_httpx_core::http::HttpRequest;
use rail_httpx_core::security::{Scanner, VulnCategory};

#[tokio::test]
async fn finds_sql_error_on_parameter() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/item");
        then.status(500).body("You have an error in your SQL syntax");
    });

    let client = test_client();
    let env = test_env();
    let req = HttpRequest::get(format!("{}/item", server.base_url())).query("id", "1");

    let scanner = Scanner::with_cap(client, 1); // ⬅️ era for_tests
    let findings = scanner.scan(&req, &env).await.unwrap();

    assert!(
        findings.iter().any(|f| f.category == VulnCategory::SqlInjection),
        "esperava finding de SQLi, obtido: {:?}",
        findings
    );
}

#[tokio::test]
async fn finds_reflected_xss() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/search");
        then.status(200)
            .body("<html><body><script>alert(1)</script></body></html>");
    });

    let client = test_client();
    let env = test_env();
    let req = HttpRequest::get(format!("{}/search", server.base_url())).query("q", "x");

    let scanner = Scanner::with_cap(client, 1);
    let findings = scanner.scan(&req, &env).await.unwrap();

    assert!(findings.iter().any(|f| f.category == VulnCategory::Xss));
}

#[tokio::test]
async fn no_findings_on_clean_endpoint() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/ok");
        then.status(200).body("tudo certo");
    });

    let client = test_client();
    let env = test_env();
    let req = HttpRequest::get(format!("{}/ok", server.base_url())).query("id", "1");

    let scanner = Scanner::with_cap(client, 1);
    let findings = scanner.scan(&req, &env).await.unwrap();
    assert!(findings.is_empty(), "falso positivo: {:?}", findings);
}