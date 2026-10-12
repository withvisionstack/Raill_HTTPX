

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use colored::*;
use rail_httpx_core::db::Database;
use rail_httpx_core::environment::Environment;
use rail_httpx_core::http::{Body, HttpClient, HttpRequest, Method};
use rail_httpx_core::limits::Limits;
use rail_httpx_core::security::{
    default_ca_dir, CertificateAuthority, Fuzzer, InterceptJob, InterceptOutcome, Interceptor,
    ProxyServer, Scanner,
};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};


#[derive(Parser)]
#[command(
    name = "rail-httpx",
    version,
    about = "RAIL_HTTPX — Postman leve + Burp core, em Rust"
)]
struct Cli {
    #[command(flatten)]
    limits: LimitsArgs,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Args, Clone, Debug)]
struct LimitsArgs {
    #[arg(long, default_value_t = 50_000, global = true)]
    max_payloads: usize,

    #[arg(long, default_value_t = 65536, global = true)]
    max_payload_bytes: usize,

    #[arg(long, default_value_t = 16777216, global = true)]
    max_response_bytes: usize,

    #[arg(long, default_value_t = 128, global = true)]
    max_concurrency: usize,

    #[arg(long, default_value_t = 3600, global = true)]
    max_duration_secs: u64,
}

impl From<&LimitsArgs> for Limits {
    fn from(a: &LimitsArgs) -> Self {
        Limits {
            max_payloads: a.max_payloads,
            max_payload_bytes: a.max_payload_bytes,
            max_response_bytes: a.max_response_bytes,
            max_concurrency: a.max_concurrency,
            max_duration_secs: a.max_duration_secs,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Envia uma requisição HTTP única (Send do Postman).
    Send {
        url: String,
        #[arg(short = 'X', long, default_value = "GET")]
        method: String,
        #[arg(short = 'H', long = "header", value_parser = parse_kv)]
        headers: Vec<(String, String)>,
        /// Body inline. Ignorado se `--data-file` for passado.
        #[arg(short = 'd', long)]
        data: Option<String>,
        /// Lê o body de um arquivo (evita briga com escaping de shell).
        #[arg(long)]
        data_file: Option<String>,
        #[arg(long)]
        insecure: bool,
    },
    /// Escaneia parâmetros em busca de vulnerabilidades comuns.
    Scan {
        url: String,
        #[arg(short = 'p', long = "param")]
        params: Vec<String>,
        #[arg(short = 'H', long = "header", value_parser = parse_kv)]
        headers: Vec<(String, String)>,
        #[arg(long)]
        insecure: bool,
    },
    /// Fuzzing com marcador FUZZ (Intruder do Burp).
    Fuzz {
        url: String,
        #[arg(short = 'w', long)]
        wordlist: Option<String>,
        #[arg(short = 'c', long, default_value_t = 20)]
        concurrency: usize,
        #[arg(long)]
        insecure: bool,
    },
    /// Executa queries em PostgreSQL / MySQL / SQLite.
    Db {
        #[arg(long)]
        url: String,
        #[arg(short = 'q', long)]
        sql: String,
    },
    /// Gerencia a Autoridade Certificadora local (para o proxy MITM).
    Ca {
        #[command(subcommand)]
        action: CaAction,
    },
    /// Sobe o proxy MITM em 127.0.0.1:<porta>.
    Proxy {
        #[arg(short = 'p', long, default_value_t = 8080)]
        port: u16,
        #[arg(short = 'v', long)]
        verbose: bool,
        /// Intercepta requests cujo host casa com este pattern (substring).
        #[arg(short = 'i', long)]
        intercept: Option<String>,
    },
}

#[derive(Subcommand)]
enum CaAction {
    /// Mostra o diretório onde o CA vive.
    Path,
    /// Exporta o certificado público para instalar no navegador/sistema.
    Export {
        #[arg(short = 'o', long, default_value = "rail-httpx-ca.pem")]
        out: String,
    },
}

fn parse_kv(s: &str) -> Result<(String, String), String> {
    let (k, v) = s.split_once(':').ok_or("use 'Chave: valor'")?;
    Ok((k.trim().to_string(), v.trim().to_string()))
}

fn make_client(insecure: bool, max_response_bytes: usize) -> Result<HttpClient> {
    if insecure {
        Ok(HttpClient::insecure_with_max_response_bytes(max_response_bytes)?)
    } else {
        Ok(HttpClient::with_max_response_bytes(max_response_bytes)?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let limits: Limits = (&cli.limits).into();
    let env = Environment::new("default");

    let default_filter = match &cli.cmd {
        Cmd::Proxy { verbose, .. } => {
            if *verbose {
                "rail_httpx_core=debug,info"
            } else {
                "rail_httpx_core::security::proxy=info,warn"
            }
        }
        _ => "warn",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();

    match cli.cmd {
        Cmd::Send { url, method, headers, data, data_file, insecure } => {
            let client = make_client(insecure, limits.max_response_bytes)?;
            let method = parse_method(&method);
            let mut req = HttpRequest { method, ..HttpRequest::get(&url) };
            for (k, v) in headers {
                req.headers.insert(k, v);
            }
            // `--data-file` tem prioridade sobre `-d` se ambos vierem.
            if let Some(path) = data_file {
                let body = std::fs::read_to_string(&path)
                    .map_err(|e| anyhow::anyhow!("lendo {path}: {e}"))?;
                req.body = Some(Body::Text(body));
            } else if let Some(d) = data {
                req.body = Some(Body::Text(d));
            }
            let resp = client.send(&req, &env).await?;
            print_response(&resp);
        }
        Cmd::Scan { url, params, headers, insecure } => {
            let client = make_client(insecure, limits.max_response_bytes)?;

            // Tira a query string da URL; params vão pelo campo `query`.
            let parsed = url::Url::parse(&url)
                .map_err(|e| anyhow::anyhow!("URL inválida: {e}"))?;
            let mut clean = parsed.clone();
            clean.set_query(None);

            let mut req = HttpRequest::get(clean.as_str());
            for (k, v) in headers {
                req.headers.insert(k, v);
            }
            for p in &params {
                req.query.insert(p.clone(), "1".into());
            }

            let scanner = Scanner::with_limits(client, usize::MAX, limits);
            let findings = scanner.scan(&req, &env).await?;
            if findings.is_empty() {
                println!("{}", "nenhuma vulnerabilidade óbvia encontrada.".yellow());
            } else {
                for f in findings {
                    let sev = format!("{:?}", f.severity).to_uppercase();
                    println!(
                        "{} [{}] {} ({})",
                        sev.red().bold(),
                        f.category.as_str(),
                        f.url,
                        f.parameter
                    );
                    println!("  payload:   {}", f.payload);
                    println!("  evidência: {}\n", f.evidence);
                }
            }
        }

        Cmd::Fuzz { url, wordlist, concurrency, insecure } => {
            let client = make_client(insecure, limits.max_response_bytes)?;
            let payloads: Vec<String> = match wordlist {
                Some(path) => std::fs::read_to_string(path)?
                    .lines()
                    .map(str::to_string)
                    .filter(|l| !l.is_empty())
                    .collect(),
                None => vec![
                    "admin".into(), "test".into(), "user".into(),
                    "'".into(), "\"".into(), "../".into(),
                ],
            };
            let template = HttpRequest::get(&url);
            let fuzzer = Fuzzer::with_limits(client, concurrency, limits);
            let results = fuzzer.fuzz(&template, &env, "FUZZ", &payloads).await?;

            println!("{:<30} {:>6} {:>8} {:>8}", "PAYLOAD", "STATUS", "LEN", "TIME(ms)");
            for r in results {
                println!(
                    "{:<30} {:>6} {:>8} {:>8}",
                    truncate(&r.payload, 30),
                    r.status,
                    r.length,
                    r.duration_ms
                );
            }
        }

        Cmd::Db { url, sql } => {
            let db = Database::connect(&url).await?;
            let out = db.query(&sql).await?;
            if let Some(n) = out.affected {
                println!("{} linhas afetadas em {}ms", n, out.duration_ms);
            } else {
                println!("{} linhas em {}ms", out.rows.len(), out.duration_ms);
                println!("{}", serde_json::to_string_pretty(&out.rows)?);
            }
        }

        Cmd::Ca { action } => match action {
            CaAction::Path => {
                println!("{}", default_ca_dir().display());
            }
            CaAction::Export { out } => {
                let ca = CertificateAuthority::new_or_load(default_ca_dir())?;
                std::fs::write(&out, ca.ca_cert_pem())?;
                let abs = std::fs::canonicalize(&out).unwrap_or_else(|_| out.clone().into());
                println!("{} CA exportado para {}", "✓".green().bold(), abs.display());
                println!();
                println!("{}", "Próximos passos:".bold());
                println!("  1. Firefox → Preferências → Privacidade & Segurança");
                println!("  2. Seção Certificados → Ver Certificados… → Autoridades");
                println!("  3. Importar… → selecione o arquivo acima");
                println!("  4. Marque \"Confiar nesta CA para identificar sites\"");
                println!();
                println!("Chrome/Windows: duplo-clique no arquivo → Instalar Certificado");
                println!("                → Autoridades Raiz Confiáveis");
            }
        },

        Cmd::Proxy { port, verbose: _, intercept } => {
            let ca = Arc::new(CertificateAuthority::new_or_load(default_ca_dir())?);
            let addr = format!("127.0.0.1:{}", port);

            let server = match intercept {
                None => ProxyServer::bind(&addr, ca).await?,
                Some(pattern) => {
                    let (ic, rx) = Interceptor::new(Some(pattern.clone()));
                    tokio::spawn(run_terminal_interceptor(rx));
                    println!(
                        "{} Interceptando hosts que contenham: {}",
                        "●".yellow().bold(),
                        pattern.cyan()
                    );
                    ProxyServer::bind_with_interceptor(&addr, ca, ic).await?
                }
            };
            let local = server.local_addr()?;

            println!(
                "{} Proxy MITM ouvindo em {}",
                "✓".green().bold(),
                local.to_string().cyan().bold()
            );
            println!();
            println!("{}", "Configure o Firefox:".bold());
            println!("  Preferências → Geral → Configurações de Rede…");
            println!("  → Configuração manual do proxy");
            println!("  → Proxy HTTP: 127.0.0.1   Porta: {}", port);
            println!("  → Marque \"Usar este proxy também para HTTPS\"");
            println!();
            println!("{}", "CTRL+C para parar.".dimmed());
            println!();

            server.run().await?;
        }
    }
    Ok(())
}

async fn run_terminal_interceptor(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<InterceptJob>,
) {
    while let Some(job) = rx.recv().await {
        print_intercept(&job);
        let outcome = prompt_intercept(&job.request).await;
        let _ = job.respond.send(outcome);
    }
}

fn print_intercept(job: &InterceptJob) {
    println!();
    println!(
        "{} {}:{} ({} bytes)",
        "┌─ INTERCEPT ─".cyan().bold(),
        job.host,
        job.port,
        job.request.len()
    );
    let text = String::from_utf8_lossy(&job.request);
    let preview = if text.len() > 2048 { &text[..2048] } else { &text };
    println!("{}", preview.trim_end());
    if text.len() > 2048 {
        println!("{}", "… (truncado)".dimmed());
    }
    println!("{}", "└─────────────".cyan().bold());
}

async fn prompt_intercept(original: &[u8]) -> InterceptOutcome {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    loop {
        print!(
            "{} ",
            "[f]orward  [e]ditar  [d]rop  [?]ajuda >".yellow().bold()
        );
        let _ = std::io::Write::flush(&mut std::io::stdout());

        let mut line = String::new();
        let n = reader.read_line(&mut line).await.unwrap_or(0);
        if n == 0 {
            return InterceptOutcome::Forward(None);
        }
        match line.trim() {
            "f" | "F" | "" => return InterceptOutcome::Forward(None),
            "d" | "D" => return InterceptOutcome::Drop,
            "e" | "E" => match edit_request(original) {
                Some(edited) => return InterceptOutcome::Forward(Some(edited)),
                None => {
                    eprintln!(
                        "{} editor falhou ou não encontrado — mantendo original",
                        "aviso:".yellow()
                    );
                    return InterceptOutcome::Forward(None);
                }
            },
            "?" => {
                println!("  f  encaminha a request como está");
                println!("  e  abre a request no editor ($EDITOR ou notepad) e reenvia o resultado");
                println!("  d  descarta a request (o cliente vê a conexão fechada)");
            }
            _ => eprintln!("{} comando desconhecido", "erro:".red()),
        }
    }
}

fn edit_request(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::process::Command;

    let mut path = std::env::temp_dir();
    path.push(format!(
        "rail_httpx_intercept_{}_{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));

    std::fs::write(&path, bytes).ok()?;

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad.exe".to_string()
            } else {
                "vi".to_string()
            }
        });

    let status = Command::new(&editor).arg(&path).status().ok()?;
    let result = if status.success() {
        std::fs::read(&path).ok()
    } else {
        None
    };
    let _ = std::fs::remove_file(&path);
    result
}

fn parse_method(s: &str) -> Method {
    match s.to_uppercase().as_str() {
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "PATCH" => Method::Patch,
        "DELETE" => Method::Delete,
        "HEAD" => Method::Head,
        "OPTIONS" => Method::Options,
        _ => Method::Get,
    }
}

fn print_response(resp: &rail_httpx_core::http::HttpResponse) {
    let status_line = format!("HTTP {} ({}ms)", resp.status, resp.duration_ms);
    if resp.status < 300 {
        println!("{}", status_line.green().bold());
    } else if resp.status < 400 {
        println!("{}", status_line.yellow().bold());
    } else {
        println!("{}", status_line.red().bold());
    }
    for (k, v) in &resp.headers {
        println!("{}: {}", k.cyan(), v);
    }
    println!();
    println!("{}", resp.body_text());
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n - 1]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_method_normalizes_case() {
        assert_eq!(parse_method("post"), Method::Post);
        assert_eq!(parse_method("DELETE"), Method::Delete);
        assert_eq!(parse_method("weird"), Method::Get);
    }

    #[test]
    fn parse_kv_splits_on_first_colon() {
        let (k, v) = parse_kv("Authorization: Bearer a:b").unwrap();
        assert_eq!(k, "Authorization");
        assert_eq!(v, "Bearer a:b");
    }

    #[test]
    fn parse_kv_rejects_missing_colon() {
        assert!(parse_kv("sem-virgula").is_err());
    }

    #[test]
    fn truncate_respects_limit() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("ab", 4), "ab");
    }

    #[test]
    fn limits_args_to_limits_is_identity() {
        let args = LimitsArgs {
            max_payloads: 100,
            max_payload_bytes: 200,
            max_response_bytes: 300,
            max_concurrency: 4,
            max_duration_secs: 60,
        };
        let l: Limits = (&args).into();
        assert_eq!(l.max_payloads, 100);
        assert_eq!(l.max_payload_bytes, 200);
        assert_eq!(l.max_response_bytes, 300);
        assert_eq!(l.max_concurrency, 4);
        assert_eq!(l.max_duration_secs, 60);
    }
}