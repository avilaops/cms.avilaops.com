# CMS Ávila Ops

Plataforma própria de sites da Ávila Ops, ao lado do
[Lojas](https://lojas.avilaops.com). O site já nasce correto em SEO técnico,
desempenho, dados estruturados, imagens, `llms.txt` e sinais de confiança, e o
painel impede quem edita de estragar isso.

## Estado

O motor está implementado e publicado como biblioteca. O CMS tem o site
público e o fluxo de publicação implementados e testados, sem painel e sem
login. Nenhum site está no ar.

| Frente | Entrega | Situação |
|---|---|---|
| Base | Convenções comuns em Rust: servidor, banco, login, publicação, n8n | Spec aprovada |
| Motor | Crate `motor-web`: validação, cabeçalho, JSON-LD, imagens, sitemaps, `robots.txt`, `llms.txt` | Implementado e publicado em [avilaops/motor-web](https://github.com/avilaops/motor-web) |
| CMS | Painel, conector MCP e renderização por host | Em implementação, por fatias |
| Lojas | Reescrita do Lojas em Rust, por fatias de rota | Spec aprovada, código não iniciado |

### Fatias do CMS

| Fatia | O que entrega | Situação |
|---|---|---|
| Site público | Site resolvido pelo host, páginas, posts, blog, sitemaps, `robots.txt`, `llms.txt`, mídia por site, semente de demonstração | Feita |
| Fluxo de publicação | Rascunho, revisão, publicação validada pelo motor, datas do servidor, troca de endereço com 301, despublicação com 410, permissão por papel, histórico | Feita |
| Esteira | `Dockerfile`, validação no GitHub Actions (`fmt`, `clippy`, testes contra Postgres), merge automático | Feita |
| Login e participação | Entrada pelo Auth no host do painel, participação por site, criação de site com limites por conta e por dia, convites de 48 horas, telas "Meus sites" e "Equipe" no painel | Feita |
| Painel | Telas de conteúdo, editor de blocos, prévia | A fazer |
| Mídia | Envio pelo painel com `alt` obrigatório, original em disco, rotina que gera AVIF e WebP, uso por documento, limite de espaço por site; imagem em uso não é apagada e sem variante não vai ao ar | Feita |
| Eventos e n8n | Fila de eventos gravada na transação do fato, entrega por webhook com nova tentativa, rota de volta com token próprio, chave do IndexNow servida por site, workflow `CMS - Operação` em [`n8n/`](n8n/cms-operacao.ts) | Feita para conteúdo, site novo e convite; o workflow está criado no n8n e ainda não publicado, à espera das duas credenciais próprias |
| Conector | Autorização e ferramentas para assistentes de IA | A fazer |
| Domínio próprio, cache e rotinas | Conferência de DNS, TLS sob demanda, cache por host, agendamento | A fazer |
| Primeira subida | Container no servidor de aplicações e site de demonstração no ar | A fazer; depende das pendências da spec do CMS |

## Como validar

Precisa de Rust estável e de um Postgres 18 para os testes de integração, que
criam uma base por teste.

```bash
export DATABASE_URL=postgres://postgres@127.0.0.1:5432/postgres
export SQLX_OFFLINE=true
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Mudou uma consulta ou uma migração: aplique as migrações em um banco de
desenvolvimento e regrave o diretório `.sqlx` com
`cargo sqlx prepare --workspace`, sem `SQLX_OFFLINE`.

## Pilha

Rust, com Axum no servidor, SQLx sobre Postgres, Askama nos templates e htmx
na interação. Um binário por aplicação, publicado em container no servidor de
aplicações da Ávila Ops. Sem Node em produção. O n8n cuida do que acontece
depois do fato e fala com terceiros.

## Como se organiza

- **Motor (`motor-web`)**: crate sem banco e sem rede. Recebe conteúdo e
  devolve os problemas que impedem a publicação e os artefatos da página. É
  usado pelo CMS e pelo Lojas, fixado por commit.
- **CMS (este repositório)**: uma aplicação que serve vários sites por host,
  com painel, login pelo Auth e conector para assistentes de IA. *Workspace*
  Cargo com `dominio` (regras puras), `dados` (consultas e migrações), `web`
  (Axum e templates) e `servidor` (o binário).
- **Lojas**: dono de catálogo, carrinho e checkout. Passa do Next.js para Rust
  uma fatia de rotas por vez, no mesmo domínio e no mesmo banco.
- **n8n**: recebe os eventos de cada produto por webhook e cuida de e-mail,
  aviso a buscadores, medição agendada e tarefas para a equipe. Os workflows
  ficam versionados no repositório do produto.

## Decisões que valem para tudo

Tomadas por Nicolas em 08 e 09/10/2026. O detalhe de cada uma está nas specs.

- A plataforma é Rust desde o dia 0, com painel renderizado por template.
- SQLx é o dono do esquema do banco.
- Sem Cloudflare e sem Twilio.
- O que decide se algo pode acontecer fica no Rust; o que acontece depois do
  fato e fala com terceiros vai para o n8n.
- Todo e-mail sai de `noreply@avilaops.com`.
- Nenhuma regra mora na tela: painel, API e conector passam pela mesma
  validação do motor.
- O Lojas migra por fatias de rota, e o envio de mensagens dele continua no
  servidor.
- Este repositório e o do motor são públicos: nada de dado, nome ou domínio de
  cliente em código, teste ou documento.

## Documentos

- [Base Rust](docs/superpowers/specs/2026-10-09-base-rust-design.md)
- [Motor](docs/superpowers/specs/2026-10-09-motor-web-design.md)
- [CMS](docs/superpowers/specs/2026-10-09-cms-design.md)
- [Lojas em Rust](docs/superpowers/specs/2026-10-09-lojas-rust-design.md)
- [Modo de trabalho dos agentes](AGENTS.md)
