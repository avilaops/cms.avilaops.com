# Contratos OAuth do CMS

**Goal:** concluir os contratos de descoberta, registro, autorização e tokens
de MCP e runtime, mantendo os recursos e credenciais separados.

**Architecture:** DTOs compartilhados entre serialização e schemas; catálogo de
rotas usado pelo despacho; exportação Rust na feature de CI existente. Comparação
direcional e baseline histórico reaproveitam o módulo de contratos. Respostas
sintéticas são identificadas pela execução; tokens e códigos são suprimidos.

**Tech Stack:** Rust, Serde, Schemars 1.2.3, Axum, SQLx/Postgres e Ajv 8.20.0.

**Referência:** AVL-343; sequência do objetivo contínuo do módulo CMS.

## Entrega

- [x] Tipar respostas de metadados, registro, tokens e erros OAuth.
- [x] Derivar schemas dos tipos de pedidos reais; declarar query/form/JSON.
- [x] Compartilhar o catálogo das 12 operações com o despacho do painel.
- [x] Exportar e comparar o contrato OAuth com baseline do commit base.
- [x] Validar exemplos, consumidor independente e respostas de todas as operações.
- [x] Exigir identidade da captura e suprimir credenciais, query de redirects e HTML.
- [x] Exercitar PKCE, consentimento, retorno registrado, uso único e refresh/replay.
- [x] Conferir CI, artefatos, baseline e registrar evidência no Linear.

CI final: 40651479008, commit candidato d0a3cec, aprovada. Contrato exportado
igual ao baseline versionado; 68 respostas, 12 operações com sucesso e grants
authorization_code/refresh_token exercitados com sucesso e recusa nos dois
recursos. Evidência detalhada no documento OAuth da AVL-343. Sem merge/deploy.

## Limites e aceite

Schemas descrevem transporte; autenticação, participação e regras de OAuth
continuam no executor. As telas e redirects mantêm testes semânticos próprios.
A evidência de HTML registra somente o tipo, sem conteúdo; tokens preservam
tipo/presença e são suprimidos antes de gravar. Não declarar que o artefato
redigido verifica o valor secreto. A troca e o uso real do token são exercitados
em memória pelos testes existentes.

Uma resposta de sucesso de cada operação precisa ser exercitada. As variantes
de authorization_code e refresh_token precisam de sucesso e recusa de replay.
Nenhum segredo ou corpo de pedido entra no artefato. Os contratos MCP JSON-RPC
e das 37 ferramentas são o módulo seguinte, sem declaração de conclusão global.

Builds e testes seguem no GitHub CI existente. Sem merge ou deploy.
