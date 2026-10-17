# Ávila App Definition — AAD v1.0 (primeiro incremento)

O App Builder é um domínio separado do `motor-web`. Esta entrega introduz a definição tipada no crate `cms-dominio`, um exemplo e testes unitários. **Não inclui App Registry, tabelas, API, painel ou runtime iOS.**

## Contrato inicial
- `schemaVersion` deve ser `1.0.0`.
- `minimumRuntimeVersion` é `1.0.0` neste protótipo, antes de negociação SemVer.
- `appId`, ids e rotas são slugs em ASCII minúsculo com hífens, com até 64 caracteres.
- Limite: 1–100 telas e até 200 componentes por tela.
- Componentes: `text`, `button`, `spacer`.
- Ações: apenas `navigate`, com destino existente.
- Campos JSON desconhecidos são recusados por Serde.

## Compatibilidade
Este é um contrato experimental de primeira fase, ainda não estável nem pronto para distribuição por um runtime Apple. A evolução para JSON Schema formal, limites de bytes/profundidade, tokens de design, fontes de dados, recursos nativos e assinaturas de manifesto exige entregas posteriores.

## Execução local
```bash
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p cms-dominio --locked
```

## Sequência dos próximos PRs
1. Fechar contrato AAD com JSON Schema e testes de compatibilidade e de entradas maliciosas.
2. Adicionar App Registry e migrations SQLx aditivas, preservando o isolamento atual por site.
3. Criar endpoints Axum e painel Askama/htmx, com autorização, revisão e versões imutáveis.
4. Desenvolver runtime SwiftUI após validação do contrato e de segurança.

A entrega não deve ser marcada como pronta nem mesclada automaticamente antes de CI, revisão técnica e aprovação explícita.
