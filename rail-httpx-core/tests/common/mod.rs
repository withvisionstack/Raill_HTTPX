//! Helpers compartilhados entre os testes de integração.
use rail_httpx_core::environment::Environment;
use rail_httpx_core::http::HttpClient;

pub fn test_env() -> Environment {
    let mut env = Environment::new("test");
    env.set("token", "tok-test");
    env
}

pub fn test_client() -> HttpClient {
    HttpClient::new().expect("cliente HTTP deve inicializar")
}