fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&match std::env::args().nth(1).as_deref() {
            Some("oauth") => cms_web::contratos_oauth::exportar(),
            Some("mcp") => cms_web::contratos_mcp::exportar(),
            _ => cms_web::contratos::exportar(),
        })
        .expect("contrato JSON")
    );
}
