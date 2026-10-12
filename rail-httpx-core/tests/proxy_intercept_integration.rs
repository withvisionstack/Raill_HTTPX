mod common;

use httpmock::prelude::*;
use rail_httpx_core::security::{
    CertificateAuthority, InterceptOutcome, Interceptor, ProxyServer,
};
use std::sync::Arc;
use tempfile::tempdir;

/// Interceptor que acrescenta um header fixo em toda request.
async fn run_append_header_interceptor(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<rail_httpx_core::security::InterceptJob>,
    header: &'static [u8],
) {
    while let Some(job) = rx.recv().await {
        // Encontra o fim dos headers (\r\n\r\n) e injeta antes.
        let mut modified = job.request.clone();
        if let Some(pos) = modified.windows(4).position(|w| w == b"\r\n\r\n") {
            let mut out = Vec::with_capacity(modified.len() + header.len());
            out.extend_from_slice(&modified[..pos]);
            out.extend_from_slice(b"\r\n");
            out.extend_from_slice(header);
            out.extend_from_slice(&modified[pos..]);
            modified = out;
        }
        // ✅ Agora o Forward leva `Some(bytes)` para substituir.
        let _ = job.respond.send(InterceptOutcome::Forward(Some(modified)));
    }
}

#[tokio::test]
async fn interceptor_can_modify_request_in_transit() {
    let backend = MockServer::start();
    let m = backend.mock(|when, then| {
        when.method(GET)
            .path("/api/data")
            .header("X-Rail-Injected", "yes");
        then.status(200).body("modified-ok");
    });
    let backend_port = url::Url::parse(&backend.base_url()).unwrap().port().unwrap();

    let dir = tempdir().unwrap();
    let ca = Arc::new(CertificateAuthority::new_or_load(dir.path()).unwrap());
    let ca_pem = ca.ca_cert_pem().to_string();

    let (ic, rx) = Interceptor::new(None);
    tokio::spawn(run_append_header_interceptor(rx, b"X-Rail-Injected: yes\r\n"));

    let server = ProxyServer::bind_with_interceptor("127.0.0.1:0", ca, ic).await.unwrap();
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
        .get(format!("https://127.0.0.1:{}/api/data", backend_port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "modified-ok");
    m.assert();
}

#[tokio::test]
async fn interceptor_can_drop_request() {
    let backend = MockServer::start();
    let m = backend.mock(|when, then| {
        when.any_request();
        then.status(200).body("should-never-arrive");
    });
    let backend_port = url::Url::parse(&backend.base_url()).unwrap().port().unwrap();

    let dir = tempdir().unwrap();
    let ca = Arc::new(CertificateAuthority::new_or_load(dir.path()).unwrap());
    let ca_pem = ca.ca_cert_pem().to_string();

    let (ic, mut rx) = Interceptor::new(None);
    tokio::spawn(async move {
        while let Some(job) = rx.recv().await {
            let _ = job.respond.send(InterceptOutcome::Drop);
        }
    });

    let server = ProxyServer::bind_with_interceptor("127.0.0.1:0", ca, ic).await.unwrap();
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

    let _ = client
        .get(format!("https://127.0.0.1:{}/never", backend_port))
        .send()
        .await;

    // Espera um tiquinho para o mock registrar (ou não) a tentativa.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    m.assert_hits(0);
}