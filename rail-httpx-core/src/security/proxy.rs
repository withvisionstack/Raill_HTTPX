//! Servidor proxy MITM estilo Burp.
//!
//! Fluxo:
//!   1. Aceita TCP
//!   2. Lê `CONNECT host:port HTTP/1.1`
//!   3. Responde `HTTP/1.1 200 Connection Established`
//!   4. Handshake TLS com o cliente usando leaf emitido pelo CA local
//!   5. Lê request HTTP de dentro do TLS
//!   6. Abre conexão com o backend real
//!      - porta 443 → TLS (verificado contra CAs públicas webpki-roots)
//!      - outras portas → TCP puro
//!   7. Encaminha bytes crus request→backend, response→cliente
//!
//! **Eventos**: se um `EventSink` foi fornecido, cada request gera um
//! `ProxyEvent` no canal, com request e response crus. É assim que a
//! GUI mostra o tráfego em tempo real.

use crate::security::ca::{CertPair, CertificateAuthority};
use crate::{Error, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// Evento emitido a cada request interceptada pelo proxy.
#[derive(Debug, Clone)]
pub struct ProxyEvent {
    pub id: u64,
    pub timestamp: SystemTime,
    pub host: String,
    pub port: u16,
    pub request: Vec<u8>,
    pub response: Option<Vec<u8>>,
    pub dropped: bool,
}

/// Canal de eventos + contador global de ids.
#[derive(Clone)]
pub struct EventSink {
    tx: mpsc::UnboundedSender<ProxyEvent>,
    next_id: Arc<AtomicU64>,
}

impl EventSink {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ProxyEvent>) {
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

trait BackendStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T> BackendStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

pub fn should_use_tls_for_backend(port: u16) -> bool {
    port == 443
}

pub fn parse_connect_line(line: &str) -> Option<(String, u16)> {
    let line = line.trim_end_matches(['\r', '\n']);
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    if method != "CONNECT" {
        return None;
    }
    let target = parts.next()?;
    let (host, port) = target.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port))
}

pub async fn read_http_message<R>(reader: &mut R) -> Result<Vec<u8>>
where
    R: AsyncBufRead + Unpin,
{
    let mut out = Vec::with_capacity(1024);
    let mut content_length: Option<usize> = None;
    let mut chunked = false;

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
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

            let mut chunk = vec![0u8; size + 2];
            reader.read_exact(&mut chunk).await?;
            out.extend_from_slice(&chunk);
        }
    } else if let Some(n) = content_length {
        let mut body = vec![0u8; n];
        reader.read_exact(&mut body).await?;
        out.extend_from_slice(&body);
    }

    Ok(out)
}

pub struct ProxyServer {
    listener: TcpListener,
    ca: Arc<CertificateAuthority>,
    interceptor: Option<Arc<crate::security::interceptor::Interceptor>>,
    events: Option<EventSink>,
}

impl ProxyServer {
    pub async fn bind(addr: &str, ca: Arc<CertificateAuthority>) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            listener,
            ca,
            interceptor: None,
            events: None,
        })
    }

    pub async fn bind_with_interceptor(
        addr: &str,
        ca: Arc<CertificateAuthority>,
        interceptor: Arc<crate::security::interceptor::Interceptor>,
    ) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            listener,
            ca,
            interceptor: Some(interceptor),
            events: None,
        })
    }

    /// Anexa um sink de eventos. Consumidor faz `.with_events(sink)` depois
    /// de construir o servidor. Encadeável.
    pub fn with_events(mut self, sink: EventSink) -> Self {
        self.events = Some(sink);
        self
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr().map_err(Into::into)
    }

    pub async fn run(self) -> Result<()> {
        loop {
            let (stream, _peer) = self.listener.accept().await?;
            let ca = self.ca.clone();
            let ic = self.interceptor.clone();
            let ev = self.events.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, ca, ic, ev).await {
                    tracing::debug!("proxy conn error: {e}");
                }
            });
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    ca: Arc<CertificateAuthority>,
    interceptor: Option<Arc<crate::security::interceptor::Interceptor>>,
    events: Option<EventSink>,
) -> Result<()> {
    use crate::security::interceptor::InterceptOutcome;

    let mut reader = BufReader::new(stream);

    let mut first = String::new();
    reader.read_line(&mut first).await?;
    let (host, port) = parse_connect_line(&first)
        .ok_or_else(|| Error::Other(format!("método não suportado: {}", first.trim())))?;

    loop {
        let mut h = String::new();
        let n = reader.read_line(&mut h).await?;
        if n == 0 || h == "\r\n" || h == "\n" {
            break;
        }
    }

    let leaf = ca.leaf_for(&host)?;

    let mut stream = reader.into_inner();
    stream
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;
    stream.flush().await?;

    let acceptor = build_server_acceptor(&leaf)?;
    let tls_stream = acceptor.accept(stream).await?;

    let mut tls_reader = BufReader::new(tls_stream);
    let request_bytes = read_http_message(&mut tls_reader).await?;
    if request_bytes.is_empty() {
        return Ok(());
    }

    let event_id = events.as_ref().map(|e| e.next_id());
    let event_ts = SystemTime::now();

    let request_bytes = match &interceptor {
        Some(ic) => match ic.intercept(&host, port, &request_bytes).await? {
            InterceptOutcome::Forward(Some(b)) => b,
            InterceptOutcome::Forward(None) => request_bytes,
            InterceptOutcome::Drop => {
                tracing::info!("MITM x  {}:{} (dropado)", host, port);
                if let (Some(sink), Some(id)) = (&events, event_id) {
                    let _ = sink.tx.send(ProxyEvent {
                        id,
                        timestamp: event_ts,
                        host,
                        port,
                        request: request_bytes,
                        response: None,
                        dropped: true,
                    });
                }
                return Ok(());
            }
        },
        None => request_bytes,
    };

    tracing::info!("MITM <- {}:{} ({} bytes)", host, port, request_bytes.len());

    let mut backend = connect_backend(&host, port).await?;
    backend.write_all(&request_bytes).await?;
    backend.flush().await?;

    let mut backend_reader = BufReader::new(backend);
    let response_bytes = read_http_message(&mut backend_reader).await?;
    tracing::info!("MITM -> {}:{} ({} bytes)", host, port, response_bytes.len());

    if let (Some(sink), Some(id)) = (&events, event_id) {
        let _ = sink.tx.send(ProxyEvent {
            id,
            timestamp: event_ts,
            host: host.clone(),
            port,
            request: request_bytes.clone(),
            response: Some(response_bytes.clone()),
            dropped: false,
        });
    }

    let mut tls_stream = tls_reader.into_inner();
    tls_stream.write_all(&response_bytes).await?;
    tls_stream.flush().await?;

    Ok(())
}

