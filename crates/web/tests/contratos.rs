#![cfg(feature = "contracts")]

use cms_web::contratos::exportar;

#[test]
fn exporta_as_operacoes_e_os_tipos_usados_pelo_modulo() {
    let contrato = exportar();
    let operacoes = contrato["operations"].as_array().expect("operações");
    assert_eq!(operacoes.len(), 16);
    let criacao = operacoes
        .iter()
        .find(|op| op["id"] == "createApp")
        .expect("criação");
    assert_eq!(criacao["request"]["additionalProperties"], false);
    assert!(
        criacao["request"]["required"]
            .as_array()
            .expect("campos")
            .contains(&"bundleId".into())
    );
    let leitura = operacoes
        .iter()
        .find(|op| op["id"] == "getRuntime")
        .expect("runtime");
    assert_eq!(leitura["authentication"], "runtime:read");
    assert_eq!(leitura["responses"]["304"], serde_json::Value::Null);
    assert_eq!(
        leitura["responses"]["200"]["properties"]["version"]["type"],
        "integer"
    );
    assert!(contrato["aadValidation"]["properties"]["schemaVersion"]["const"] == "1.0.0");
}

#[test]
fn rascunho_permite_json_arbitrario_e_publicacao_exige_a_estrutura_aad() {
    let contrato = exportar();
    let operacoes = contrato["operations"].as_array().expect("operações");
    let rascunho = operacoes
        .iter()
        .find(|op| op["id"] == "saveDraft")
        .expect("rascunho");
    assert_eq!(rascunho["request"], true);
    let runtime = operacoes
        .iter()
        .find(|op| op["id"] == "getRuntime")
        .expect("runtime");
    let definicao = &runtime["responses"]["200"]["properties"]["definition"];
    assert_eq!(definicao["properties"]["screens"]["type"], "array");
    assert_eq!(definicao["additionalProperties"], false);
}
