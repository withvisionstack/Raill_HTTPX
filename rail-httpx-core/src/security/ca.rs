//! Autoridade Certificadora local — o coração "Burp-style" do proxy MITM.
//!
//! Gera um CA raiz (auto-assinado) e emite certificados-folha assinados
//! por ele, um por domínio interceptado. O usuário instala o `ca.pem`
//! uma vez no sistema/navegador, e a partir daí o proxy pode assinar
//! qualquer domínio sem aviso de "certificado inválido".

use crate::{Error, Result};
use rcgen::{BasicConstraints, Certificate, CertificateParams, DnType, IsCa, KeyPair};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Par cert+chave em PEM, pronto para virar `ServerConfig` do rustls.
#[derive(Debug, Clone)]
pub struct CertPair {
    pub cert_pem: String,
    pub key_pem: String,
}

/// Autoridade Certificadora local.
///
/// Thread-safe: pode ser compartilhada via `Arc` entre conexões.
pub struct CertificateAuthority {
    ca_cert: Certificate,
    ca_key: KeyPair,
    ca_cert_pem: String,
    ca_key_pem: String,
    ca_dir: PathBuf,
    /// Cache de certificados-folha por domínio — evita regerar a cada conexão.
    leaf_cache: Mutex<HashMap<String, Arc<CertPair>>>,
}

impl CertificateAuthority {
    /// Carrega o CA do diretório, ou gera um novo se não existir.
    ///
    /// Arquivos criados: `ca.pem` (certificado público) e `ca.key` (chave privada).
    /// O `ca.pem` é o que você instala no Firefox/Chrome/Windows.
    pub fn new_or_load(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let cert_path = dir.join("ca.pem");
        let key_path = dir.join("ca.key");

        let (ca_cert, ca_key, ca_cert_pem, ca_key_pem) =
            if cert_path.exists() && key_path.exists() {
                let cert_pem = std::fs::read_to_string(&cert_path)?;
                let key_pem = std::fs::read_to_string(&key_path)?;

                let key = KeyPair::from_pem(&key_pem)
                    .map_err(|e| Error::Other(format!("CA key PEM inválido: {e}")))?;

                let params = CertificateParams::from_ca_cert_pem(&cert_pem)
                    .map_err(|e| Error::Other(format!("CA cert PEM inválido: {e}")))?;

                let cert = params
                    .self_signed(&key)
                    .map_err(|e| Error::Other(format!("reconstituir CA: {e}")))?;

                (cert, key, cert_pem, key_pem)
            } else {
                let (cert, key) = generate_ca()?;
                let cert_pem = cert.pem();
                let key_pem = key.serialize_pem();
                std::fs::write(&cert_path, &cert_pem)?;
                std::fs::write(&key_path, &key_pem)?;
                (cert, key, cert_pem, key_pem)
            };

        Ok(Self {
            ca_cert,
            ca_key,
            ca_cert_pem,
            ca_key_pem,
            ca_dir: dir,
            leaf_cache: Mutex::new(HashMap::new()),
        })
    }
    /// PEM do certificado do CA — o que o usuário instala no sistema.
    pub fn ca_cert_pem(&self) -> &str { &self.ca_cert_pem }

    /// PEM da chave privada do CA. **Nunca** exponha isso pra fora.
    pub fn ca_key_pem(&self) -> &str { &self.ca_key_pem }

    /// Diretório onde o CA vive no disco.
    pub fn ca_dir(&self) -> &Path { &self.ca_dir }

    /// Emite (ou recupera do cache) um certificado-folha para `domain`.
    ///
    /// O retorno é um `Arc<CertPair>` — o proxy MITM usa isso para montar
    /// o `ServerConfig` do rustls sem clonar a chave privada toda vez.
    pub fn leaf_for(&self, domain: &str) -> Result<Arc<CertPair>> {
        // Fast path — cache hit.
        if let Some(pair) = self.leaf_cache.lock().unwrap().get(domain).cloned() {
            return Ok(pair);
        }

        // Slow path — gera e guarda.
        let pair = Arc::new(generate_leaf(domain, &self.ca_cert, &self.ca_key)?);
        self.leaf_cache
            .lock()
            .unwrap()
            .insert(domain.to_string(), pair.clone());
        Ok(pair)
    }

    /// Quantidade de certificados-folha no cache (útil em testes/telemetria).
    pub fn cached_leaf_count(&self) -> usize {
        self.leaf_cache.lock().unwrap().len()
    }
}

fn generate_ca() -> Result<(Certificate, KeyPair)> {
    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| Error::Other(format!("params CA: {e}")))?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "RAIL_HTTPX Local CA");

    let key = KeyPair::generate().map_err(|e| Error::Other(format!("key CA: {e}")))?;
    let cert = params
        .self_signed(&key)
        .map_err(|e| Error::Other(format!("self-sign CA: {e}")))?;
    Ok((cert, key))
}

