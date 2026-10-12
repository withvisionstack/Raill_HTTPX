//! # RAIL_HTTPX Core
//!
//! Postman leve + Burp core, em Rust.
//! Todo o código segue TDD: os testes de cada módulo estão em `#[cfg(test)]`
//! e também em `tests/` para integração.

pub mod db;
pub mod environment;
pub mod error;
pub mod http;
pub mod limits;
pub mod security;
pub mod webhook;          

pub use error::{Error, Result};
pub use environment::Environment;
pub use limits::Limits;