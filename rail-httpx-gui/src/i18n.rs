//! Internacionalização leve — PT-BR (padrão), EN, ES.
//!
//! Não usa crate externa: as strings são `&'static str` organizadas
//! em `Strings`. Trocar de idioma é trocar qual `Strings` está ativo.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    PtBr,
    En,
    Es,
}

impl Default for Language {
    fn default() -> Self { Language::PtBr }
}

impl Language {
    /// Código persistido no config.toml.
    pub fn code(&self) -> &'static str {
        match self {
            Language::PtBr => "ptbr",
            Language::En => "en",
            Language::Es => "es",
        }
    }

    /// Rótulo que aparece no menu de escolha.
    pub fn label(&self) -> &'static str {
        match self {
            Language::PtBr => "Português (BR)",
            Language::En => "English",
            Language::Es => "Español",
        }
    }

    pub fn from_code(s: &str) -> Self {
        match s {
            "en" => Language::En,
            "es" => Language::Es,
            _ => Language::PtBr,
        }
    }

    pub fn all() -> [Language; 3] {
        [Language::PtBr, Language::En, Language::Es]
    }

    pub fn strings(&self) -> &'static Strings {
        match self {
            Language::PtBr => &PT_BR,
            Language::En => &EN,
            Language::Es => &ES,
        }
    }
}

pub struct Strings {
    // -------- Tabs --------
    pub tab_send: &'static str,
    pub tab_proxy: &'static str,
    pub tab_about: &'static str,

    // -------- Send --------

    pub url_placeholder: &'static str,
    pub send_button: &'static str,
    pub insecure_tls: &'static str,
    pub headers_label: &'static str,
    pub body_label: &'static str,
    pub response_label: &'static str,
    pub no_response_yet: &'static str,

    // -------- Proxy --------
    pub proxy_port: &'static str,
    pub proxy_start: &'static str,
    pub proxy_stop: &'static str,
    pub proxy_running_at: &'static str,
    pub proxy_export_ca: &'static str,
    pub proxy_export_button: &'static str,
    pub proxy_captured: &'static str,
    pub proxy_col_time: &'static str,
    pub proxy_col_method: &'static str,
    pub proxy_col_host: &'static str,
    pub proxy_col_status: &'static str,
    pub proxy_col_req_b: &'static str,
    pub proxy_col_resp_b: &'static str,
    pub proxy_select_row: &'static str,
    pub proxy_request_header: &'static str,
    pub proxy_response_header: &'static str,
    pub proxy_dropped: &'static str,
    pub proxy_forwarded: &'static str,

    // -------- About --------
    pub about_subtitle: &'static str,
    pub about_tabs_header: &'static str,
    pub about_send_desc: &'static str,
    pub about_proxy_desc: &'static str,
    pub about_planned: &'static str,

    // -------- Settings / status --------


    pub theme_dark: &'static str,
    pub theme_light: &'static str,
    pub status_ready: &'static str,
    pub status_sending: &'static str,
    pub status_last_response: &'static str,
    pub status_error: &'static str,
    pub status_proxy: &'static str,


    // -------- Webhook --------
    pub tab_webhook: &'static str,
    pub webhook_port: &'static str,
    pub webhook_endpoint: &'static str,
    pub webhook_copy: &'static str,
    pub webhook_copied: &'static str,
    pub webhook_start: &'static str,
    pub webhook_stop: &'static str,
    pub webhook_running_at: &'static str,
    pub webhook_captured: &'static str,
    pub webhook_col_time: &'static str,
    pub webhook_col_method: &'static str,
    pub webhook_col_path: &'static str,
    pub webhook_col_remote: &'static str,
    pub webhook_col_body_b: &'static str,
    pub webhook_select_row: &'static str,
    pub webhook_headers_header: &'static str,
    pub webhook_body_header: &'static str,


