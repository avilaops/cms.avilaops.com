# Base Rust da plataforma: desenho

Data: 09/10/2026. Convenções comuns ao CMS e ao Lojas em Rust. Não é um
produto: é o que os dois têm de igual, escrito uma vez.

Specs que dependem desta: `2026-10-09-motor-web-design.md`,
`2026-10-09-cms-design.md` e `2026-10-09-lojas-rust-design.md`.

## Decisões de Nicolas (09/10/2026)

- A plataforma é escrita em Rust desde o dia 0.
- Painel administrativo também em Rust, renderizado por template. Não há React
  nem Node em produção.
- SQLx é o dono do esquema do banco desde o dia 0.
- Sem Cloudflare e sem Twilio: a plataforma usa a estrutura própria da Ávila
  Ops. Esta decisão prevalece, nestes projetos, sobre a linha do contexto
  corporativo que cita os dois.
- O n8n assume o que não precisa ser código Rust: o que acontece depois do
  fato e fala com terceiros. Ver "O que vai para o n8n".
- Todo e-mail sai de `noreply@avilaops.com`.

## O que se espera do Rust, e o que não se promete

O ganho escrito aqui é o que dá para sustentar: um binário único por
aplicação, memória por processo bem menor que a de um servidor Node, tempo de
resposta previsível sem pausa de coletor, e erro de tipo, de consulta SQL e de
template pego na compilação.

Não entram como requisito: resposta "em menos de 5 ms" e ganho "em ordens de
grandeza". O tempo de uma página de loja é dominado pelo Postgres. Cada
aplicação mede o seu, e a meta vem da medição.

A capacidade para um número muito grande de sites depende mais do banco, do
cache e do TLS por domínio do que da linguagem. Esta spec não dimensiona isso;
registra as escolhas que não fecham a porta.

## Pilha

| Parte | Escolha | Motivo |
|---|---|---|
| Servidor HTTP | Axum sobre Tokio | Roteamento tipado, `tower` para camadas |
| Banco | SQLx com Postgres 18 | Consulta conferida em compilação, pool assíncrono |
| Templates | Askama | HTML em arquivo, escape automático, erro em compilação |
| Interação | htmx e JavaScript pontual | Painel de formulários, sem empacotador |
| Regras de conteúdo | crate `motor-web` | Mesma régua nos dois produtos |
| Serialização | `serde`, `serde_json` | |
| Erros | `thiserror` nas bibliotecas | Erro tipado; sem `unwrap` fora de teste |
| Registro | `tracing` com saída JSON | |
| HTTP de saída | `reqwest` com `rustls` | Sem OpenSSL na imagem |

Askama foi preferido ao Maud porque o template é um arquivo HTML: quem edita
layout, pessoa ou agente, mexe em HTML, e não em macro Rust.

O JavaScript que sobra é servido como arquivo estático, sem etapa de build:
htmx, o carrinho da vitrine e o SDK de cartão do Mercado Pago, que é do próprio
Mercado Pago em qualquer cenário.

## Organização do código

Cada aplicação é um *workspace* Cargo com esta divisão:

| Crate | Conteúdo | Pode depender de |
|---|---|---|
| `dominio` | Regras puras, tipos, cálculo | `motor-web`, `serde` |
| `dados` | Consultas SQLx, migrações | `dominio` |
| `integracoes` | Clientes HTTP de terceiros | `dominio` |
| `web` | Axum, templates, sessões | os três acima |
| `servidor` | `main`, configuração, agendador | todos |

Regra que vale nos dois produtos: nada por site em código. Se a mudança pede
`if site == "x"`, vira dado.

Português nos nomes e comentários. Dinheiro em centavos, `i64`.

## Banco e esquema

- Migrações em `dados/migrations`, aplicadas por `sqlx migrate run`.
- Consultas com `query!` e `query_as!`. O diretório `.sqlx` é versionado
  (`cargo sqlx prepare`), para o build do container não precisar de banco.
- Migração só acrescenta: tabela, coluna nula, índice. Remover coluna vai em
  deploy separado, depois de sair o código que a usava.
- Regra que hoje mora no banco continua no banco: gatilho, `CHECK`, índice
  parcial. O Rust chama; não reimplementa.
- Transação explícita onde há dinheiro ou estoque. `SELECT … FOR UPDATE`
  continua sendo SQL escrito à mão.
- Teste de integração em container próprio, na imagem de produção do Postgres,
  nunca no banco de desenvolvimento.

## Resolução do site pelo host

- O servidor lê `X-Forwarded-Host` e, na falta dele, `Host`; normaliza e
  resolve o site.
- Não há cache do site entre requisições. Um mapa com prazo de validade já
  causou bug no Lojas. A consulta é por índice único, e o resultado vive só no
  pedido.
- `www.` redireciona para o apex com 308.
- Host desconhecido responde 404 sem tocar em dado de site algum.

## Login

