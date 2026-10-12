//! Helpers compartilhados entre workspaces.

use std::collections::HashMap;

/// Parseia o texto do campo "Cabeçalhos" para um mapa.
///
/// Aceita:
///   - `Nome: valor`
///   - `"Nome": "valor"` (colado do DevTools)
///   - linhas vazias (ignoradas)
///   - qualquer coisa sem `:` (ignorada)
///
/// Remove aspas, tabs, `\r`, `\n` dos nomes e valores — tudo isso
/// causa `builder error` no reqwest quando vem de copy-paste.
pub fn parse_headers_text(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else { continue };
        let k = sanitize(k);
        let v = sanitize(v);
        if !k.is_empty() {
            out.insert(k, v);
        }
    }
    out
}

fn sanitize(s: &str) -> String {
    s.trim()
        .trim_matches('"')
        .chars()
        .filter(|c| *c != '\r' && *c != '\n' && *c != '\t')
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_headers() {
        let h = parse_headers_text("Content-Type: application/json\nAccept: */*");
        assert_eq!(h.get("Content-Type").unwrap(), "application/json");
        assert_eq!(h.get("Accept").unwrap(), "*/*");
    }

    #[test]
    fn tolerates_devtools_format() {
        let h = parse_headers_text(r#""Cookie": "PHPSESSID=abc; security=low""#);
        assert_eq!(h.get("Cookie").unwrap(), "PHPSESSID=abc; security=low");
    }

    #[test]
    fn strips_inner_quotes_on_value_edges() {
        let h = parse_headers_text(r#"Authorization: "Bearer abc""#);
        assert_eq!(h.get("Authorization").unwrap(), "Bearer abc");
    }

    #[test]
    fn ignores_lines_without_colon() {
        let h = parse_headers_text("sem dois pontos\nX-Test: ok");
        assert_eq!(h.len(), 1);
        assert_eq!(h.get("X-Test").unwrap(), "ok");
    }

    #[test]
    fn tolerates_empty_lines() {
        let h = parse_headers_text("\n\nX: 1\n\n");
        assert_eq!(h.len(), 1);
    }
}