# RAIL_HTTPX

> Postman leve + Burp core, em Rust. Cliente HTTP, proxy MITM, fuzzer, scanner
> de vulnerabilidades, servidor de webhook e cliente de banco — tudo num único
> binário nativo, sem Electron, sem conta, sem telemetria.

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)]()
[![Status](https://img.shields.io/badge/status-em%20desenvolvimento-yellow.svg)]()
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

> ⚠️ **Projeto em fase de progresso.** Já é executável e funcional (GUI +
> CLI), mas ainda em desenvolvimento ativo — features marcadas como `□` na
> roadmap ainda não existem, e a API/CLI pode mudar entre versões.

---

## 📸 Screenshots

<p align="center">
  <img src="screenshots/send.png" alt="Aba Send — cliente HTTP" width="47%">
  &nbsp;
  <img src="screenshots/about.png" alt="Aba About" width="47%">
</p>

---

## Por que RAIL_HTTPX?

As ferramentas que usamos no dia a dia estão ficando pesadas e fragmentadas:

- **Postman** tem 300+ MB, Electron, 3 processos, login obrigatório, telemetria.
- **Insomnia** tem histórico de mudar de modelo de licença, plugins pagos.
- **Burp Suite Community** faz interceptação, mas não tem fuzzer com paralelismo real, não tem DB, não tem cliente HTTP.

O RAIL_HTTPX resolve isso em **um binário único de ~20 MB**, multiplataforma,
em três idiomas (PT-BR, EN, ES), com tema claro/escuro — e é **open source de verdade** (MIT).

---

## Instalação

```bash
git clone https://github.com/withvisionstack/Raill_HTTPX.git
cd rail_httpx
cargo build --release
```

O binário final fica em `target/release/`.

---

## Recursos

### 🌐 Cliente HTTP (aba Send)
- Métodos GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS
- Headers em texto livre (aceita o formato colado do DevTools do Firefox/Chrome)
- Body em texto/JSON
- Variáveis de ambiente com sintaxe `{{var}}`
- Modo TLS inseguro (self-signed, igual Burp)
- Trunca respostas grandes pra proteger a RAM do operador

### 🕵️ Proxy MITM (aba Proxy)
- CA local gerado em runtime (`~/.rail_httpx/ca/`)
- Intercepta HTTPS com certificado forjado por domínio (com cache)
- Encaminha pra backend real com TLS verificado contra as CAs públicas (webpki-roots)
- Log em tempo real de toda request/response (headers, body, tempos)
- Export do CA público pra instalar no Firefox/navegador

### 🔨 Fuzzer (aba Fuzzer)
- Wordlist com marcador `FUZZ` na URL
- Paralelismo configurável
- Tabela com payload, status, tamanho, tempo
- Cores por status (2xx verde, 4xx azul, 5xx vermelho)
- Carrega wordlist de arquivo (testado com SecLists)

### 🛡️ Scanner (aba Scanner)
- Injeção automática nos query params selecionados
- Detecta:
  - SQL Injection (error-based + time-based)
  - Cross-Site Scripting (refletido)
  - Server-Side Template Injection
  - Path Traversal
  - Command Injection
- Severidade por categoria (Critical / High / Medium)
- Evidência extraída da resposta

### 🪝 Webhook receiver (aba Webhook)
- Servidor HTTP local que aceita qualquer método
- Loga payloads recebidos em tempo real
- Útil para debugar integrações com Stripe, GitHub, Mercado Pago, Twilio

### 🗄️ Cliente de banco (aba DB)
- **PostgreSQL**, **MySQL** e **SQLite** via `sqlx`
- Editor SQL
- Tabela de resultado com colunas dinâmicas
- Reporta linhas afetadas para mutações
- Heurística de detecção de SELECT vs mutação (comentários `--` e `/* */` suportados)

### 💻 CLI (`rail-httpx`)
Todos os recursos acima funcionam também via linha de comando — pra scripts e CI.

```bash
rail-httpx send https://httpbin.org/get
rail-httpx send https://httpbin.org/post -X POST -d '{"a":1}'
rail-httpx send http://ollama:11434/api/generate -X POST --data-file body.json

rail-httpx scan "http://alvo/item?id=1" -p id -H "Cookie: sessao=abc"

rail-httpx fuzz "http://alvo/FUZZ" -w words.txt -c 50

rail-httpx db --url sqlite:///./dados.db -q "SELECT * FROM users LIMIT 5"
```

---

## 🗺️ Roadmap

| Status | Feature                                    |
|:------:|---------------------------------------------|
| ✅     | Cliente HTTP com env vars                   |
| ✅     | Proxy MITM com CA próprio                   |
| ✅     | Fuzzer paralelo com wordlist                |
| ✅     | Scanner de vulnerabilidades comuns          |
| ✅     | Webhook receiver                            |
| ✅     | Cliente de banco (PG/MySQL/SQLite)          |
| ✅     | i18n PT-BR / EN / ES                        |
| ✅     | Tema claro / escuro                         |
| ⬜     | WebSocket client                            |
| ⬜     | gRPC client                                 |
| ⬜     | GraphQL helper                              |
| ⬜     | Coleções / import do Postman                |
| ⬜     | Histórico persistente                       |

---

## 🤝 Contribuindo

Projeto open source e de uso livre pela comunidade Rust. Issues, PRs e
sugestões são bem-vindos — especialmente nos itens ainda em aberto da
roadmap acima.

```bash
git clone https://github.com/withvisionstack/Raill_HTTPX.git
cd rail_httpx
cargo build
cargo test
```

## 📄 Licença

[MIT](LICENSE) — use, modifique e distribua livremente.
