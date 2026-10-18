# Mobile Runtime Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Entregar OAuth de cliente nativo e leitura segura da definição AAD publicada para o runtime iOS.

**Architecture:** Credenciais móveis vivem em tabelas e tipos separados do MCP. O backend vincula cliente, token, conta e app e revalida a participação ao servir o ambiente; o pacote Swift consome somente a definição publicada pela fronteira validada.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL 18, Swift 5.9, Foundation, XCTest, GitHub Actions macOS.

**Spec:** `docs/superpowers/specs/2026-10-18-mobile-runtime-delivery-design.md`

## Global Constraints

- OAuth usa Authorization Code e PKCE S256 em agente externo.
- Tokens runtime e MCP permanecem incapazes de acessar o recurso um do outro.
- O endpoint móvel é somente leitura e nunca entrega rascunho.
- iOS 16 e macOS 13 permanecem os alvos mínimos.
- Nenhum segredo, deploy, TestFlight ou publicação automática.
- Testes pesados e builds executam somente no GitHub Actions.

## Review Focus

- Token válido para outro app recebe `404`, nunca o AAD.
- Revogar participação invalida a próxima leitura mesmo com token ainda ativo.
- Código ou refresh token reutilizado falha sem revelar qual campo divergiu.
- Token MCP em `/api/runtime` e token runtime em `/mcp` falham fechados.
- `304` só ocorre depois de autenticação e autorização completas.

---

### Task 1: Persistência OAuth runtime

**Files:**
- Create: `crates/dados/migrations/20261018000017_runtime_oauth.sql`
- Create: `crates/dados/src/runtime_oauth.rs`
- Modify: `crates/dados/src/lib.rs`
- Test: `crates/dados/tests/runtime_oauth.rs`

**Interfaces:**
- Produces: `RuntimeAuthorization`, `RuntimeTokens`, `runtime_client`, `create_runtime_code`, `exchange_runtime_code`, `rotate_runtime_tokens`, `runtime_session_by_access_token` e `revoke_runtime_session`.

- [ ] **Step 1:** Escrever testes SQLx para cliente vinculado ao app, PKCE, uso único, hashes, rotação e expiração.
- [ ] **Step 2:** Executar no GitHub e confirmar falha por migration/API ausentes.
- [ ] **Step 3:** Criar migration aditiva e funções mínimas de dados.
- [ ] **Step 4:** Executar suíte de dados e confirmar todos os casos verdes.

### Task 2: Autorização e token HTTP

**Files:**
- Create: `crates/web/src/runtime_oauth.rs`
- Create: `crates/web/templates/autorizar_runtime.html`
- Modify: `crates/web/src/lib.rs`
- Modify: `crates/web/src/painel.rs`
- Test: `crates/web/tests/runtime_oauth.rs`

**Interfaces:**
- Consumes: persistência da Task 1 e sessão Ávila Auth no navegador.
- Produces: `GET/POST /oauth/runtime/authorize`, `POST /oauth/runtime/token` e metadados específicos do recurso.

- [ ] **Step 1:** Escrever testes HTTP para pedido válido, cliente/retorno inválido, PKCE, `state`, recusa, troca e renovação.
- [ ] **Step 2:** Confirmar vermelho no GitHub.
- [ ] **Step 3:** Implementar handlers e consentimento sem compartilhar escopos MCP.
- [ ] **Step 4:** Confirmar suíte HTTP verde e ausência de tokens em logs.

### Task 3: Endpoint de entrega publicada

**Files:**
- Modify: `crates/dados/src/apps.rs`
- Create: `crates/web/src/runtime_apps.rs`
- Modify: `crates/web/src/painel.rs`
- Test: `crates/web/tests/runtime_apps.rs`

**Interfaces:**
- Produces: `published_runtime_definition(pool, account, app, environment)` e `GET /api/runtime/v1/apps/{slug}/environments/{environment}`.

- [ ] **Step 1:** Escrever testes para `200`, `304`, app/ambiente ausente, cross-tenant, participação revogada e separação de tokens.
- [ ] **Step 2:** Confirmar vermelho no GitHub.
- [ ] **Step 3:** Implementar consulta e handler somente leitura com ETag, no-store, correlação e desafio Bearer.
- [ ] **Step 4:** Confirmar testes HTTP e mutation check das guardas de autorização.

### Task 4: Cliente Swift do runtime

**Files:**
- Create: `platforms/ios/Sources/AvilaAADRuntime/AADRuntimeAPIClient.swift`
- Create: `platforms/ios/Tests/AvilaAADRuntimeTests/AADRuntimeAPIClientTests.swift`
- Modify: `platforms/ios/Sources/AvilaAADRuntime/AADDefinitionLoader.swift`

**Interfaces:**
- Produces: `AADRuntimeTransport`, `AADRuntimeAPIClient`, erros `unauthenticated`, `notModified`, `invalidResponse` e loader remoto autenticado.

- [ ] **Step 1:** Escrever XCTest com transporte real controlado para `200`, `304`, `401`, definição inválida e cabeçalhos.
- [ ] **Step 2:** Confirmar vermelho no runner macOS.
- [ ] **Step 3:** Implementar cliente com injeção, Keychain e decoder AAD estrito.
- [ ] **Step 4:** Confirmar Swift Package, build iOS e screenshots verdes.

### Task 5: Documentação, integração e evidência

**Files:**
- Modify: `docs/app-builder/api-v1.md`
- Modify: `docs/app-builder/ios-runtime-pilot.md`
- Create: `docs/app-builder/mobile-runtime-delivery.md`
- Modify: `.github/workflows/validar-swiftui.yml`

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces: contrato operacional, comandos, limites e artefatos de CI.

- [ ] **Step 1:** Documentar cadastro, autorização, token, entrega, erros e separação MCP/runtime.
- [ ] **Step 2:** Executar `git diff --check`, formatação, Clippy, Rust/SQLx e Swift no GitHub.
- [ ] **Step 3:** Revisar diff completo e segurança; corrigir achados importantes.
- [ ] **Step 4:** Abrir PR sem merge, deploy ou publicação automática e registrar evidências no projeto Linear.
