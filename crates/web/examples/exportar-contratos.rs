fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&if std::env::args().nth(1).as_deref() == Some("oauth") {
            cms_web::contratos_oauth::exportar()
        } else {
            cms_web::contratos::exportar()
        }).expect("contrato JSON")
    );
}
