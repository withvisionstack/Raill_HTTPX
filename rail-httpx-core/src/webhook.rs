//! Servidor HTTP local para receber webhooks.
//!
//! Escuta numa porta, aceita qualquer request, loga e responde 200 OK.
//! É o mesmo padrão do proxy MITM, mas **sem TLS** e sem encaminhar —
//! a request termina aqui.
//!
//! Uso típico: você cola o endpoint no Stripe/GitHub/serviço externo
//! e vê o payload chegando em tempo real na GUI.

use crate::{Error, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

/// Request recebida por um webhook.
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub id: u64,
    pub timestamp: SystemTime,
    pub remote_addr: SocketAddr,
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Canal de eventos + contador.
#[derive(Clone)]
pub struct WebhookSink {
    tx: mpsc::UnboundedSender<WebhookEvent>,
    next_id: Arc<AtomicU64>,
}

impl WebhookSink {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<WebhookEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                tx,
                next_id: Arc::new(AtomicU64::new(1)),
            },
            rx,
        )
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}

/// Servidor de webhook.
pub struct WebhookServer {
    listener: TcpListener,
    sink: Option<WebhookSink>,
    /// Teto de bytes lidos por request (headers + body).
    max_request_bytes: usize,
}

impl WebhookServer {
    pub async fn bind(addr: &str) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            listener,
            sink: None,
            max_request_bytes: 1024 * 1024, // 1 MiB default
        })
    }

    pub fn with_events(mut self, sink: WebhookSink) -> Self {
        self.sink = Some(sink);
        self
    }

    pub fn with_max_request_bytes(mut self, max: usize) -> Self {
        self.max_request_bytes = max;
        self
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr().map_err(Into::into)
    }

    pub async fn run(self) -> Result<()> {
        loop {
            let (stream, peer) = self.listener.accept().await?;
            let sink = self.sink.clone();
            let max = self.max_request_bytes;
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, peer, sink, max).await {
                    tracing::debug!("webhook conn error: {e}");
                }
            });
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    sink: Option<WebhookSink>,
    max_bytes: usize,
) -> Result<()> {
    let mut reader = BufReader::new(stream);

    let raw = read_request_with_limit(&mut reader, max_bytes).await?;
    if raw.is_empty() {
        return Ok(());
    }

    // Parseia o request (linha + headers + body) já lido.
    let parsed = parse_raw_request(&raw);
    tracing::info!(
        "webhook <- {} {} {} ({} bytes)",
        peer,
        parsed.method,
        parsed.path,
        parsed.body.len()
    );

    if let Some(sink) = &sink {
        let _ = sink.tx.send(WebhookEvent {
            id: sink.next_id(),
            timestamp: SystemTime::now(),
            remote_addr: peer,
            method: parsed.method,
            path: parsed.path,
            headers: parsed.headers,
            body: parsed.body,
        });
    }

    // Resposta sempre 200 OK — webhooks esperam isso pra marcar como entregue.
    let mut stream = reader.into_inner();
    let body = b"OK";
    let resp = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         ",
        body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

/// Lê uma mensagem HTTP completa (headers + body) com teto de tamanho.
/// Suporta `Content-Length` e `Transfer-Encoding: chunked`.
async fn read_request_with_limit<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(1024);
    let mut content_length: Option<usize> = None;
    let mut chunked = false;

    // -------- headers --------
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        if out.len() + line.len() > max_bytes {
            return Err(Error::Limit(format!(
                "headers excedem {max_bytes} bytes"
            )));
        }
        out.extend_from_slice(line.as_bytes());
        if line == "\r\n" || line == "\n" {
            break;
        }

        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse().ok();
        } else if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            chunked = true;
        }
    }

    // -------- body --------
    if chunked {
        loop {
            let mut size_line = String::new();
            reader.read_line(&mut size_line).await?;
            out.extend_from_slice(size_line.as_bytes());

            let size_str = size_line.trim();
            let size = usize::from_str_radix(size_str.split(';').next().unwrap_or("0").trim(), 16)
                .unwrap_or(0);

            if size == 0 {
                let mut end = String::new();
                reader.read_line(&mut end).await?;
                out.extend_from_slice(end.as_bytes());
                break;
            }

            if out.len() + size + 2 > max_bytes {
                return Err(Error::Limit(format!(
                    "body excede {max_bytes} bytes (chunked)"
                )));
            }

            let mut chunk = vec![0u8; size + 2];
            reader.read_exact(&mut chunk).await?;
            out.extend_from_slice(&chunk);
        }
    } else if let Some(n) = content_length {
        let n = n.min(max_bytes.saturating_sub(out.len()));
        let mut body = vec![0u8; n];
        reader.read_exact(&mut body).await?;
        out.extend_from_slice(&body);
    }

    Ok(out)
}

