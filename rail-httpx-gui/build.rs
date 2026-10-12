fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        // Reusa o mesmo PNG — o winres aceita png e converte.
        res.set_icon("assets/icon.png");
        res.set("ProductName", "RAIL_HTTPX");
        res.set("FileDescription", "Postman leve + Burp core, em Rust");
        res.set("LegalCopyright", "MIT License");
        if let Err(e) = res.compile() {
            eprintln!("aviso: não consegui embutir ícone no .exe: {e}");
        }
    }
}