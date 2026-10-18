# Entrega móvel autenticada do App Builder — design

## Intenção

Permitir que um aplicativo iOS público obtenha a definição AAD publicada para
um ambiente sem reutilizar o cookie do painel nem ampliar os privilégios das
conexões MCP. Esta fase entrega OAuth para cliente nativo e leitura remota;
cache assinado, app host e distribuição permanecem nas etapas seguintes do
roadmap.

## Objetivos

- usar Authorization Code com PKCE S256 em navegador externo;
- vincular cada cliente nativo a um único aplicativo e a retornos cadastrados;
- emitir tokens opacos, curtos e armazenados no banco somente como SHA-256;
- autorizar a leitura novamente no servidor pela conta, aplicativo e ambiente;
- entregar somente uma versão AAD já publicada e validada;
- oferecer cliente Swift injetável, renovação e reautenticação segura;
- manter builds e testes pesados exclusivamente no GitHub Actions.

## Não objetivos

- transformar o conector MCP em token de aplicativo;
- implementar OpenID Connect ou emitir ID Token nesta fase;
- aceitar registro dinâmico aberto de clientes iOS;
- entregar rascunhos, editar conteúdo ou publicar versões pelo runtime;
- cache offline, assinatura do manifesto, TestFlight ou App Store.

## Decisões

### Recurso OAuth separado

O recurso móvel usa tabelas e tipos próprios: `cliente_runtime`,
`codigo_runtime` e `sessao_runtime`. As tabelas `*_mcp`, seus escopos e seus
tokens não são reutilizados. O único escopo móvel inicial é `runtime:read`.

O cliente é público e não possui segredo. Um proprietário cadastra no painel
um nome e de um a cinco retornos. Cada cliente pertence a um único `app_id`;
o cadastro não é um endpoint público. Retornos aceitos são HTTPS ou esquema
privado em notação DNS reversa que corresponda ao `bundle_id` do app.

### Autorização

`GET /oauth/runtime/authorize` abre no agente externo. O CMS autentica a pessoa
pela sessão existente do Ávila Auth, valida cliente, retorno, `state`, PKCE S256
e participação atual no aplicativo. A tela mostra aplicativo, cliente e o
escopo somente leitura. A aprovação cria código de uso único válido por cinco
minutos.

`POST /oauth/runtime/token` troca código ou refresh token. Access token vale
uma hora; refresh token vale 30 dias e gira a cada uso. Falha de verificador,
cliente, retorno, token ou expiração responde o mesmo `invalid_grant`.

OAuth autentica o acesso ao recurso; não é chamado de OIDC porque esta fase não
emite `id_token`, não expõe `userinfo` e não promete identidade ao cliente.

### Entrega

`GET /api/runtime/v1/apps/{slug}/environments/{environment}` exige Bearer do
recurso móvel. O token precisa pertencer ao cliente do mesmo app, conter
`runtime:read`, estar ativo e a conta ainda precisa possuir `AppAction::Read`.
Ambiente aceita somente `development`, `staging` ou `production`.

Resposta `200`:

```json
{
  "appId": "app-exemplo",
  "environment": "production",
  "version": 7,
  "definition": {}
}
```

O `ETag` identifica app, ambiente e versão. `If-None-Match` igual responde
`304`. Toda resposta usa `Cache-Control: private, no-store` até a fase de cache
assinado e inclui `X-Correlation-Id`. App, ambiente ou participação ausente
responde `404` para não revelar existência; token ausente, inválido ou expirado
responde `401` com metadados do recurso protegido.

### Swift

`AADRuntimeAPIClient` recebe `baseURL`, `appId`, `environment`, um transporte
HTTP e um `AADCredentialStoring`. Ele envia o access token, aceita `200` ou
`304`, decodifica `definition` exclusivamente por `AADDefinitionDecoder` e
nunca registra corpo ou credencial. `401` produz estado de reautenticação; a
orquestração de `ASWebAuthenticationSession` pertence ao futuro app host.

## Segurança

- navegador externo e PKCE seguem RFC 8252 e RFC 9700;
- nenhum segredo de cliente é embutido no aplicativo;
- códigos são de uso único, tokens são opacos e somente hashes chegam ao banco;
- refresh token gira e a sessão pode ser revogada;
- token MCP nunca autoriza `/api/runtime` e token runtime nunca autoriza `/mcp`;
- autorização é reavaliada no servidor a cada leitura;
- nenhum AAD vem de rascunho ou versão não publicada.

## Compatibilidade e migração

A migration é apenas aditiva. Rotas administrativas e OAuth MCP preservam
contratos atuais. A fase começa sem clientes runtime; portanto nenhum app ganha
acesso móvel implicitamente.

## Evidência e aceite

1. Testes de domínio/dados cobrem PKCE, uso único, rotação, expiração e hashes.
2. Testes HTTP cobrem consentimento, leitura, `304`, cross-tenant, token MCP,
   token expirado, ambiente ausente e revogação de participação.
3. XCTest cobre `200`, `304`, `401`, payload inválido e ausência de vazamento.
4. `cargo fmt`, Clippy, Rust/SQLx, Swift e build iOS passam no GitHub Actions.
5. Nenhum deploy, assinatura ou publicação é executado automaticamente.

## Referências normativas

- RFC 8252, OAuth 2.0 for Native Apps.
- RFC 9700, OAuth 2.0 Security Best Current Practice.
- RFC 7636, Proof Key for Code Exchange.
- Apple AuthenticationServices, `ASWebAuthenticationSession`.
