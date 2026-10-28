# Contratos de apps e runtime do CMS

Responsável: Codex, módulo CMS. Referência: AVL-343.

## Fonte do contrato

`cms_web::contratos::exportar`, compilado com a feature `contracts`, exporta
JSON Schema 2020-12 dos DTOs efetivamente usados nos handlers. O catálogo
`app_routes` define as 15 operações da API de apps e a leitura do runtime;
o despacho usa o mesmo catálogo. O erro JSON também tem DTO compartilhado.
Schemars é fixado em 1.2.3 e não integra as features padrão de produção.

A exportação distingue rascunho JSON arbitrário de definição AAD publicada.
Os schemas de transporte descrevem os tipos Serde; a validação contextual,
permissões, vínculo ao aplicativo, limites de negócio e transações continuam
no domínio e nos handlers. O schema AAD semântico já existente é exportado
separadamente em `aadValidation` e validado com o exemplo versionado.

## Compatibilidade e baseline

A CI exporta `target/contracts/apps-v1.json` do commit testado. O baseline
histórico é lido com `git show` de `docs/contracts/apps-v1.baseline.json` no
commit base do PR, nunca da cópia alterável do candidato. Na primeira entrega,
ausência do baseline é declarada por `initialContract`; não presume histórico.
O baseline versionado do candidato deve coincidir com a exportação atual.

O comparador verifica inclusão direcional: valores aceitos pela entrada antiga
devem continuar aceitos; saídas novas devem continuar aceitas pelo contrato
antigo. Confere operações, credenciais, status e cabeçalhos, propriedades,
obrigatoriedade, tipos, enumerações, limites e uniões discriminadas. Formatos e
padrões diferentes exigem revisão. Construções não suportadas ou sem prova
de inclusão são recusadas; isso pode reprovar uma mudança compatível, mas não
autoriza flexibilizar o gate sem uma prova e teste correspondente.

A comparação de JSON Schema não certifica a semântica de negócio. Aprovação
de contrato não dispensa os testes de autorização, isolamento e consistência.
O consumidor JavaScript exige inteiros seguros; JSON/JavaScript não representa
todos os valores i64 com precisão. O backend não passou a usar aritmética JS.

## Evidência executável

O workflow `Validar` instala as dependências de ferramenta por lockfile,
executa os testes do comparador/baseline/consumidor e compila os schemas Rust.
As suítes com Postgres produzem somente respostas sintéticas de apps/runtime
em `target/contracts/responses`. Cookies, tokens, códigos OAuth e corpos de
pedidos não são gravados. O coletor existe apenas na feature de CI.

O validador independente confere cada resposta capturada contra o schema e os
cabeçalhos declarados, e a passa ao consumidor JavaScript. Todas as 16 operações
precisam de resposta de sucesso exercitada. O exemplo AAD cobre texto, botão,
navegação e espaçador. Sucesso, conflitos, recusas e `304` permanecem distintos.

O artefato `cms-contracts` contém contrato atual, baseline histórico, respostas
sintéticas e relatório de compatibilidade, com revisão do baseline, revisão
executada, hash do contrato, operações cobertas e contagem de respostas.
`cms-mlops-evidence` também registra o resultado da etapa de contratos.

## Atualizar

1. Alterar o DTO/validação/handler e seus testes; não editar um schema para
   fazê-lo divergir do servidor.
2. Conferir a exportação na CI e versioná-la como baseline do próximo incremento.
3. Ler a comparação com o commit base; editar o baseline do candidato não remove
   uma incompatibilidade histórica. Uma quebra exige nova versão de contrato e
   migração explícita dos consumidores.
4. Conferir o artefato e registrar os resultados no Linear. Atualizações de
   fontes de políticas também precisam atualizar o manifesto de integridade.

Builds e testes exclusivamente no GitHub CI. O workflow `Consultas`, em modo
`somente_metadados`, prepara lockfile e diff de formatação como artefatos quando
o checkout não tem toolchain. Não executa aplicações em servidores.

## Limites e sequência do objetivo

Este módulo cobre apps v1 e leitura runtime. Não declara contratos completos
de OAuth, ferramentas MCP, conteúdo editorial ou n8n. Esses módulos continuam
na sequência do objetivo de desenvolvimento, assim como a instrumentação do
executor, autorização vigente no agendamento e recuperação de eventos.
