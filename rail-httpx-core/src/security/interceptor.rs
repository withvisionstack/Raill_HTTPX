//! Interceptor plugável para o proxy MITM.
//!
//! O proxy envia jobs por um canal; quem consome (o CLI, ou um teste)
//! decide se encaminha, modifica ou descarta. O canal serializa a
//! interação — mesmo com N requests em paralelo, o usuário vê uma por vez.
//!
//! Contrato: `intercept` **empresta** a request. Devolver
//! `Forward(None)` = usa a original; `Forward(Some(b))` = substitui;
//! `Drop` = descarta. Assim o chamador decide sem clonar no hot path.

use crate::{Error, Result};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

/// Decisão sobre uma request interceptada.
#[derive(Debug, Clone)]
pub enum InterceptOutcome {
    /// `None` = mantém a request original; `Some(b)` = substitui os bytes.
    Forward(Option<Vec<u8>>),
    /// Descarta — o cliente vê a conexão fechada.
    Drop,
}

/// Job enviado ao interceptor.
#[derive(Debug)]
pub struct InterceptJob {
    pub host: String,
    pub port: u16,
    pub request: Vec<u8>,
    pub respond: oneshot::Sender<InterceptOutcome>,
}

/// Interceptor: casa com hosts por pattern e delega a decisão.
///
/// `pattern = None` significa "casa com tudo".
pub struct Interceptor {
    pattern: Option<String>,
    tx: mpsc::UnboundedSender<InterceptJob>,
}

impl Interceptor {
    /// Cria interceptor + receiver. Quem consome o receiver é responsável
    /// por responder cada job (senão o proxy trava esperando).
    pub fn new(pattern: Option<String>) -> (Arc<Self>, mpsc::UnboundedReceiver<InterceptJob>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Arc::new(Self { pattern, tx }), rx)
    }

    /// `true` se este host deve ser interceptado. Pura.
    pub fn matches_host(&self, host: &str) -> bool {
        match &self.pattern {
            None => true,
            Some(p) => host.contains(p.as_str()),
        }
    }

    /// Chamado pelo proxy por request. **Empresta** a request —
    /// só clona se for de fato interceptar.
    pub async fn intercept(
        &self,
        host: &str,
        port: u16,
        request: &[u8],
    ) -> Result<InterceptOutcome> {
        if !self.matches_host(host) {
            return Ok(InterceptOutcome::Forward(None));
        }
        let (respond, rx) = oneshot::channel();
        self.tx
            .send(InterceptJob {
                host: host.to_string(),
                port,
                request: request.to_vec(), // único clone do caminho
                respond,
            })
            .map_err(|_| Error::Other("interceptor fechado".into()))?;
        rx.await
            .map_err(|_| Error::Other("interceptor dropado sem responder".into()))
    }
}

// --------------------------- UNIT TESTS ---------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_pattern_matches_everything() {
        let (ic, _rx) = Interceptor::new(None);
        assert!(ic.matches_host("api.github.com"));
        assert!(ic.matches_host("localhost"));
        assert!(ic.matches_host(""));
    }

    #[test]
    fn pattern_matches_by_substring() {
        let (ic, _rx) = Interceptor::new(Some("api.".into()));
        assert!(ic.matches_host("api.github.com"));
        assert!(ic.matches_host("api.stripe.com"));
        assert!(!ic.matches_host("github.com"));
    }

    #[test]
    fn pattern_matches_exact_host() {
        let (ic, _rx) = Interceptor::new(Some("localhost".into()));
        assert!(ic.matches_host("localhost"));
        assert!(ic.matches_host("localhost:3000"));
    }

    #[tokio::test]
    async fn non_matching_host_passes_through_with_none() {
        let (ic, _rx) = Interceptor::new(Some("api.".into()));
        let req = b"GET / HTTP/1.1\r\n\r\n";

        let outcome = ic.intercept("other.com", 443, req).await.unwrap();
        match outcome {
            InterceptOutcome::Forward(None) => {}
            other => panic!("esperava Forward(None), veio {other:?}"),
        }
    }

    #[tokio::test]
    async fn matching_host_goes_through_channel_and_can_modify() {
        let (ic, mut rx) = Interceptor::new(None);

        let responder = tokio::spawn(async move {
            let job = rx.recv().await.unwrap();
            assert_eq!(job.host, "api.x");
            assert_eq!(job.port, 443);
            let mut modified = job.request.clone();
            modified.extend_from_slice(b"X-Injected: 1\r\n");
            let _ = job.respond.send(InterceptOutcome::Forward(Some(modified)));
        });

        let original = b"GET / HTTP/1.1\r\n\r\n";
        let outcome = ic.intercept("api.x", 443, original).await.unwrap();
        responder.await.unwrap();

        match outcome {
            InterceptOutcome::Forward(Some(bytes)) => {
                assert!(bytes.len() > original.len());
                assert!(bytes.windows(12).any(|w| w == b"X-Injected: "));
            }
            other => panic!("esperava Forward(Some), veio {other:?}"),
        }
    }

    #[tokio::test]
    async fn forward_none_means_use_original() {
        let (ic, mut rx) = Interceptor::new(None);
        tokio::spawn(async move {
            let job = rx.recv().await.unwrap();
            let _ = job.respond.send(InterceptOutcome::Forward(None));
        });

        let outcome = ic.intercept("x", 443, b"GET / HTTP/1.1\r\n\r\n").await.unwrap();
        assert!(matches!(outcome, InterceptOutcome::Forward(None)));
    }

    #[tokio::test]
    async fn dropped_response_propagates() {
        let (ic, mut rx) = Interceptor::new(None);
        tokio::spawn(async move {
            let job = rx.recv().await.unwrap();
            let _ = job.respond.send(InterceptOutcome::Drop);
        });

        let outcome = ic.intercept("x", 443, b"").await.unwrap();
        assert!(matches!(outcome, InterceptOutcome::Drop));
    }
}