    // -------- Scanner --------
    pub tab_scanner: &'static str,
    pub scanner_url_label: &'static str,
    pub scanner_params_label: &'static str,
    pub scanner_params_hint: &'static str,
    pub scanner_max_payloads: &'static str,
    pub scanner_insecure: &'static str,
    pub scanner_run: &'static str,
    pub scanner_running: &'static str,
    pub scanner_findings: &'static str,
    pub scanner_no_findings: &'static str,
    pub scanner_col_severity: &'static str,
    pub scanner_col_category: &'static str,
    pub scanner_col_param: &'static str,
    pub scanner_col_payload: &'static str,
    pub scanner_col_status: &'static str,
    pub scanner_requires_query: &'static str,


    // -------- Fuzzer --------
    pub tab_fuzzer: &'static str,
    pub fuzzer_url_label: &'static str,
    pub fuzzer_marker_hint: &'static str,
    pub fuzzer_concurrency: &'static str,
    pub fuzzer_insecure: &'static str,
    pub fuzzer_wordlist_label: &'static str,
    pub fuzzer_wordlist_file: &'static str,
    pub fuzzer_load_file: &'static str,
    pub fuzzer_run: &'static str,
    pub fuzzer_running: &'static str,
    pub fuzzer_results: &'static str,
    pub fuzzer_col_payload: &'static str,
    pub fuzzer_col_status: &'static str,
    pub fuzzer_col_length: &'static str,
    pub fuzzer_col_time: &'static str,
    pub fuzzer_no_results: &'static str,
    pub fuzzer_requires_marker: &'static str,
    pub fuzzer_requires_payloads: &'static str,


    // -------- DB --------
    pub tab_db: &'static str,
    pub db_url_label: &'static str,
    pub db_url_hint: &'static str,
    pub db_connect: &'static str,
    pub db_disconnect: &'static str,
    pub db_connected: &'static str,
    pub db_sql_label: &'static str,
    pub db_run: &'static str,
    pub db_running: &'static str,
    pub db_empty: &'static str,
    pub db_rows_info: &'static str,
    pub db_affected_info: &'static str,
    pub db_requires_connection: &'static str,
    pub db_connecting: &'static str,
    pub db_disconnected: &'static str,
}




