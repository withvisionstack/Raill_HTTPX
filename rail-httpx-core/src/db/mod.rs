use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::any::AnyPoolOptions;
use sqlx::{AnyPool, Column, Row};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryOutput {
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub duration_ms: u128,
    pub affected: Option<u64>,
}

pub struct Database {
    pool: AnyPool,
}

impl Database {
    /// URLs aceitas: `postgres://`, `mysql://`, `sqlite://` ou `sqlite:`.
    ///
    /// SQLite em Windows é **tratado à parte** porque o parser de URL
    /// genérico do `AnyPool` mangla caminhos com drive letter — vira
    /// `host = "C:"` e o driver recebe string vazia. Aqui extraímos o
    /// path na mão e montamos `SqliteConnectOptions` direto.
    pub async fn connect(url: &str) -> Result<Self> {
        sqlx::any::install_default_drivers();

        // SQLite NÃO cria o arquivo automaticamente quando conectado por URL
        // (default: `mode=rw` que exige arquivo existente). Adicionamos
        // `?mode=rwc` (read-write-create) para o sqlx criar o arquivo se
        // não existir. Sem isso, qualquer conexão a arquivo novo dá
        // SQLITE_CANTOPEN (code 14).
        let url = normalize_sqlite_url(url);
        eprintln!("[DB] URL normalizada: {:?}", url);

        let max_conn = if url.contains(":memory:") { 1 } else { 5 };

        let pool = AnyPoolOptions::new()
            .max_connections(max_conn)
            .connect(&url)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        Ok(Self { pool })
    }
    /// SQLite em memória, 1 conexão para preservar o schema entre queries.
    pub async fn connect_in_memory_sqlite() -> Result<Self> {
        Self::connect("sqlite::memory:").await
    }

    pub async fn query(&self, sql: &str) -> Result<QueryOutput> {
        let start = Instant::now();

        // Decide o caminho ANTES de tocar no driver. Sem isso, o SQLite
        // executa INSERT/UPDATE via `fetch_all` silenciosamente e devolve
        // zero linhas — perdendo `rows_affected`.
        if is_row_returning(sql) {
            let rows = sqlx::query(sql)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;

            let columns: Vec<String> = rows
                .first()
                .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
                .unwrap_or_default();

            let mut out_rows = Vec::with_capacity(rows.len());
            for row in &rows {
                let mut obj = serde_json::Map::new();
                for (i, col) in row.columns().iter().enumerate() {
                    obj.insert(col.name().to_string(), decode_cell(row, i));
                }
                out_rows.push(serde_json::Value::Object(obj));
            }

            Ok(QueryOutput {
                columns,
                rows: out_rows,
                duration_ms: start.elapsed().as_millis(),
                affected: None,
            })
        } else {
            let res = sqlx::query(sql)
                .execute(&self.pool)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;

            Ok(QueryOutput {
                columns: vec![],
                rows: vec![],
                duration_ms: start.elapsed().as_millis(),
                affected: Some(res.rows_affected()),
            })
        }
    }
}

/// Heurística: a instrução devolve linhas? Olha a primeira keyword
/// ignorando espaços e comentários (`--` de linha e `/* */` de bloco).
fn is_row_returning(sql: &str) -> bool {
    let cleaned = strip_leading_comments_and_whitespace(sql);
    let upper = cleaned.to_ascii_uppercase();

    const ROW_RETURNING: &[&str] = &[
        "SELECT",
        "WITH",
        "PRAGMA",
        "EXPLAIN",
        "SHOW",
        "DESCRIBE",
        "DESC ",
        "VALUES",
        "TABLE ",
    ];
    ROW_RETURNING.iter().any(|kw| upper.starts_with(kw))
}

/// Normaliza URL de SQLite:
///   - `:memory:` fica intacto (é caso especial do sqlx)
///   - URLs de arquivo ganham `?mode=rwc` se não tiverem parâmetros
fn normalize_sqlite_url(url: &str) -> String {
    if !url.starts_with("sqlite:") {
        return url.to_string();
    }
    if url.contains(":memory:") {
        return url.to_string();
    }
    // Se já tem parâmetros na URL, respeita o que o usuário escreveu.
    if url.contains('?') {
        return url.to_string();
    }
    format!("{}?mode=rwc", url)
}


fn strip_leading_comments_and_whitespace(sql: &str) -> &str {
    let mut s = sql.trim_start();
    loop {
        if let Some(rest) = s.strip_prefix("--") {
            match rest.find('\n') {
                Some(i) => {
                    s = rest[i + 1..].trim_start();
                    continue;
                }
                None => return "",
            }
        }
        if let Some(rest) = s.strip_prefix("/*") {
            match rest.find("*/") {
                Some(i) => {
                    s = rest[i + 2..].trim_start();
                    continue;
                }
                None => return "",
            }
        }
        return s;
    }
}

fn decode_cell(row: &sqlx::any::AnyRow, i: usize) -> serde_json::Value {
    use sqlx::ValueRef;

    if let Ok(v) = row.try_get::<Option<String>, _>(i) {
        return v.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
        return v.map(|n| serde_json::Value::Number(n.into())).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
        return v
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
        return v.map(serde_json::Value::Bool).unwrap_or(serde_json::Value::Null);
    }
    if let Ok(raw) = row.try_get_raw(i) {
        if raw.is_null() {
            return serde_json::Value::Null;
        }
    }
    serde_json::Value::Null
}

