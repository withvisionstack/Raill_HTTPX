use super::{Body, HttpRequest, HttpResponse};
use crate::environment::Environment;
use crate::{Error, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Constrói o builder base — chamado 2× para gerar clientes com
/// políticas de redirect diferentes (reqwest não expõe redirect
/// por-request; é decisão de `ClientBuilder`).
fn base_builder(insecure: bool) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(insecure)
        .danger_accept_invalid_hostnames(insecure)
        .user_agent("RAIL_HTTPX/0.1")
        .pool_max_idle_per_host(16)
}

#[derive(Clone)]
pub struct HttpClient {
    /// Não segue redirects — comportamento padrão tipo Burp/Postman.
    no_redirect: reqwest::Client,
    /// Segue até 10 redirects (opt-in via `HttpRequest::follow_redirects`).
    with_redirect: reqwest::Client,
    /// Reflete o modo TLS usado na construção. Útil para testes/telemetria.
    pub insecure: bool,
    /// Teto de bytes lidos do corpo. `None` = sem teto (compatibilidade).
    pub max_response_bytes: Option<usize>,
}

impl HttpClient {
    pub fn new() -> Result<Self> { Self::build(false, None) }

    /// Cliente "estilo Burp": ignora validação de TLS.
    /// Só use em ambientes de pentest **autorizados**.
    pub fn insecure() -> Result<Self> { Self::build(true, None) }

    /// Cliente com teto de corpo — recomendado em produção (ver `Limits`).
    pub fn with_max_response_bytes(max: usize) -> Result<Self> {
        Self::build(false, Some(max))
    }

    /// Combinação pentest + teto de corpo. Usado pelo CLI quando
    /// `--insecure` é passado junto com `--max-response-bytes`.
    pub fn insecure_with_max_response_bytes(max: usize) -> Result<Self> {
        Self::build(true, Some(max))
    }

    fn build(insecure: bool, max_response_bytes: Option<usize>) -> Result<Self> {
        let no_redirect = base_builder(insecure)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        let with_redirect = base_builder(insecure)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;

        Ok(Self { no_redirect, with_redirect, insecure, max_response_bytes })
    }

    pub async fn send(&self, req: &HttpRequest, env: &Environment) -> Result<HttpResponse> {
        let url = env.render(&req.url);
        let parsed = url::Url::parse(&url).map_err(|e| Error::Url(e.to_string()))?;

        let method = reqwest::Method::from_bytes(req.method.as_str().as_bytes())
            .map_err(|e| Error::Other(e.to_string()))?;

        // Escolhe o cliente conforme a preferência do request.
        let client = if req.follow_redirects { &self.with_redirect } else { &self.no_redirect };

        let mut builder = client.request(method, parsed);

        // query params
        let rendered_query: Vec<(String, String)> = req
            .query
            .iter()
            .map(|(k, v)| (env.render(k), env.render(v)))
            .collect();
        if !rendered_query.is_empty() {
            builder = builder.query(&rendered_query);
        }

        // headers
        for (k, v) in &req.headers {
            builder = builder.header(env.render(k), env.render(v));
        }

        // body
        if let Some(body) = &req.body {
            builder = match body {
                Body::Json(v) => builder.json(v),
                Body::Text(t) => builder.body(env.render(t)),
                Body::Form(f) => {
                    let r: HashMap<String, String> = f
                        .iter()
                        .map(|(k, v)| (env.render(k), env.render(v)))
                        .collect();
                    builder.form(&r)
                }
                Body::Raw(b) => builder.body(b.clone()),
            };
        }

        // timeout por-request (isso SIM é suportado pelo RequestBuilder)
        if let Some(ms) = req.timeout_ms {
            builder = builder.timeout(Duration::from_millis(ms));
        }

        let start = Instant::now();
        let resp = builder.send().await?;
        let duration_ms = start.elapsed().as_millis();

        let status = resp.status().as_u16();
        let final_url = resp.url().to_string();
        let headers: HashMap<String, String> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // Leitura do corpo: sem teto = leitura direta; com teto = stream + corte.
        let body = match self.max_response_bytes {
            None => resp.bytes().await?.to_vec(),
            Some(max) => {
                use futures::StreamExt;
                let mut stream = resp.bytes_stream();
                let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk?;
                    let remaining = max.saturating_sub(buf.len());
                    if remaining == 0 { break; }
                    let take = remaining.min(chunk.len());
                    buf.extend_from_slice(&chunk[..take]);
                    if buf.len() >= max { break; }
                }
                buf
            }
        };

