# Ávila App Definition — AAD v1.0 (primeiro incremento)

O App Builder é um domínio separado do `motor-web`. A definição tipada vive no
crate `cms-dominio`; o Registry, a API e o painel consomem o mesmo contrato. O
runtime iOS permanece uma entrega separada.

## Contrato inicial
- `schemaVersion` deve ser `1.0.0`.
- `minimumRuntimeVersion` é `1.0.0` neste protótipo, antes de negociação SemVer.
- `appId`, ids e rotas são slugs em ASCII minúsculo com hífens, com até 64 caracteres.
- Limite: 1–100 telas e até 200 componentes por tela.
- Componentes: `text`, `button`, `spacer`.
- Ações: apenas `navigate`, com destino existente.
- Campos JSON desconhecidos são recusados por Serde.

## Compatibilidade
O contrato formal está em [`aad-v1.schema.json`](aad-v1.schema.json). Esta
primeira versão continua restrita a navegação determinística e aos três
componentes documentados. Tokens de design, fontes de dados, recursos nativos,
assinatura do manifesto e runtime Apple exigem versões futuras do contrato.

## Execução local
```bash
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p cms-dominio --locked
```

## Camadas entregues
1. Contrato AAD tipado, JSON Schema, fixture e testes.
2. App Registry e migrações SQLx aditivas, isolados do domínio de sites.
3. [API e painel](api-v1.md), com autorização, revisão otimista, versões
   imutáveis, ambientes e auditoria.
4. Runtime SwiftUI: ainda não entregue; depende da validação do contrato e de
   segurança.

A entrega não deve ser marcada como pronta nem mesclada automaticamente antes de CI, revisão técnica e aprovação explícita.