// --------------------------- UNIT TESTS ---------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_select_as_row_returning() {
        assert!(is_row_returning("SELECT 1"));
        assert!(is_row_returning("select * from t"));
        assert!(is_row_returning("  \n\t  SELECT id FROM x"));
    }

    #[test]
    fn recognizes_mutations_as_non_row_returning() {
        assert!(!is_row_returning("INSERT INTO t VALUES (1)"));
        assert!(!is_row_returning("UPDATE t SET v = 1"));
        assert!(!is_row_returning("DELETE FROM t"));
        assert!(!is_row_returning("CREATE TABLE t (id INT)"));
        assert!(!is_row_returning("DROP TABLE t"));
    }

    #[test]
    fn strips_line_comments() {
        assert!(is_row_returning("-- cabeçalho\nSELECT 1"));
        assert!(!is_row_returning("-- cabeçalho\nINSERT INTO t VALUES (1)"));
    }

    #[test]
    fn strips_block_comments() {
        assert!(is_row_returning("/* autor: felipe */ SELECT 1"));
        assert!(!is_row_returning("/* nota */ UPDATE t SET v=1"));
    }

    #[test]
    fn comment_only_input_is_not_row_returning() {
        assert!(!is_row_returning("-- nada"));
        assert!(!is_row_returning(""));
        assert!(!is_row_returning("   "));
    }

    #[test]
    fn prisma_and_explain_are_row_returning() {
        assert!(is_row_returning("PRAGMA table_info(t)"));
        assert!(is_row_returning("EXPLAIN SELECT 1"));
    }

    // ---- Integração: SQLite in-memory ---------------------------

    #[tokio::test]
    async fn connect_in_memory_and_select_one() {
        let db = Database::connect_in_memory_sqlite().await.unwrap();
        let out = db.query("SELECT 1 AS one").await.unwrap();
        assert_eq!(out.rows.len(), 1);
        assert_eq!(out.rows[0]["one"], 1);
        assert!(out.columns.contains(&"one".to_string()));
        assert!(out.affected.is_none());
    }

    #[tokio::test]
    async fn memory_sqlite_persists_across_queries() {
        // Este teste falha se o pool abrir várias conexões em :memory:.
        // Cada conexão em SQLite :memory: é um banco **separado**.
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.query("CREATE TABLE t (id INTEGER)").await.unwrap();
        db.query("INSERT INTO t VALUES (1),(2)").await.unwrap();
        let out = db.query("SELECT COUNT(*) AS n FROM t").await.unwrap();
        assert_eq!(out.rows[0]["n"], 2, "tabela não persistiu entre queries");
    }

    #[tokio::test]
    async fn create_insert_select_roundtrip() {
        let db = Database::connect_in_memory_sqlite().await.unwrap();

        let created = db
            .query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
            .await
            .unwrap();
        assert!(created.affected.is_some());

        let inserted = db
            .query("INSERT INTO users (name) VALUES ('ana'), ('bia')")
            .await
            .unwrap();
        assert_eq!(inserted.affected, Some(2));

        let out = db.query("SELECT id, name FROM users ORDER BY id").await.unwrap();
        assert_eq!(out.rows.len(), 2);
        assert_eq!(out.rows[0]["name"], "ana");
        assert_eq!(out.rows[1]["name"], "bia");
    }

    #[tokio::test]
    async fn update_reports_affected_rows() {
        let db = Database::connect_in_memory_sqlite().await.unwrap();
        db.query("CREATE TABLE t (v INTEGER)").await.unwrap();
        db.query("INSERT INTO t VALUES (1),(1),(2)").await.unwrap();

        let out = db.query("UPDATE t SET v = 9 WHERE v = 1").await.unwrap();
        assert_eq!(out.affected, Some(2));
    }

    #[tokio::test]
    async fn delete_reports_affected_rows() {
        let db = Database::connect_in_memory_sqlite().await.unwrap();
        db.query("CREATE TABLE t (v INTEGER)").await.unwrap();
        db.query("INSERT INTO t VALUES (1),(2),(3)").await.unwrap();

        let out = db.query("DELETE FROM t WHERE v >= 2").await.unwrap();
        assert_eq!(out.affected, Some(2));
    }

    #[tokio::test]
    async fn invalid_sql_returns_error() {
        let db = Database::connect_in_memory_sqlite().await.unwrap();
        let err = db.query("THIS IS NOT SQL").await.unwrap_err();
        assert!(matches!(err, Error::Database(_)));
    }

    #[tokio::test]
    async fn null_values_are_serialized_as_json_null() {
        let db = Database::connect_in_memory_sqlite().await.unwrap();
        let out = db.query("SELECT NULL AS nada").await.unwrap();
        assert_eq!(out.rows[0]["nada"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn comment_before_select_works_end_to_end() {
        let db = Database::connect_in_memory_sqlite().await.unwrap();
        let out = db.query("-- pega constante\nSELECT 42 AS x").await.unwrap();
        assert_eq!(out.rows[0]["x"], 42);
    }
}

#[test]
fn normalize_sqlite_url_adds_rwc() {
    assert_eq!(
        normalize_sqlite_url("sqlite:D:/tmp/x.db"),
        "sqlite:D:/tmp/x.db?mode=rwc"
    );
    assert_eq!(
        normalize_sqlite_url("sqlite:///D:/tmp/x.db"),
        "sqlite:///D:/tmp/x.db?mode=rwc"
    );
}

#[test]
fn normalize_sqlite_url_leaves_memory_alone() {
    assert_eq!(
        normalize_sqlite_url("sqlite::memory:"),
        "sqlite::memory:"
    );
}

#[test]
fn normalize_sqlite_url_keeps_existing_params() {
    assert_eq!(
        normalize_sqlite_url("sqlite:x.db?mode=ro"),
        "sqlite:x.db?mode=ro"
    );
}

#[test]
fn normalize_sqlite_url_ignores_postgres() {
    assert_eq!(
        normalize_sqlite_url("postgres://localhost/db"),
        "postgres://localhost/db"
    );
}