fn generate_leaf(domain: &str, ca_cert: &Certificate, ca_key: &KeyPair) -> Result<CertPair> {
    let mut params = CertificateParams::new(vec![domain.to_string()])
        .map_err(|e| Error::Other(format!("params leaf: {e}")))?;
    params
        .distinguished_name
        .push(DnType::CommonName, domain);

    let key = KeyPair::generate().map_err(|e| Error::Other(format!("key leaf: {e}")))?;
    let cert = params
        .signed_by(&key, ca_cert, ca_key)
        .map_err(|e| Error::Other(format!("sign leaf: {e}")))?;

    Ok(CertPair {
        cert_pem: cert.pem(),
        key_pem: key.serialize_pem(),
    })
}

/// Diretório padrão do CA: `~/.rail_httpx/ca/` (Unix)
/// ou `%USERPROFILE%\.rail_httpx\ca\` (Windows).
///
/// Não usa crate `dirs` pra manter as dependências enxutas.
pub fn default_ca_dir() -> PathBuf {
    let base = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".rail_httpx").join("ca")
}

// --------------------------- UNIT TESTS ---------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_ca_files_on_first_run() {
        let dir = tempdir().unwrap();
        let ca = CertificateAuthority::new_or_load(dir.path()).unwrap();

        assert!(dir.path().join("ca.pem").exists(), "ca.pem não foi criado");
        assert!(dir.path().join("ca.key").exists(), "ca.key não foi criado");
        assert!(ca.ca_cert_pem().contains("BEGIN CERTIFICATE"));
        assert!(ca.ca_key_pem().contains("BEGIN PRIVATE KEY"));
    }

    #[test]
    fn loads_same_ca_on_second_call() {
        let dir = tempdir().unwrap();
        let ca1 = CertificateAuthority::new_or_load(dir.path()).unwrap();
        let ca2 = CertificateAuthority::new_or_load(dir.path()).unwrap();

        assert_eq!(ca1.ca_cert_pem(), ca2.ca_cert_pem());
        assert_eq!(ca1.ca_key_pem(), ca2.ca_key_pem());
    }

    #[test]
    fn leaf_for_returns_valid_pem_pair() {
        let dir = tempdir().unwrap();
        let ca = CertificateAuthority::new_or_load(dir.path()).unwrap();
        let leaf = ca.leaf_for("example.com").unwrap();

        assert!(leaf.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(leaf.key_pem.contains("BEGIN PRIVATE KEY"));
        // Folha é diferente do CA — senão o navegador usaria o mesmo cert pra tudo.
        assert_ne!(leaf.cert_pem, ca.ca_cert_pem());
    }

    #[test]
    fn leaf_is_cached_per_domain() {
        let dir = tempdir().unwrap();
        let ca = CertificateAuthority::new_or_load(dir.path()).unwrap();

        let a = ca.leaf_for("example.com").unwrap();
        let b = ca.leaf_for("example.com").unwrap();

        // Arc::ptr_eq garante que é literalmente o mesmo objeto, não só conteúdo igual.
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(ca.cached_leaf_count(), 1);
    }

    #[test]
    fn different_domains_get_different_leafs() {
        let dir = tempdir().unwrap();
        let ca = CertificateAuthority::new_or_load(dir.path()).unwrap();

        let a = ca.leaf_for("a.example").unwrap();
        let b = ca.leaf_for("b.example").unwrap();

        assert!(!Arc::ptr_eq(&a, &b));
        assert_ne!(a.cert_pem, b.cert_pem);
        assert_ne!(a.key_pem, b.key_pem);
        assert_eq!(ca.cached_leaf_count(), 2);
    }

    #[test]
    fn ca_survives_reload_and_leaf_cache_starts_empty() {
        let dir = tempdir().unwrap();
        {
            let ca1 = CertificateAuthority::new_or_load(dir.path()).unwrap();
            let _ = ca1.leaf_for("example.com").unwrap();
            // ca1 é dropado aqui.
        }
        let ca2 = CertificateAuthority::new_or_load(dir.path()).unwrap();
        // CA continua o mesmo...
        assert!(ca2.ca_cert_pem().contains("BEGIN CERTIFICATE"));
        // ...mas o cache de folhas reiniciou (é in-memory por sessão).
        assert_eq!(ca2.cached_leaf_count(), 0);
    }

    #[test]
    fn default_ca_dir_ends_with_expected_suffix() {
        let d = default_ca_dir();
        assert!(d.ends_with(".rail_httpx/ca") || d.ends_with(".rail_httpx\\ca"));
    }
}