use crate::{Error, Result};

/// Limites operacionais da ferramenta.
/// Protegem o PC do operador **e** o alvo de acidentes.
#[derive(Debug, Clone)]
pub struct Limits {
    pub max_payloads: usize,
    pub max_payload_bytes: usize,
    pub max_response_bytes: usize,
    pub max_concurrency: usize,
    pub max_duration_secs: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_payloads: 50_000,
            max_payload_bytes: 64 * 1024,       // 64 KiB
            max_response_bytes: 16 * 1024 * 1024, // 16 MiB
            max_concurrency: 128,
            max_duration_secs: 3600,
        }
    }
}

impl Limits {
    pub fn check_payload(&self, p: &str) -> Result<()> {
        if p.len() > self.max_payload_bytes {
            return Err(Error::Limit(format!(
                "payload excede {} bytes (recebido {})",
                self.max_payload_bytes,
                p.len()
            )));
        }
        Ok(())
    }

    pub fn check_payload_count(&self, n: usize) -> Result<()> {
        if n > self.max_payloads {
            return Err(Error::Limit(format!(
                "{} payloads > limite de {}",
                n, self.max_payloads
            )));
        }
        Ok(())
    }

    pub fn check_concurrency(&self, c: usize) -> Result<()> {
        if c == 0 || c > self.max_concurrency {
            return Err(Error::Limit(format!(
                "concorrência {} fora do intervalo 1..={}",
                c, self.max_concurrency
            )));
        }
        Ok(())
    }

    pub fn check_duration_secs(&self, s: u64) -> Result<()> {
        if s == 0 || s > self.max_duration_secs {
            return Err(Error::Limit(format!(
                "duração {}s fora do intervalo 1..={}",
                s, self.max_duration_secs
            )));
        }
        Ok(())
    }

    /// Corta o corpo da resposta para `max_response_bytes`.
    /// Pura — a truncagem em si acontece no cliente HTTP.
    pub fn truncate_len(&self, body_len: usize) -> usize {
        body_len.min(self.max_response_bytes)
    }
}

// --------------------------- UNIT TESTS ---------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let l = Limits::default();
        assert!(l.max_payloads > 0);
        assert!(l.max_payload_bytes >= 1024);
        assert!(l.max_response_bytes >= 1024 * 1024);
        assert!((1..=256).contains(&l.max_concurrency));
        assert!(l.max_duration_secs > 0);
    }

    // ---- payload size -------------------------------------------

    #[test]
    fn accepts_payload_at_limit() {
        let l = Limits { max_payload_bytes: 8, ..Default::default() };
        assert!(l.check_payload("12345678").is_ok());
    }

    #[test]
    fn rejects_payload_above_limit() {
        let l = Limits { max_payload_bytes: 8, ..Default::default() };
        let err = l.check_payload("123456789").unwrap_err();
        assert!(matches!(err, Error::Limit(_)));
        assert!(err.to_string().contains("excede"));
    }

    // ---- payload count ------------------------------------------

    #[test]
    fn accepts_count_at_limit() {
        let l = Limits { max_payloads: 3, ..Default::default() };
        assert!(l.check_payload_count(3).is_ok());
    }

    #[test]
    fn rejects_count_above_limit() {
        let l = Limits { max_payloads: 3, ..Default::default() };
        assert!(l.check_payload_count(4).is_err());
    }

    // ---- concurrency --------------------------------------------

    #[test]
    fn rejects_zero_concurrency() {
        let l = Limits::default();
        assert!(l.check_concurrency(0).is_err());
    }

    #[test]
    fn rejects_concurrency_above_cap() {
        let l = Limits { max_concurrency: 8, ..Default::default() };
        assert!(l.check_concurrency(9).is_err());
        assert!(l.check_concurrency(8).is_ok());
    }

    // ---- duration -----------------------------------------------

    #[test]
    fn rejects_zero_duration() {
        let l = Limits::default();
        assert!(l.check_duration_secs(0).is_err());
    }

    #[test]
    fn rejects_duration_above_cap() {
        let l = Limits { max_duration_secs: 60, ..Default::default() };
        assert!(l.check_duration_secs(61).is_err());
        assert!(l.check_duration_secs(60).is_ok());
    }

    // ---- truncate_len (pura) ------------------------------------

    #[test]
    fn truncate_len_caps_at_limit() {
        let l = Limits { max_response_bytes: 10, ..Default::default() };
        assert_eq!(l.truncate_len(5), 5);
        assert_eq!(l.truncate_len(10), 10);
        assert_eq!(l.truncate_len(11), 10);
        assert_eq!(l.truncate_len(1_000_000), 10);
    }
}