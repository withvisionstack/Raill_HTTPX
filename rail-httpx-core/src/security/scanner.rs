use super::payloads::default_sets;
use crate::environment::Environment;
use crate::http::{HttpClient, HttpRequest, HttpResponse};
use crate::limits::Limits;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity { Info, Low, Medium, High, Critical }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VulnCategory {
    SqlInjection,
    Xss,
    PathTraversal,
    Ssti,
    CommandInjection,
}

impl VulnCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            VulnCategory::SqlInjection => "SQL Injection",
            VulnCategory::Xss => "Cross-Site Scripting",
            VulnCategory::PathTraversal => "Path Traversal",
            VulnCategory::Ssti => "Server-Side Template Injection",
            VulnCategory::CommandInjection => "Command Injection",
        }
    }

    pub fn severity(&self) -> Severity {
        match self {
            VulnCategory::SqlInjection | VulnCategory::CommandInjection => Severity::Critical,
            VulnCategory::Ssti | VulnCategory::PathTraversal => Severity::High,
            VulnCategory::Xss => Severity::Medium,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Finding {
    pub category: VulnCategory,
    pub severity: Severity,
    pub url: String,
    pub parameter: String,
    pub payload: String,
    pub evidence: String,
    pub status: u16,
}

pub struct Scanner {
    client: HttpClient,
    pub max_payloads_per_category: usize,
    limits: Limits,
}

impl Scanner {
    pub fn new(client: HttpClient) -> Self {
        Self {
            client,
            max_payloads_per_category: usize::MAX,
            limits: Limits::default(),
        }
    }

    pub fn with_cap(client: HttpClient, max_payloads_per_category: usize) -> Self {
        Self {
            client,
            max_payloads_per_category: max_payloads_per_category.max(1),
            limits: Limits::default(),
        }
    }

    pub fn with_limits(
        client: HttpClient,
        max_payloads_per_category: usize,
        limits: Limits,
    ) -> Self {
        Self {
            client,
            max_payloads_per_category: max_payloads_per_category.max(1),
            limits,
        }
    }

    pub fn limits(&self) -> &Limits { &self.limits }

    pub async fn scan(&self, base: &HttpRequest, env: &Environment) -> Result<Vec<Finding>> {
        // Guarda-corpo: total planejado.
        let total_planned: usize = default_sets()
            .iter()
            .map(|s| self.max_payloads_per_category.min(s.payloads.len()))
            .sum::<usize>()
            * base.query.len().max(1);
        self.limits.check_payload_count(total_planned)?;
        self.limits.check_duration_secs(self.limits.max_duration_secs)?;

        let fut = async {
            let mut findings = Vec::new();
            for set in default_sets() {
                let take = self.max_payloads_per_category.min(set.payloads.len());
                for payload in set.payloads.iter().take(take) {
                    self.limits.check_payload(payload)?;
                    for param in base.query.keys() {
                        let mut req = base.clone();
                        req.query.insert(param.clone(), (*payload).into());

                        let resp = self.client.send(&req, env).await?;

                        if let Some(f) = detect_response(&resp, set.category, payload, param, &req.url) {
                            findings.push(f);
                        }

                        if matches!(set.category, VulnCategory::SqlInjection)
                            && resp.duration_ms > 4_500
                        {
                            findings.push(Finding {
                                category: set.category,
                                severity: Severity::High,
                                url: req.url.clone(),
                                parameter: param.clone(),
                                payload: (*payload).into(),
                                evidence: format!(
                                    "resposta demorou {}ms (time-based blind)",
                                    resp.duration_ms
                                ),
                                status: resp.status,
                            });
                        }
                    }
                }
            }
            Ok::<_, Error>(findings)
        };

        tokio::time::timeout(Duration::from_secs(self.limits.max_duration_secs), fut)
            .await
            .map_err(|_| Error::Limit(format!(
                "scan excedeu orçamento de {}s",
                self.limits.max_duration_secs
            )))?
    }
}

/// Função pura — testável isoladamente sem rede.
pub fn detect_response(
    resp: &HttpResponse,
    category: VulnCategory,
    payload: &str,
    param: &str,
    url: &str,
) -> Option<Finding> {
    let body_lc = resp.body_text().to_lowercase();
    let body_raw = resp.body_text();

    let evidence: Option<String> = match category {
        VulnCategory::SqlInjection => {
            const ERRORS: &[&str] = &[
                "you have an error in your sql syntax",
                "unclosed quotation mark",
                "sqlstate",
                "pg_query",
                "sqlite3::",
                "ora-01756",
                "syntax error at or near",
            ];
            ERRORS.iter().find(|e| body_lc.contains(**e)).map(|e| (*e).to_string())
        }
        VulnCategory::Xss => {
            let lc = payload.to_lowercase();
            if body_raw.contains(payload) || body_lc.contains(&lc) {
                Some(format!("payload refletido sem escape: {}", payload))
            } else {
                None
            }
        }
        VulnCategory::PathTraversal => {
            if body_lc.contains("root:x:0:0") || body_lc.contains("[extensions]") {
                Some("arquivo sensível retornado".into())
            } else {
                None
            }
        }
        VulnCategory::Ssti => {
            if payload.contains("7*7") && body_raw.contains("49") {
                Some("expressão avaliada pelo template engine".into())
            } else {
                None
            }
        }
        VulnCategory::CommandInjection => {
            if body_lc.contains("uid=") && body_lc.contains("gid=") {
                Some("saída de `id` no corpo da resposta".into())
            } else {
                None
            }
        }
    };

    evidence.map(|e| Finding {
        category,
        severity: category.severity(),
        url: url.to_string(),
        parameter: param.to_string(),
        payload: payload.to_string(),
        evidence: e,
        status: resp.status,
    })
}

pub fn severity_color(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "red",
        Severity::High => "yellow",
        Severity::Medium => "cyan",
        Severity::Low => "blue",
        Severity::Info => "white",
    }
}

// --------------------------- UNIT TESTS ---------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::HttpResponse;
    use std::collections::HashMap;

    fn resp(status: u16, body: &str, ms: u128) -> HttpResponse {
        HttpResponse {
            status,
            headers: HashMap::new(),
            body: body.as_bytes().to_vec(),
            duration_ms: ms,
            url: "http://x/".into(),
        }
    }

    #[test]
    fn severities_are_mapped() {
        assert_eq!(VulnCategory::SqlInjection.severity(), Severity::Critical);
        assert_eq!(VulnCategory::CommandInjection.severity(), Severity::Critical);
        assert_eq!(VulnCategory::Ssti.severity(), Severity::High);
        assert_eq!(VulnCategory::PathTraversal.severity(), Severity::High);
        assert_eq!(VulnCategory::Xss.severity(), Severity::Medium);
    }

    #[test]
    fn detects_sql_error_signature() {
        let r = resp(500, "You have an error in your SQL syntax near ...", 10);
        let f = detect_response(&r, VulnCategory::SqlInjection, "'", "id", "u").unwrap();
        assert_eq!(f.severity, Severity::Critical);
        assert!(f.evidence.contains("sql syntax"));
    }

    #[test]
    fn does_not_flag_clean_response_for_sqli() {
        let r = resp(200, "ok", 5);
        assert!(detect_response(&r, VulnCategory::SqlInjection, "'", "id", "u").is_none());
    }

    #[test]
    fn detects_reflected_xss() {
        let payload = "<script>alert(1)</script>";
        let r = resp(200, &format!("<html>{}</html>", payload), 3);
        let f = detect_response(&r, VulnCategory::Xss, payload, "q", "u").unwrap();
        assert_eq!(f.severity, Severity::Medium);
    }

    #[test]
    fn detects_path_traversal_passwd() {
        let r = resp(200, "root:x:0:0:root:/root:/bin/bash", 4);
        let f = detect_response(&r, VulnCategory::PathTraversal, "../", "f", "u").unwrap();
        assert_eq!(f.severity, Severity::High);
    }

    #[test]
    fn detects_ssti_7x7() {
        let r = resp(200, "result: 49", 2);
        let f = detect_response(&r, VulnCategory::Ssti, "{{7*7}}", "t", "u").unwrap();
        assert!(f.evidence.contains("template"));
    }

    #[test]
    fn ssti_requires_math_marker() {
        let r = resp(200, "49", 2);
        assert!(detect_response(&r, VulnCategory::Ssti, "${x}", "t", "u").is_none());
    }

    #[test]
    fn detects_command_injection_id_output() {
        let r = resp(200, "uid=0(root) gid=0(root) groups=0(root)", 5);
        let f = detect_response(&r, VulnCategory::CommandInjection, "; id", "c", "u").unwrap();
        assert_eq!(f.severity, Severity::Critical);
    }

    #[test]
    fn category_labels_are_stable() {
        assert_eq!(VulnCategory::SqlInjection.as_str(), "SQL Injection");
        assert_eq!(VulnCategory::Xss.as_str(), "Cross-Site Scripting");
    }
}