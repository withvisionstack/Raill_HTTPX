use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("erro HTTP: {0}")]
    Http(#[from] reqwest::Error),

    #[error("erro de banco: {0}")]
    Database(String),

    #[error("erro de I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("erro de serialização: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("URL inválida: {0}")]
    Url(String),

    #[error("limite excedido: {0}")]
    Limit(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_url_error() {
        let e = Error::Url("sem esquema".into());
        assert_eq!(e.to_string(), "URL inválida: sem esquema");
    }

    #[test]
    fn displays_limit_error() {
        let e = Error::Limit("payload grande".into());
        assert_eq!(e.to_string(), "limite excedido: payload grande");
    }

    #[test]
    fn converts_from_io() {
        let io = std::io::Error::new(std::io::ErrorKind::Other, "boom");
        let e: Error = io.into();
        assert!(matches!(e, Error::Io(_)));
    }
}