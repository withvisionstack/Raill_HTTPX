use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Variáveis interpoláveis em requests usando `{{nome}}` (igual Postman).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Environment {
    pub name: String,
    #[serde(default)]
    pub vars: HashMap<String, String>,
}

impl Environment {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), vars: HashMap::new() }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }

    /// Substitui todas as ocorrências de `{{chave}}`. Variáveis
    /// não resolvidas ficam intactas (útil para debugar).
    pub fn render(&self, input: &str) -> String {
        let mut out = input.to_string();
        // Ordena por tamanho desc para evitar que "ho" casque dentro de "host".
        let mut keys: Vec<&String> = self.vars.keys().collect();
        keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
        for k in keys {
            let needle = format!("{{{{{}}}}}", k);
            out = out.replace(&needle, &self.vars[k]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- RED → GREEN → REFACTOR (exemplo canônico) -------------------

    #[test]
    fn renders_single_var() {
        let mut env = Environment::new("dev");
        env.set("host", "api.exemplo.com");
        assert_eq!(env.render("https://{{host}}/"), "https://api.exemplo.com/");
    }

    #[test]
    fn renders_multiple_vars() {
        let mut env = Environment::new("dev");
        env.set("host", "api.exemplo.com");
        env.set("token", "abc");
        assert_eq!(
            env.render("https://{{host}}/u?t={{token}}"),
            "https://api.exemplo.com/u?t=abc"
        );
    }

    #[test]
    fn leaves_unknown_vars_intact() {
        let env = Environment::new("dev");
        assert_eq!(env.render("{{nao_existe}}"), "{{nao_existe}}");
    }

    #[test]
    fn does_not_render_partial_prefix_collision() {
        // "ho" não pode casar dentro de "{{host}}"
        let mut env = Environment::new("dev");
        env.set("ho", "X");
        env.set("host", "api");
        assert_eq!(env.render("{{host}}"), "api");
    }

    #[test]
    fn renders_repeated_occurrences() {
        let mut env = Environment::new("dev");
        env.set("id", "42");
        assert_eq!(env.render("/{{id}}/{{id}}"), "/42/42");
    }
}