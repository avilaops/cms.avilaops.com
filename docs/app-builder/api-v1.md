# API do App Builder v1

Todos os endpoints ficam no host privado do CMS, exigem sessão do Ávila Auth e
respondem JSON com `Cache-Control: private, no-store` e `X-Correlation-Id`.
Mutações com cookie exigem `Origin` igual ao painel.

## Projetos

- `GET /api/v1/apps`: lista os aplicativos da conta.
- `POST /api/v1/apps`: cria um projeto com `slug`, `name` e `bundleId`.
- `GET /api/v1/apps/{id}`: lê metadados e papel da conta.

## Rascunho e validação

- `GET /api/v1/apps/{id}/draft`: devolve definição e revisão; envia `ETag`.
- `PUT /api/v1/apps/{id}/draft`: grava uma definição com `If-Match` obrigatório.
- `POST /api/v1/apps/{id}/validate`: valida o AAD enviado ou o rascunho salvo.

Conflito de revisão responde `409 draft_conflict`. Um rascunho não vira versão
sem desserializar como `AppDefinition`, passar pela validação AAD e possuir
`appId` igual ao slug do projeto.

## Versões e ambientes

- `GET /api/v1/apps/{id}/versions`: lista snapshots imutáveis.
- `GET /api/v1/apps/{id}/versions/{number}`: lê uma definição versionada.
- `POST /api/v1/apps/{id}/versions`: cria uma versão; exige
  `Idempotency-Key`.
- `GET /api/v1/apps/{id}/environments`: lista publicações.
- `PUT /api/v1/apps/{id}/environments/{name}`: publica uma versão em
  `development`, `staging` ou `production`.

Repetir a mesma chave idempotente devolve a versão original. Rollback é uma
nova publicação de uma versão anterior; o snapshot nunca é alterado.

## Participantes e auditoria

- `GET /api/v1/apps/{id}/members`: lista participantes; apenas proprietário.
- `PUT /api/v1/apps/{id}/members`: adiciona ou muda `account` e `role`.
- `DELETE /api/v1/apps/{id}/members`: remove `account`.
- `GET /api/v1/apps/{id}/audit`: lista os 50 eventos mais recentes.

Papéis: `owner`, `editor` e `viewer`. O último proprietário não pode ser
removido nem rebaixado. A API responde `404` em acesso de outro tenant para
não revelar a existência do aplicativo.

## Painel

- `/painel/apps`: lista e cria projetos.
- `/painel/apps/{id}`: editor AAD, preview de referência, versões, ambientes,
  participantes e auditoria.

O preview web é uma representação determinística do contrato. Ele não é
descrito como teste do runtime Apple e não executa JavaScript vindo do AAD.
