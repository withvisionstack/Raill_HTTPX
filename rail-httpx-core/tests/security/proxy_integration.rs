mod common;

use rail_httpx_core::security::{CertificateAuthority, ProxyServer};
use std::sync::Arc;
use tempfile::tempdir;

/// Prova que o MITM funciona de ponta a ponta:
/// cliente confia no nosso CA → proxy intercepta → TLS handshake com leaf
/// gerado em runtime → resposta chega.
#[tokio::test]
async fn proxy_mitm_works_end_to_end() {
    let dir = tempdir().unwrap();
    let ca = Arc::new(CertificateAuthority::new_or_load(dir.path()).unwrap());
    let ca_pem = ca.ca_cert_pem().to_string();

    let server = ProxyServer::bind("127.0.0.1:0", ca).await.unwrap();
    let proxy_addr = server.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Cliente confia no CA gerado em runtime — é o que o Firefox faria
    // depois de instalar o `ca.pem`.
    let ca_cert = reqwest::Certificate::from_pem(ca_pem.as_bytes()).unwrap();

    // `resolve` mapeia o nome fictício para um IP qualquer — o proxy
    // responde antes de qualquer tentativa de conexão real.
    let client = reqwest::Client::builder()
        .add_root_certificate(ca_cert)
        .proxy(reqwest::Proxy::all(format!("http://{}", proxy_addr)).unwrap())
        .resolve("example.test", "127.0.0.1:1".parse().unwrap())
        .build()
        .unwrap();

    let resp = client
        .get("https://example.test/hello")
        .send()
        .await
        .expect("MITM falhou no handshake TLS");

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("rail_httpx MITM working for example.test"),
        "corpo inesperado: {body}"
    );
}