O login do painel é do Auth (`auth.avilaops.com`).

- Em host `*.avilaops.com`: ler o cookie `avila_sso`, verificar o JWT com
  `SSO_JWT_SECRET`, **fixando HS256**, exigindo `iss == "auth.avilaops.com"` e
  `exp`. Campos: `sub`, `email`, `nome`, `foto`, `papel`, `mfa`.
- Sessão válida não é autorização: o cookie vale em todo `*.avilaops.com`. A
  aplicação confere `permitido` em `GET /api/session?app=<id>`, servidor a
  servidor, e guarda a própria participação por conta.
- Sem sessão: 302 para `/login?app=<id>&returnTo=<url https do próprio host>`.
- Em domínio próprio de cliente o cookie não chega. O caminho é OIDC
  (`/oauth/authorize`, `/oauth/token`, `/oauth/userinfo` do Auth), com
  `client_id` e `client_secret` da aplicação.
- O Auth não entrega a empresa da conta. Cada aplicação mantém a tabela de
  participação (conta, site, papel).
- A sessão própria da aplicação é um cookie `HttpOnly`, `SameSite=Lax`,
  `Secure`, assinado com HMAC-SHA256, e o registro é relido do banco a cada
  pedido.

Pendência no Auth, fora desta spec: os tokens são HS256 com segredo
compartilhado, então toda aplicação que valida também poderia assinar. A troca
para RS256 está no roteiro do Auth como "depois".

## Estado que não pode viver na memória do processo

No Lojas atual quatro coisas vivem na memória de um processo e somem no deploy.
Com mais de uma instância elas dão resultado errado. Na base Rust:

| Estado | Onde fica |
|---|---|
| Limitador de requisições por chave | Tabela com janela, atualizada por `UPDATE` atômico |
| Tentativas de login | Tabela, mesma técnica |
| Métricas por site | Agregado em memória, descarregado no banco a cada minuto; perder um minuto é aceito |
| Fila de imagens | Tabela de tarefas com reivindicação por `UPDATE … WHERE` |

Nenhum serviço novo entra por causa disso. Redis ou fila externa só com medição
que mostre que o Postgres não dá conta.

## Rotinas

O que roda sozinho é um catálogo no código, disparado pelo próprio servidor a
cada minuto. A trava é a linha da rotina no banco, então várias instâncias não
duplicam trabalho. Rotina nova é entrada no catálogo, não agendamento em
serviço de fora.

## Imagens

- Decodificar e redimensionar: crate `image`.
- WebP: crate `webp` (libwebp), porque o codificador WebP do `image` é só sem
  perdas e gera arquivo maior que o JPG de origem.
- AVIF: o codificador do próprio `image` (`ravif` por baixo). Leva segundos
  por imagem.
- Arquivo aceito no envio: JPG, PNG ou WebP. AVIF é só formato de saída.
- Tudo isso já está no `motor-web`, atrás da *feature* `imagem`: a aplicação
  chama `processar_imagem` e cuida só da fila e do disco.
- O upload grava o original, responde, e as variantes saem pela fila de
  tarefas. Enquanto não existem, a página usa o original com as dimensões já
  conhecidas.
- O processamento roda em `spawn_blocking`, com limite de tarefas simultâneas.
- Arquivos em disco, no volume do container, servidos pelo próprio servidor com
  cache longo: o nome leva o hash do conteúdo.

## Segurança

- Segredo de terceiro (token de gateway, de canal, de frete) só cifrado no
  banco, AES-256-GCM. Nunca em log, nunca em resposta.
- Token e chave de API guardados só como SHA-256.
- Comparação de segredo em tempo constante.
- Requisição para endereço digitado por cliente: só `https`, nome resolvido
  antes e recusado se for rede interna, redirecionamento não seguido.
- Texto de quem edita nunca é marcação. O que vai para o HTML passa pelo escape
  do Askama ou do `motor-web`.
- Cabeçalhos em toda resposta HTML: `Content-Security-Policy` sem `unsafe-inline`
  para script, `X-Content-Type-Options`, `Referrer-Policy`.

## Esteira de publicação

O caminho em uso é o build manual do `avilaops/infra`
(`scripts/publicar-manual.ps1`): build no `apps-noclient`, troca no servidor de
aplicações.

O GitHub Actions roda em repositório público da conta: o `motor-web` valida lá
desde 09/10/2026. O bloqueio por cobrança vale para repositório privado. Como
o repositório do CMS é público, a validação dele (`fmt`, `clippy`, testes)
roda no Actions; a publicação segue pelo caminho manual até o infra decidir o
contrário.

O que cada aplicação cumpre:

- `Dockerfile` na raiz, contexto `.`, `linux/amd64`, sem build-arg além de
  `GIT_SHA` e sem segredo de build.
- Build em estágios com `cargo-chef`, para a camada de dependências ficar em
  cache. Imagem final `debian:bookworm-slim`, com as bibliotecas dinâmicas que
  o binário usa e o `curl` do healthcheck.