async fn connect_backend(host: &str, port: u16) -> Result<Box<dyn BackendStream>> {
    let tcp = TcpStream::connect((host, port))
        .await
        .map_err(|e| Error::Other(format!("connect backend {host}:{port}: {e}")))?;

    if should_use_tls_for_backend(port) {
        let connector = build_client_connector()?;
        let server_name = ServerName::try_from(host.to_string())
            .map_err(|e| Error::Other(format!("SNI inválido para {host}: {e}")))?;
        let tls = connector
            .connect(server_name, tcp)
            .await
            .map_err(|e| Error::Other(format!("TLS backend {host}:{port}: {e}")))?;
        Ok(Box::new(tls))
    } else {
        Ok(Box::new(tcp))
    }
}

fn build_server_acceptor(leaf: &CertPair) -> Result<TlsAcceptor> {
    let certs = load_certs_from_pem(&leaf.cert_pem)?;
    let key = load_key_from_pem(&leaf.key_pem)?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| Error::Other(format!("rustls server config: {e}")))?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

fn build_client_connector() -> Result<TlsConnector> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

fn load_certs_from_pem(pem: &str) -> Result<Vec<CertificateDer<'static>>> {
    let mut cursor = std::io::Cursor::new(pem.as_bytes());
    let certs: std::result::Result<Vec<_>, _> = rustls_pemfile::certs(&mut cursor).collect();
    certs.map_err(|e| Error::Other(format!("PEM cert: {e}")))
}

fn load_key_from_pem(pem: &str) -> Result<PrivateKeyDer<'static>> {
    let mut cursor = std::io::Cursor::new(pem.as_bytes());
    rustls_pemfile::private_key(&mut cursor)
        .map_err(|e| Error::Other(format!("PEM key: {e}")))?
        .ok_or_else(|| Error::Other("nenhuma chave privada no PEM".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_connect_with_crlf() {
        assert_eq!(
            parse_connect_line("CONNECT example.com:443 HTTP/1.1\r\n"),
            Some(("example.com".into(), 443))
        );
    }

    #[test]
    fn parses_connect_without_trailing_crlf() {
        assert_eq!(
            parse_connect_line("CONNECT example.com:443 HTTP/1.1"),
            Some(("example.com".into(), 443))
        );
    }

    #[test]
    fn parses_non_standard_port() {
        assert_eq!(
            parse_connect_line("CONNECT internal.local:8443 HTTP/1.1\r\n"),
            Some(("internal.local".into(), 8443))
        );
    }

    #[test]
    fn rejects_get_method() {
        assert!(parse_connect_line("GET / HTTP/1.1\r\n").is_none());
    }

    #[test]
    fn rejects_post_method() {
        assert!(parse_connect_line("POST /x HTTP/1.1\r\n").is_none());
    }

    #[test]
    fn rejects_missing_port() {
        assert!(parse_connect_line("CONNECT example.com HTTP/1.1\r\n").is_none());
    }

    #[test]
    fn rejects_invalid_port() {
        assert!(parse_connect_line("CONNECT example.com:abc HTTP/1.1\r\n").is_none());
    }

    #[test]
    fn rejects_empty_host() {
        assert!(parse_connect_line("CONNECT :443 HTTP/1.1\r\n").is_none());
    }

    #[test]
    fn rejects_empty_line() {
        assert!(parse_connect_line("").is_none());
    }

    #[test]
    fn port_443_uses_tls() {
        assert!(should_use_tls_for_backend(443));
    }

    #[test]
    fn port_80_is_plain_tcp() {
        assert!(!should_use_tls_for_backend(80));
    }

    #[test]
    fn local_ports_are_plain_tcp() {
        assert!(!should_use_tls_for_backend(3000));
        assert!(!should_use_tls_for_backend(8080));
        assert!(!should_use_tls_for_backend(0));
    }

    #[test]
    fn event_sink_increments_ids() {
        let (sink, _rx) = EventSink::new();
        assert_eq!(sink.next_id(), 1);
        assert_eq!(sink.next_id(), 2);
        assert_eq!(sink.next_id(), 3);
    }
}