pub static PT_BR: Strings = Strings {
    tab_send: "Enviar",
    tab_proxy: "Proxy",
    tab_about: "Sobre",

    tab_webhook: "Webhook",
    webhook_port: "Porta:",
    webhook_endpoint: "Endpoint:",
    webhook_copy: "Copiar",
    webhook_copied: "✓ copiado",
    webhook_start: "▶  Iniciar",
    webhook_stop: "■  Parar",
    webhook_running_at: "● escutando em",
    webhook_captured: "Recebidos:",
    webhook_col_time: "Hora",
    webhook_col_method: "Método",
    webhook_col_path: "Path",
    webhook_col_remote: "Origem",
    webhook_col_body_b: "Body B",
    webhook_select_row: "(selecione um request acima)",
    webhook_headers_header: "Cabeçalhos",
    webhook_body_header: "Corpo",

    tab_fuzzer: "Fuzzer",
    fuzzer_url_label: "URL (use FUZZ como marcador):",
    fuzzer_marker_hint: "ex.: http://alvo.local/FUZZ ou /api?id=FUZZ",
    fuzzer_concurrency: "Concorrência:",
    fuzzer_insecure: "TLS inseguro",
    fuzzer_wordlist_label: "Wordlist (um payload por linha):",
    fuzzer_wordlist_file: "Ou arquivo:",
    fuzzer_load_file: "Carregar",
    fuzzer_run: "▶  Executar",
    fuzzer_running: "executando…",
    fuzzer_results: "Resultados:",
    fuzzer_col_payload: "Payload",
    fuzzer_col_status: "Status",
    fuzzer_col_length: "Tamanho",
    fuzzer_col_time: "Tempo",
    fuzzer_no_results: "(nenhum resultado ainda)",
    fuzzer_requires_marker: "a URL precisa conter FUZZ",
    fuzzer_requires_payloads: "wordlist vazia",

    tab_db: "DB",
    db_url_label: "Conexão:",
    db_url_hint: "sqlite://arquivo.db ou postgres://user:pass@host/db",
    db_connect: "Conectar",
    db_disconnect: "Desconectar",
    db_connected: "● conectado",
    db_sql_label: "SQL:",
    db_run: "▶  Executar",
    db_running: "executando…",
    db_empty: "(execute uma query para ver o resultado)",
    db_rows_info: "linhas em",
    db_affected_info: "linhas afetadas em",
    db_requires_connection: "conecte primeiro",
    db_connecting: "conectando…",
    db_disconnected: "desconectado",

    tab_scanner: "Scanner",
    scanner_url_label: "URL (com query params):",
    scanner_params_label: "Params a escanear (vírgula; vazio = todos):",
    scanner_params_hint: "id,user,cat",
    scanner_max_payloads: "Payloads por categoria:",
    scanner_insecure: "TLS inseguro",
    scanner_run: "▶  Escanear",
    scanner_running: "escaneando…",
    scanner_findings: "Achados:",
    scanner_no_findings: "(nenhum achado ainda)",
    scanner_col_severity: "Severidade",
    scanner_col_category: "Categoria",
    scanner_col_param: "Param",
    scanner_col_payload: "Payload",
    scanner_col_status: "Status",
    scanner_requires_query: "a URL precisa ter ao menos um query param",


    url_placeholder: "https://…",
    send_button: "▶  Enviar",
    insecure_tls: "TLS inseguro (ignora certificado do servidor)",
    headers_label: "Cabeçalhos (um por linha, formato `Nome: valor`):",
    body_label: "Corpo:",
    response_label: "Resposta",
    no_response_yet: "(sem resposta ainda)",

    proxy_port: "Porta:",
    proxy_start: "▶  Iniciar",
    proxy_stop: "■  Parar",
    proxy_running_at: "● rodando em",
    proxy_export_ca: "Exportar CA:",
    proxy_export_button: "Exportar",
    proxy_captured: "Requisições capturadas:",
    proxy_col_time: "Hora",
    proxy_col_method: "Método",
    proxy_col_host: "Host",
    proxy_col_status: "Status",
    proxy_col_req_b: "Req B",
    proxy_col_resp_b: "Resp B",
    proxy_select_row: "(selecione uma requisição acima)",
    proxy_request_header: "Requisição",
    proxy_response_header: "Resposta",
    proxy_dropped: "DESCARTADA",
    proxy_forwarded: "encaminhada",

    about_subtitle: "Postman leve + Burp core, em Rust.",
    about_tabs_header: "Abas:",
    about_send_desc: "cliente HTTP",
    about_proxy_desc: "MITM com CA local e log em tempo real",
    about_planned: "planejado",


    theme_dark: "Escuro",
    theme_light: "Claro",
    status_ready: "pronto.",
    status_sending: "enviando…",
    status_last_response: "última resposta:",
    status_error: "erro:",
    status_proxy: "proxy",
};