- Escuta em `0.0.0.0:<porta>`; `expose` no compose, nunca `ports`.
- `GET /api/saude`, sem autenticação: 200 quando o banco responde, 503 quando
  não. O binário precisa ficar saudável em 90 segundos.
- `.conf` em `deploy/production/<dominio>.conf` do infra, com `HEALTH_URL`.
  `CONTAINER_HEALTH_URL` não serve: executa `node` dentro da imagem.
- A primeira subida de um container é manual: o `avila-deploy` exige container
  existente. O bloco do Caddy e a inclusão do banco na rotina de backup também.
- Workflow do repositório: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, e `merge-automatico.yml` no padrão da casa.
- Build de aplicação nunca roda no servidor de produção.

### Pré-requisito no infra: migração sem Prisma

Hoje o `avila-deploy` só sabe migrar com Prisma, e o dump antes da migração só
dispara dentro desse gancho. Antes da primeira aplicação Rust ir ao ar, o
`scripts/deploy-container.sh` e o `scripts/deploy-container-local.sh` ganham um
modo novo:

- `MIGRATE_MODE=comando` e `MIGRATE_COMMAND`, executado em container temporário
  da imagem nova, com o `MIGRATE_ENV_FILE`.
- A aplicação expõe `servidor migrar --conferir` (sai com 0 se não há
  pendência) e `servidor migrar`.
- Havendo pendência e `MIGRATE_DUMP_DB` definido, o dump roda antes, como já
  acontece com Prisma.
- Falhou a migração, o serviço não é tocado.
- Volta de versão troca a imagem e não desfaz migração.

O binário não migra sozinho ao iniciar: com mais de uma instância, duas
migrariam ao mesmo tempo, e o dump prévio deixaria de existir.

## Sem Cloudflare

- DNS dos domínios em registro A direto para o servidor de aplicações, como já
  é com `crm.avilaops.com`.
- TLS pelo Caddy do host. Domínio próprio de cliente usa TLS sob demanda, com o
  Caddy perguntando à aplicação se o host é de um site ativo
  (`GET /api/dominio-permitido`), como o Lojas já faz.
- Cache de página: a aplicação manda `Cache-Control` com `s-maxage` e
  `stale-while-revalidate`, e o cache fica em memória no próprio servidor,
  por host e caminho, invalidado ao publicar. Só para pedido sem cookie de
  sessão.
- Sem CDN, imagem e página saem de um ponto só. É aceito enquanto o público for
  do Brasil; a medição diz quando deixa de ser.

Pendência: o Lojas atual cria registro de DNS pela API da Cloudflare ao
provisionar loja (`src/lib/provisionar.ts`, `src/lib/dominio.ts`). A spec do
Lojas trata a troca.

## Sem Twilio

WhatsApp pela Cloud API da Meta, como o Lojas já faz. A conexão com a Meta é do
Auth: cada sistema lê por `GET /api/meta/ativos`. SMS e voz não fazem parte da
plataforma.

## Remetente de e-mail

Decisão de Nicolas (09/10/2026): todo e-mail da plataforma sai de
`noreply@avilaops.com`, seja enviado pelo servidor ou pelo n8n. O que muda por
site ou por loja é o nome de exibição e o `Reply-To`, nunca o endereço do
remetente. Não se cria remetente por produto nem por cliente.

## O que vai para o n8n

Decisão de Nicolas (09/10/2026): o n8n assume o que não precisa ser código
Rust. O que acontece depois do fato e fala com terceiros vai para o n8n:
mensagem a pessoa, aviso a buscador, medição agendada, tarefa para a equipe. O
que decide se algo pode acontecer fica no Rust: validação, permissão, cobrança,
estoque, publicação.

- O servidor grava o evento no banco, na transação do fato, e entrega por
  webhook com `authorization`. O n8n responde na hora e encerra o evento por
  uma rota de volta.
- Entrega pelo menos uma vez; o mesmo fato tem o mesmo identificador.
- Nada no caminho do visitante ou de quem edita espera o n8n.
- Workflow é código: fica versionado no repositório do produto e é criado pelo
  conector do n8n. Aponta o erro para o `Handler de Erro Central → Todoist`.
- No CMS, o envio de e-mail é todo do n8n: não há cliente de SMTP no servidor.
  No Lojas a decisão é outra e está na spec dele.

## Testes

- Unitários ao lado do código, `cargo test`.
- Integração contra Postgres em container, uma base por suíte.
- Saída de texto (HTML, XML, JSON-LD) comparada com referência versionada, com
  o crate `insta`.
- Nome de cliente real não entra em fixture, exemplo ou teste.

## O que esta spec não decide

- Dimensionamento do banco, réplica de leitura e particionamento.
- Onde ficam os arquivos quando um disco só não bastar.
- Troca do HS256 no Auth.
