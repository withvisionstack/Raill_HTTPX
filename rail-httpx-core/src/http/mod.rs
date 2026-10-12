pub mod client;

pub use client::HttpClient;   
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method { Get, Post, Put, Patch, Delete, Head, Options }

impl Default for Method { fn default() -> Self { Method::Get } }

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Body {
    Json(serde_json::Value),
    Text(String),
    Form(HashMap<String, String>),
    Raw(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpRequest {
    #[serde(default)]
    pub method: Method,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub query: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<Body>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub follow_redirects: bool,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            url: url.into(),
            headers: HashMap::new(),
            query: HashMap::new(),
            body: None,
            timeout_ms: Some(30_000),
            follow_redirects: false,
        }
    }

    pub fn post(url: impl Into<String>, body: Body) -> Self {
        let mut r = Self::get(url);
        r.method = Method::Post;
        r.body = Some(body);
        r
    }

    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.headers.insert(k.into(), v.into());
        self
    }

    pub fn query(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.query.insert(k.into(), v.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    #[serde(skip)]
    pub body: Vec<u8>,
    pub duration_ms: u128,
    pub url: String,
}

impl HttpResponse {
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    pub fn body_json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Method ----------------------------------------------------

    #[test]
    fn method_as_str_uppercase() {
        assert_eq!(Method::Get.as_str(), "GET");
        assert_eq!(Method::Delete.as_str(), "DELETE");
    }

    #[test]
    fn default_method_is_get() {
        assert_eq!(Method::default(), Method::Get);
    }

    #[test]
    fn method_serializes_uppercase() {
        let s = serde_json::to_string(&Method::Post).unwrap();
        assert_eq!(s, "\"POST\"");
    }

    // ---- HttpRequest builder --------------------------------------

    #[test]
    fn get_sets_sane_defaults() {
        let r = HttpRequest::get("https://x");
        assert_eq!(r.method, Method::Get);
        assert_eq!(r.url, "https://x");
        assert!(r.headers.is_empty());
        assert!(r.query.is_empty());
        assert!(r.body.is_none());
        assert_eq!(r.timeout_ms, Some(30_000));
        assert!(!r.follow_redirects);
    }

    #[test]
    fn builder_adds_header_and_query() {
        let r = HttpRequest::get("https://x")
            .header("Accept", "application/json")
            .query("q", "rust");
        assert_eq!(r.headers.get("Accept").unwrap(), "application/json");
        assert_eq!(r.query.get("q").unwrap(), "rust");
    }

    #[test]
    fn post_builder_sets_body() {
        let r = HttpRequest::post("https://x", Body::Text("hi".into()));
        assert_eq!(r.method, Method::Post);
        assert!(matches!(r.body, Some(Body::Text(ref t)) if t == "hi"));
    }

    // ---- HttpResponse ---------------------------------------------

    #[test]
    fn response_text_and_json() {
        let resp = HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: br#"{"ok":true}"#.to_vec(),
            duration_ms: 5,
            url: "https://x".into(),
        };
        assert_eq!(resp.body_text(), "{\"ok\":true}");
        assert_eq!(resp.body_json().unwrap()["ok"], true);
    }

    #[test]
    fn response_invalid_json_returns_none() {
        let resp = HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: b"not-json".to_vec(),
            duration_ms: 1,
            url: "x".into(),
        };
        assert!(resp.body_json().is_none());
    }
}