pub static EN: Strings = Strings {
    tab_send: "Send",
    tab_proxy: "Proxy",
    tab_about: "About",

    tab_webhook: "Webhook",
    webhook_port: "Port:",
    webhook_endpoint: "Endpoint:",
    webhook_copy: "Copy",
    webhook_copied: "✓ copied",
    webhook_start: "▶  Start",
    webhook_stop: "■  Stop",
    webhook_running_at: "● listening at",
    webhook_captured: "Received:",
    webhook_col_time: "Time",
    webhook_col_method: "Method",
    webhook_col_path: "Path",
    webhook_col_remote: "Remote",
    webhook_col_body_b: "Body B",
    webhook_select_row: "(select a request above)",
    webhook_headers_header: "Headers",
    webhook_body_header: "Body",

    tab_fuzzer: "Fuzzer",
    fuzzer_url_label: "URL (use FUZZ as marker):",
    fuzzer_marker_hint: "e.g. http://target.local/FUZZ or /api?id=FUZZ",
    fuzzer_concurrency: "Concurrency:",
    fuzzer_insecure: "insecure TLS",
    fuzzer_wordlist_label: "Wordlist (one payload per line):",
    fuzzer_wordlist_file: "Or file:",
    fuzzer_load_file: "Load",
    fuzzer_run: "▶  Run",
    fuzzer_running: "running…",
    fuzzer_results: "Results:",
    fuzzer_col_payload: "Payload",
    fuzzer_col_status: "Status",
    fuzzer_col_length: "Length",
    fuzzer_col_time: "Time",
    fuzzer_no_results: "(no results yet)",
    fuzzer_requires_marker: "URL must contain FUZZ",
    fuzzer_requires_payloads: "wordlist empty",

    tab_scanner: "Scanner",
    scanner_url_label: "URL (with query params):",
    scanner_params_label: "Params to scan (comma; empty = all):",
    scanner_params_hint: "id,user,cat",
    scanner_max_payloads: "Payloads per category:",
    scanner_insecure: "insecure TLS",
    scanner_run: "▶  Scan",
    scanner_running: "scanning…",
    scanner_findings: "Findings:",
    scanner_no_findings: "(no findings yet)",
    scanner_col_severity: "Severity",
    scanner_col_category: "Category",
    scanner_col_param: "Param",
    scanner_col_payload: "Payload",
    scanner_col_status: "Status",
    scanner_requires_query: "URL must have at least one query param",

    tab_db: "DB",
    db_url_label: "Connection:",
    db_url_hint: "sqlite://file.db or postgres://user:pass@host/db",
    db_connect: "Connect",
    db_disconnect: "Disconnect",
    db_connected: "● connected",
    db_sql_label: "SQL:",
    db_run: "▶  Run",
    db_running: "running…",
    db_empty: "(run a query to see results)",
    db_rows_info: "rows in",
    db_affected_info: "rows affected in",
    db_requires_connection: "connect first",
    db_connecting: "connecting…",
    db_disconnected: "disconnected",


    url_placeholder: "https://…",
    send_button: "▶  Send",
    insecure_tls: "insecure TLS (ignore server certificate)",
    headers_label: "Headers (one per line, `Name: value`):",
    body_label: "Body:",
    response_label: "Response",
    no_response_yet: "(no response yet)",

    proxy_port: "Port:",
    proxy_start: "▶  Start",
    proxy_stop: "■  Stop",
    proxy_running_at: "● running at",
    proxy_export_ca: "Export CA:",
    proxy_export_button: "Export",
    proxy_captured: "Captured requests:",
    proxy_col_time: "Time",
    proxy_col_method: "Method",
    proxy_col_host: "Host",
    proxy_col_status: "Status",
    proxy_col_req_b: "Req B",
    proxy_col_resp_b: "Resp B",
    proxy_select_row: "(select a request above)",
    proxy_request_header: "Request",
    proxy_response_header: "Response",
    proxy_dropped: "DROPPED",
    proxy_forwarded: "forwarded",

    about_subtitle: "Lightweight Postman + Burp core, in Rust.",
    about_tabs_header: "Tabs:",
    about_send_desc: "HTTP client",
    about_proxy_desc: "MITM with local CA and live log",
    about_planned: "planned",


    theme_dark: "Dark",
    theme_light: "Light",
    status_ready: "ready.",
    status_sending: "sending…",
    status_last_response: "last response:",
    status_error: "error:",
    status_proxy: "proxy",
};

