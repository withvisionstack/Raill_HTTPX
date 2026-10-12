use crate::environment::Environment;
use crate::http::{Body, HttpClient, HttpRequest, HttpResponse};
use crate::limits::Limits;
use crate::{Error, Result};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzResult {
    pub payload: String,
    pub status: u16,
    pub length: usize,
    pub duration_ms: u128,
    #[serde(skip)]
    pub response: Option<HttpResponse>,
}

pub struct Fuzzer {
    client: HttpClient,
    concurrency: usize,
    limits: Limits,
}

impl Fuzzer {
    pub fn new(client: HttpClient, concurrency: usize) -> Self {
        Self::with_limits(client, concurrency, Limits::default())
    }

    pub fn with_limits(client: HttpClient, concurrency: usize, limits: Limits) -> Self {
        let concurrency = concurrency.clamp(1, limits.max_concurrency);
        Self { client, concurrency, limits }
    }

    pub fn concurrency(&self) -> usize { self.concurrency }
    pub fn limits(&self) -> &Limits { &self.limits }

    pub async fn fuzz(
        &self,
        template: &HttpRequest,
        env: &Environment,
        marker: &str,
        payloads: &[String],
    ) -> Result<Vec<FuzzResult>> {
        // Guarda-corpos ANTES de qualquer I/O.
        self.limits.check_payload_count(payloads.len())?;
        for p in payloads {
            self.limits.check_payload(p)?;
        }
        self.limits.check_concurrency(self.concurrency)?;
        self.limits.check_duration_secs(self.limits.max_duration_secs)?;

        let jobs = payloads.iter().map(|p| {
            let client = self.client.clone();
            let env = env.clone();
            let mut req = template.clone();
            inject(&mut req, marker, p);
            let p = p.clone();
            async move {
                match client.send(&req, &env).await {
                    Ok(resp) => FuzzResult {
                        payload: p,
                        status: resp.status,
                        length: resp.body.len(),
                        duration_ms: resp.duration_ms,
                        response: Some(resp),
                    },
                    Err(_) => FuzzResult {
                        payload: p,
                        status: 0,
                        length: 0,
                        duration_ms: 0,
                        response: None,
                    },
                }
            }
        });

        let fut = stream::iter(jobs)
            .buffer_unordered(self.concurrency)
            .collect::<Vec<_>>();

        tokio::time::timeout(Duration::from_secs(self.limits.max_duration_secs), fut)
            .await
            .map_err(|_| Error::Limit(format!(
                "fuzz excedeu orçamento de {}s",
                self.limits.max_duration_secs
            )))
    }
}

/// Função pura — substituição do marker em todos os campos do request.
pub fn inject(req: &mut HttpRequest, marker: &str, payload: &str) {
    req.url = req.url.replace(marker, payload);
    for v in req.headers.values_mut() {
        *v = v.replace(marker, payload);
    }
    for v in req.query.values_mut() {
        *v = v.replace(marker, payload);
    }
    if let Some(body) = &mut req.body {
        match body {
            Body::Text(t) => *t = t.replace(marker, payload),
            Body::Json(v) => {
                if let Some(s) = v.as_str() {
                    *v = serde_json::Value::String(s.replace(marker, payload));
                }
            }
            Body::Form(f) => {
                for v in f.values_mut() {
                    *v = v.replace(marker, payload);
                }
            }
            Body::Raw(_) => {}
        }
    }
}

// --------------------------- UNIT TESTS ---------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn req_with(marker: &str) -> HttpRequest {
        let mut headers = HashMap::new();
        headers.insert("X-Test".into(), format!("v-{}", marker));

        let mut query = HashMap::new();
        query.insert("q".into(), marker.to_string());

        let mut form = HashMap::new();
        form.insert("field".into(), marker.to_string());

        HttpRequest {
            method: crate::http::Method::Post,
            url: format!("http://x/{}/path", marker),
            headers,
            query,
            body: Some(Body::Form(form)),
            timeout_ms: Some(1000),
            follow_redirects: false,
        }
    }

    #[test]
    fn inject_replaces_in_url() {
        let mut r = req_with("FUZZ");
        inject(&mut r, "FUZZ", "AAA");
        assert_eq!(r.url, "http://x/AAA/path");
    }

    #[test]
    fn inject_replaces_in_headers_and_query() {
        let mut r = req_with("FUZZ");
        inject(&mut r, "FUZZ", "AAA");
        assert_eq!(r.headers["X-Test"], "v-AAA");
        assert_eq!(r.query["q"], "AAA");
    }

    #[test]
    fn inject_replaces_in_form_body() {
        let mut r = req_with("FUZZ");
        inject(&mut r, "FUZZ", "AAA");
        match r.body.unwrap() {
            Body::Form(f) => assert_eq!(f["field"], "AAA"),
            _ => panic!("esperava Form"),
        }
    }

    #[test]
    fn inject_handles_json_string_body() {
        let mut r = HttpRequest::post("http://x", Body::Json(serde_json::json!("FUZZ")));
        inject(&mut r, "FUZZ", "AAA");
        assert_eq!(r.body.unwrap(), Body::Json(serde_json::json!("AAA")));
    }

    #[test]
    fn inject_leaves_raw_body_alone() {
        let mut r = HttpRequest::post("http://x", Body::Raw(b"FUZZ".to_vec()));
        inject(&mut r, "FUZZ", "AAA");
        match r.body.unwrap() {
            Body::Raw(b) => assert_eq!(b, b"FUZZ"),
            _ => panic!("esperava Raw"),
        }
    }

    #[test]
    fn concurrency_has_floor_of_one() {
        let c = HttpClient::new().unwrap();
        let f = Fuzzer::new(c, 0);
        assert_eq!(f.concurrency(), 1);
    }

    #[test]
    fn clamps_concurrency_to_limit() {
        let client = HttpClient::new().unwrap();
        let limits = Limits { max_concurrency: 4, ..Default::default() };
        let f = Fuzzer::with_limits(client, 100, limits);
        assert_eq!(f.concurrency(), 4);
    }

    #[test]
    fn clamps_concurrency_to_minimum_one_with_limits() {
        let client = HttpClient::new().unwrap();
        let limits = Limits::default();
        let f = Fuzzer::with_limits(client, 0, limits);
        assert_eq!(f.concurrency(), 1);
    }
}