        Ok(HttpResponse { status, headers, body, duration_ms, url: final_url })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn insecure_flag_is_reflected() {
        assert!(HttpClient::insecure().unwrap().insecure);
        assert!(!HttpClient::new().unwrap().insecure);
    }

    #[test]
    fn insecure_with_max_response_bytes_sets_both() {
        let c = HttpClient::insecure_with_max_response_bytes(1234).unwrap();
        assert!(c.insecure);
        assert_eq!(c.max_response_bytes, Some(1234));
    }

    #[tokio::test]
    async fn sends_get_and_gets_200() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(GET).path("/ping");
            then.status(200).body("pong");
        });

        let client = HttpClient::new().unwrap();
        let env = Environment::new("test");
        let req = HttpRequest::get(format!("{}/ping", server.base_url()));

        let resp = client.send(&req, &env).await.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body_text(), "pong");
        m.assert();
    }

    #[tokio::test]
    async fn interpolates_env_vars_in_url_and_headers() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/users")
                .header("Authorization", "Bearer tok-123");
            then.status(200).body("ok");
        });

        let mut env = Environment::new("test");
        env.set("base", server.base_url());
        env.set("token", "tok-123");

        let req = HttpRequest::get("{{base}}/v1/users")
            .header("Authorization", "Bearer {{token}}");

        let client = HttpClient::new().unwrap();
        let resp = client.send(&req, &env).await.unwrap();
        assert_eq!(resp.status, 200);
        m.assert();
    }

    #[tokio::test]
    async fn sends_json_body() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST)
                .path("/echo")
                .header("content-type", "application/json")
                .json_body(serde_json::json!({"a":1}));
            then.status(201).body("created");
        });

        let client = HttpClient::new().unwrap();
        let env = Environment::new("test");
        let req = HttpRequest::post(
            format!("{}/echo", server.base_url()),
            Body::Json(serde_json::json!({"a":1})),
        );

        let resp = client.send(&req, &env).await.unwrap();
        assert_eq!(resp.status, 201);
        m.assert();
    }

    #[tokio::test]
    async fn invalid_url_returns_error() {
        let client = HttpClient::new().unwrap();
        let env = Environment::new("test");
        let req = HttpRequest::get("não-é-url");
        let err = client.send(&req, &env).await.unwrap_err();
        assert!(matches!(err, Error::Url(_)));
    }

    #[tokio::test]
    async fn follows_redirect_when_requested() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/start");
            then.status(302).header("location", "/dest");
        });
        let dest = server.mock(|when, then| {
            when.method(GET).path("/dest");
            then.status(200).body("chegou");
        });

        let client = HttpClient::new().unwrap();
        let env = Environment::new("test");

        let req = HttpRequest::get(format!("{}/start", server.base_url()));
        let resp = client.send(&req, &env).await.unwrap();
        assert_eq!(resp.status, 302);
        dest.assert_hits(0);

        let mut req = HttpRequest::get(format!("{}/start", server.base_url()));
        req.follow_redirects = true;
        let resp = client.send(&req, &env).await.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body_text(), "chegou");
        dest.assert_hits(1);
    }

    #[tokio::test]
    async fn truncates_response_body_at_max() {
        let server = MockServer::start();
        let big = "A".repeat(10_000);
        server.mock(|when, then| {
            when.method(GET).path("/big");
            then.status(200).body(&big);
        });

        let client = HttpClient::with_max_response_bytes(100).unwrap();
        let env = Environment::new("test");
        let req = HttpRequest::get(format!("{}/big", server.base_url()));

        let resp = client.send(&req, &env).await.unwrap();
        assert_eq!(resp.body.len(), 100);
    }
}