pub static ES: Strings = Strings {
    tab_send: "Enviar",
    tab_proxy: "Proxy",
    tab_about: "Acerca de",

    tab_webhook: "Webhook",
    webhook_port: "Puerto:",
    webhook_endpoint: "Endpoint:",
    webhook_copy: "Copiar",
    webhook_copied: "✓ copiado",
    webhook_start: "▶  Iniciar",
    webhook_stop: "■  Detener",
    webhook_running_at: "● escuchando en",
    webhook_captured: "Recibidos:",
    webhook_col_time: "Hora",
    webhook_col_method: "Método",
    webhook_col_path: "Ruta",
    webhook_col_remote: "Origen",
    webhook_col_body_b: "Body B",
    webhook_select_row: "(selecciona una solicitud arriba)",
    webhook_headers_header: "Cabeceras",
    webhook_body_header: "Cuerpo",

    tab_fuzzer: "Fuzzer",
    fuzzer_url_label: "URL (usa FUZZ como marcador):",
    fuzzer_marker_hint: "ej.: http://objetivo.local/FUZZ o /api?id=FUZZ",
    fuzzer_concurrency: "Concurrencia:",
    fuzzer_insecure: "TLS inseguro",
    fuzzer_wordlist_label: "Wordlist (un payload por línea):",
    fuzzer_wordlist_file: "O archivo:",
    fuzzer_load_file: "Cargar",
    fuzzer_run: "▶  Ejecutar",
    fuzzer_running: "ejecutando…",
    fuzzer_results: "Resultados:",
    fuzzer_col_payload: "Payload",
    fuzzer_col_status: "Estado",
    fuzzer_col_length: "Tamaño",
    fuzzer_col_time: "Tiempo",
    fuzzer_no_results: "(sin resultados todavía)",
    fuzzer_requires_marker: "la URL debe contener FUZZ",
    fuzzer_requires_payloads: "wordlist vacía",

    tab_scanner: "Scanner",
    scanner_url_label: "URL (con query params):",
    scanner_params_label: "Params a escanear (coma; vacío = todos):",
    scanner_params_hint: "id,user,cat",
    scanner_max_payloads: "Payloads por categoría:",
    scanner_insecure: "TLS inseguro",
    scanner_run: "▶  Escanear",
    scanner_running: "escaneando…",
    scanner_findings: "Hallazgos:",
    scanner_no_findings: "(sin hallazgos todavía)",
    scanner_col_severity: "Severidad",
    scanner_col_category: "Categoría",
    scanner_col_param: "Param",
    scanner_col_payload: "Payload",
    scanner_col_status: "Estado",
    scanner_requires_query: "la URL debe tener al menos un query param",

    tab_db: "DB",
    db_url_label: "Conexión:",
    db_url_hint: "sqlite://archivo.db o postgres://user:pass@host/db",
    db_connect: "Conectar",
    db_disconnect: "Desconectar",
    db_connected: "● conectado",
    db_sql_label: "SQL:",
    db_run: "▶  Ejecutar",
    db_running: "ejecutando…",
    db_empty: "(ejecuta una consulta para ver resultados)",
    db_rows_info: "filas en",
    db_affected_info: "filas afectadas en",
    db_requires_connection: "conectar primero",
    db_connecting: "conectando…",
    db_disconnected: "desconectado",


    url_placeholder: "https://…",
    send_button: "▶  Enviar",
    insecure_tls: "TLS inseguro (ignorar certificado del servidor)",
    headers_label: "Cabeceras (una por línea, `Nombre: valor`):",
    body_label: "Cuerpo:",
    response_label: "Respuesta",
    no_response_yet: "(sin respuesta aún)",

    proxy_port: "Puerto:",
    proxy_start: "▶  Iniciar",
    proxy_stop: "■  Detener",
    proxy_running_at: "● corriendo en",
    proxy_export_ca: "Exportar CA:",
    proxy_export_button: "Exportar",
    proxy_captured: "Solicitudes capturadas:",
    proxy_col_time: "Hora",
    proxy_col_method: "Método",
    proxy_col_host: "Host",
    proxy_col_status: "Estado",
    proxy_col_req_b: "Req B",
    proxy_col_resp_b: "Resp B",
    proxy_select_row: "(selecciona una solicitud arriba)",
    proxy_request_header: "Solicitud",
    proxy_response_header: "Respuesta",
    proxy_dropped: "DESCARTADA",
    proxy_forwarded: "reenviada",

    about_subtitle: "Postman ligero + Burp core, en Rust.",
    about_tabs_header: "Pestañas:",
    about_send_desc: "cliente HTTP",
    about_proxy_desc: "MITM con CA local y log en tiempo real",
    about_planned: "planeado",


    theme_dark: "Oscuro",
    theme_light: "Claro",
    status_ready: "listo.",
    status_sending: "enviando…",
    status_last_response: "última respuesta:",
    status_error: "error:",
    status_proxy: "proxy",
};