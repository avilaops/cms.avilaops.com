# Entrega autenticada do runtime móvel

Este contrato liga o App Registry ao pacote `AvilaAADRuntime` sem reutilizar o
OAuth do MCP. Ele entrega somente snapshots publicados e não oferece escrita.

## Provisionamento

Um cliente nativo é pré-cadastrado com:

- `app_id` único;
- nome exibido no consentimento;
- de um ou mais retornos exatos do aplicativo.

O cadastro usa a operação interna `register_runtime_client`; não há registro
dinâmico público. O retorno usado na autorização e na troca precisa ser
idêntico a um retorno cadastrado.

## Autorização

O aplicativo cria um verificador PKCE de 43 a 128 caracteres, calcula o desafio
S256 e abre no navegador externo:

```text
GET /oauth/runtime/authorize
  ?response_type=code
  &client_id={uuid}
  &redirect_uri={retorno codificado}
  &code_challenge={desafio}
  &code_challenge_method=S256
  &scope=runtime%3Aread
  &state={valor imprevisível}
```

O CMS autentica pelo Ávila Auth, confere cliente, retorno, PKCE e participação
no aplicativo. Aprovar redireciona com `code` e o mesmo `state`; recusar envia
`error=access_denied`. O código vence em cinco minutos e só pode ser tentado uma
vez, inclusive quando o verificador está errado.

## Tokens

A troca usa `application/x-www-form-urlencoded`:

```text
POST /oauth/runtime/token
grant_type=authorization_code&code=...&code_verifier=...&client_id=...&redirect_uri=...
```

Renovação:

```text
POST /oauth/runtime/token
grant_type=refresh_token&refresh_token=...&client_id=...
```

Resposta:

```json
{
  "access_token": "opaco",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "opaco",
  "scope": "runtime:read"
}
```

O refresh token vale 30 dias e gira em cada uso. O par anterior deixa de valer.
Código, PKCE, cliente, retorno, refresh inválido ou expirado produzem o mesmo
`invalid_grant`, sem indicar qual comparação falhou.

## Entrega publicada

```text
GET /api/runtime/v1/apps/{slug}/environments/{environment}
Authorization: Bearer {access_token}
Accept: application/json
If-None-Match: "{etag anterior}"
```

`environment` aceita `development`, `staging` ou `production`. Resposta `200`:

```json
{
  "appId": "app-exemplo",
  "environment": "production",
  "version": 7,
  "definition": {}
}
```

O servidor confere que o token pertence ao mesmo aplicativo, possui
`runtime:read`, continua ativo e que a conta ainda participa do aplicativo. Só
então compara o `ETag` e pode responder `304`. Rascunhos e versões sem ponteiro
de ambiente nunca são entregues.

## Erros e isolamento

- `401 unauthenticated`: Bearer ausente, inválido, expirado ou de outro recurso;
- `403 insufficient_scope`: sessão válida sem `runtime:read`;
- `404 not_found`: aplicativo, ambiente, publicação ou participação ausente;
- `405 method_not_allowed`: método diferente de `GET` na entrega.

Todas as respostas da entrega usam `Cache-Control: private, no-store` e
`X-Correlation-Id`. Tokens são tratados como segredo e não entram em logs. MCP
e runtime usam tabelas, hashes, escopos e metadados separados.

## Swift

`AADRuntimeAPIClient` recebe URL base, slug, ambiente, transporte e um
`AADCredentialStoring`. O cliente lê `AADRuntimeCredential` do Keychain, envia
Bearer e `If-None-Match`, aceita apenas `200` ou `304` e passa a definição pelo
`AADDefinitionDecoder`. `401` vira `unauthenticated`; `304`, `notModified`;
envelope ou AAD inválido, `invalidResponse`.

Esta fase não executa login dentro do pacote e não publica build. A
orquestração de `ASWebAuthenticationSession`, cache assinado, última versão
válida e rollback pertencem às próximas fases do roadmap.
