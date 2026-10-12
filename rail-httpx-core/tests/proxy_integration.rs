mod common;

use httpmock::prelude::*;
use rail_httpx_core::security::{CertificateAuthority, ProxyServer};
use std::sync::Arc;
use tempfile::tempdir;

/// Prova que o proxy MITM encaminha de verdade:
///   cliente → proxy (MITM TLS) → backend HTTP real → volta
///
/// O backend é um `httpmock` normal (HTTP puro em 127.0.0.1:PORT).
/// O cliente acha que está falando HTTPS com `127.0.0.1:PORT`,
/// porque o proxy gera um leaf pra `127.0.0.1` assinado pelo CA local.
#[tokio::test]
async fn proxy_forwards_to_real_backend() {
    // 1. Backend "de verdade" — HTTP puro
    let backend = MockServer::start();
    let m = backend.mock(|when, then| {
        when.method(GET).path("/api/users");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"users":["ana","bia"]}"#);
    });
    let backend_url = url::Url::parse(&backend.base_url()).unwrap();
    let backend_port = backend_url.port().unwrap();

    // 2. Proxy MITM com CA próprio
    let dir = tempdir().unwrap();
    let ca = Arc::new(CertificateAuthority::new_or_load(dir.path()).unwrap());
    let ca_pem = ca.ca_cert_pem().to_string();

    let server = ProxyServer::bind("127.0.0.1:0", ca).await.unwrap();
    let proxy_addr = server.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // 3. Cliente HTTPS confiando no CA local, falando através do proxy
    let ca_cert = reqwest::Certificate::from_pem(ca_pem.as_bytes()).unwrap();
    let client = reqwest::Client::builder()
        .add_root_certificate(ca_cert)
        .proxy(reqwest::Proxy::all(format!("http://{}", proxy_addr)).unwrap())
        .build()
        .unwrap();

    // 4. URL "HTTPS" apontando pro mesmo 127.0.0.1:PORT do backend
    let url = format!("https://127.0.0.1:{}/api/users", backend_port);
    let resp = client.get(&url).send().await.expect("request via proxy falhou");

    // 5. A resposta do backend real chegou — encaminhamento funcionou.
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), r#"{"users":["ana","bia"]}"#);
    m.assert();
}

/// Prova que uma segunda request no mesmo cliente também funciona
/// (o proxy aceita nova conexão CONNECT a cada vez).
#[tokio::test]
async fn proxy_handles_multiple_requests() {
    let backend = MockServer::start();
    let m = backend.mock(|when, then| {
        when.method(GET).path("/ping");
        then.status(200).body("pong");
    });
    let backend_url = url::Url::parse(&backend.base_url()).unwrap();
    let backend_port = backend_url.port().unwrap();

    let dir = tempdir().unwrap();
    let ca = Arc::new(CertificateAuthority::new_or_load(dir.path()).unwrap());
    let ca_pem = ca.ca_cert_pem().to_string();

    let server = ProxyServer::bind("127.0.0.1:0", ca).await.unwrap();
    let proxy_addr = server.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = server.run().await;
    });

    let ca_cert = reqwest::Certificate::from_pem(ca_pem.as_bytes()).unwrap();
    let client = reqwest::Client::builder()
        .add_root_certificate(ca_cert)
        .proxy(reqwest::Proxy::all(format!("http://{}", proxy_addr)).unwrap())
        .build()
        .unwrap();

    let url = format!("https://127.0.0.1:{}/ping", backend_port);

    for _ in 0..3 {
        let resp = client.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), "pong");
    }
    m.assert_hits(3);
}

/// Teste manual contra a internet real. Ignorado por padrão porque
/// depende de rede externa. Rode com:
///
///   cargo test -p rail-httpx-core --test proxy_integration \
///       -- --ignored proxy_forwards_to_real_https_backend --nocapture
///
/// Ele prova que o proxy consegue:
///   - fazer TLS com o cliente (usando CA local)
///   - fazer TLS com api.github.com (usando CAs públicas)
///   - encaminhar request/response entre os dois
#[tokio::test]
#[ignore]
async fn proxy_forwards_to_real_https_backend() {
    let dir = tempdir().unwrap();
    let ca = Arc::new(CertificateAuthority::new_or_load(dir.path()).unwrap());
    let ca_pem = ca.ca_cert_pem().to_string();

    let server = ProxyServer::bind("127.0.0.1:0", ca).await.unwrap();
    let proxy_addr = server.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = server.run().await;
    });

    let ca_cert = reqwest::Certificate::from_pem(ca_pem.as_bytes()).unwrap();
    let client = reqwest::Client::builder()
        .add_root_certificate(ca_cert)
        .proxy(reqwest::Proxy::all(format!("http://{}", proxy_addr)).unwrap())
        .build()
        .unwrap();

    let resp = client
        .get("https://api.github.com/zen")
        .header("User-Agent", "rail-httpx-test")
        .send()
        .await
        .expect("request via proxy falhou");

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(!body.is_empty(), "corpo vazio — proxy não encaminhou?");
    println!("resposta do GitHub via MITM: {body}");
}