// --------------------------- Parser puro ---------------------------

#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Parseia uma request HTTP crua em partes. Pura — testável sem rede.
pub fn parse_raw_request(raw: &[u8]) -> ParsedRequest {
    // Encontra \r\n\r\n (ou \n\n) que separa headers de body.
    let (headers_end, body_start) = match find_headers_end(raw) {
        Some((h, b)) => (h, b),
        None => (raw.len(), raw.len()),
    };

    let headers_blob = String::from_utf8_lossy(&raw[..headers_end]);
    let mut lines = headers_blob.lines();

    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("?").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }

    let body = if body_start < raw.len() {
        raw[body_start..].to_vec()
    } else {
        Vec::new()
    };

    ParsedRequest {
        method,
        path,
        headers,
        body,
    }
}

/// Retorna `(header_end_offset, body_start_offset)`.
fn find_headers_end(raw: &[u8]) -> Option<(usize, usize)> {
    // \r\n\r\n
    if let Some(pos) = find_subseq(raw, b"\r\n\r\n") {
        return Some((pos, pos + 4));
    }
    // \n\n (tolerante)
    find_subseq(raw, b"\n\n").map(|pos| (pos, pos + 2))
}

fn find_subseq(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

// --------------------------- UNIT TESTS ---------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_post_with_body() {
        let raw = b"POST /hook HTTP/1.1\r\n\
                    Host: localhost\r\n\
                    Content-Type: application/json\r\n\
                    Content-Length: 13\r\n\
                    \r\n\
                    {\"a\":1,\"b\":2}";
        let p = parse_raw_request(raw);
        assert_eq!(p.method, "POST");
        assert_eq!(p.path, "/hook");
        assert_eq!(p.headers.len(), 3);
        assert_eq!(p.headers[0], ("Host".into(), "localhost".into()));
        assert_eq!(p.body, b"{\"a\":1,\"b\":2}");
    }

    #[test]
    fn parses_get_without_body() {
        let raw = b"GET /health HTTP/1.1\r\nHost: x\r\n\r\n";
        let p = parse_raw_request(raw);
        assert_eq!(p.method, "GET");
        assert_eq!(p.path, "/health");
        assert!(p.body.is_empty());
    }

    #[test]
    fn parses_empty_raw() {
        let p = parse_raw_request(b"");
        assert_eq!(p.method, "?");
        assert_eq!(p.path, "/");
        assert!(p.body.is_empty());
    }

    #[test]
    fn tolerates_lf_only_line_endings() {
        let raw = b"POST /x HTTP/1.1\nHost: y\n\nhello";
        let p = parse_raw_request(raw);
        assert_eq!(p.method, "POST");
        assert_eq!(p.path, "/x");
        assert_eq!(p.body, b"hello");
    }

    #[test]
    fn headers_with_colon_in_value() {
        let raw = b"POST /x HTTP/1.1\r\n\
                    X-Time: 12:34:56\r\n\
                    \r\n";
        let p = parse_raw_request(raw);
        let time = p
            .headers
            .iter()
            .find(|(k, _)| k == "X-Time")
            .expect("header X-Time");
        assert_eq!(time.1, "12:34:56");
    }

    #[test]
    fn sink_increments_ids() {
        let (sink, _rx) = WebhookSink::new();
        assert_eq!(sink.next_id(), 1);
        assert_eq!(sink.next_id(), 2);
    }

    #[tokio::test]
    async fn read_with_limit_truncates_oversized_body() {
        // Body real com 2000 bytes, mas teto de 500.
        // A função deve ler até o teto, sem tentar alocar mais.
        let body = vec![b'A'; 2000];
        let mut raw = format!(
            "POST /x HTTP/1.1\r\nContent-Length: 2000\r\n\r\n"
        )
            .into_bytes();
        raw.extend_from_slice(&body);

        let mut reader = BufReader::new(&raw[..]);
        let result = read_request_with_limit(&mut reader, 500).await;

        assert!(result.is_ok(), "deveria ler até o teto sem erro");
        let out = result.unwrap();
        assert!(
            out.len() <= 500,
            "saída deve respeitar o teto (obtido {} bytes)",
            out.len()
        );
        // Headers chegaram intactos.
        assert!(out.starts_with(b"POST /x HTTP/1.1"));
    }

    #[tokio::test]
    async fn read_with_limit_allows_body_under_cap() {
        let body = b"hello world";
        let mut raw = format!(
            "POST /x HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
            .into_bytes();
        raw.extend_from_slice(body);

        let mut reader = BufReader::new(&raw[..]);
        let result = read_request_with_limit(&mut reader, 1024).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.ends_with(b"hello world"), "body deveria estar intacto");
    }
}