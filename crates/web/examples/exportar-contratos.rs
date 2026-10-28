fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&cms_web::contratos::exportar()).expect("contrato JSON")
    );
}
