# CMS Ávila Ops

Plataforma própria de sites da Ávila Ops, ao lado do
[Lojas](https://lojas.avilaops.com). O site já nasce correto em SEO técnico,
desempenho, dados estruturados, imagens, `llms.txt` e sinais de confiança, e o
painel impede quem edita de estragar isso.

## Estado

O motor está implementado e publicado como biblioteca. O CMS tem todas as
fatias de código implementadas e testadas: site público, painel, mídia,
conector e rotinas. A aplicação está no ar desde 10/10/2026, com o site de
demonstração publicado.

| Frente | Entrega | Situação |
|---|---|---|
| Base | Convenções comuns em Rust: servidor, banco, login, publicação, n8n | Spec aprovada |
| Motor | Crate `motor-web`: validação, cabeçalho, JSON-LD, imagens, sitemaps, `robots.txt`, `llms.txt` | Implementado e publicado em [avilaops/motor-web](https://github.com/avilaops/motor-web) |
| CMS | Painel, conector MCP e renderização por host | Em implementação, por fatias |
| Lojas | Reescrita do Lojas em Rust, por fatias de rota | Spec aprovada, código não iniciado |

### Fatias do CMS

| Fatia | O que entrega | Situação |
|---|---|---|
| Site público | Site resolvido pelo host, páginas, posts, blog, listagem por categoria (`/blog/categoria/<slug>`) e por autor (`/autor/<slug>`), sitemaps, `robots.txt`, `llms.txt`, mídia por site, semente de demonstração | Feita |
| Fluxo de publicação | Rascunho, revisão, publicação validada pelo motor, datas do servidor, troca de endereço com 301, despublicação com 410, permissão por papel, histórico | Feita |
| Esteira | `Dockerfile`, validação no GitHub Actions (`fmt`, `clippy`, testes contra Postgres), merge automático | Feita |
| Login e participação | Entrada pelo Auth no host do painel, participação por site, criação de site com limites por conta e por dia, convites de 48 horas, telas "Meus sites" e "Equipe" no painel | Feita |
| Painel | Página do site com a lista de páginas e posts, editor de blocos sem JavaScript, com abertura de página e dez seções (cartões, imagem e texto, depoimentos, números, passos, planos, galeria, logos, faixa de chamada e contato), doze modelos de página e de post para começar, prévia pelo template do site, ações de revisão e publicação por papel, cadastro de autores e categorias | Feita, com as telas de identidade do site e de histórico. O menu sai das páginas publicadas e o tema é um só; editor de menu e de aparência ficam para quando houver segundo tema |
| Mídia | Envio pelo painel com `alt` obrigatório, original em disco, rotina que gera AVIF e WebP, legenda, crédito e direitos por imagem (autoria, aviso, licença e página de aquisição) corrigíveis depois do envio, `ImageObject` no dado estruturado e `max-image-preview:large`, uso por documento, limite de espaço por site; imagem em uso não é apagada e sem variante não vai ao ar | Feita |
| Eventos e n8n | Fila de eventos gravada na transação do fato, entrega por webhook com nova tentativa, rota de volta com token próprio, chave do IndexNow servida por site, workflow `CMS - Operação` em [`n8n/`](n8n/cms-operacao.ts) | Feita para conteúdo, site novo e convite; o workflow está criado no n8n e ainda não publicado, à espera das duas credenciais próprias |
| Conector | Servidor de autorização próprio (registro dinâmico, PKCE S256, tokens só como hash), ponto MCP com 21 ferramentas por escopo (inclui cadastro de autor e categoria, edição da identidade, correção de imagem, envio de imagem por endereço e modelos de página; no post, autor, categoria e imagem vão só pela referência), tela de conexões com desconectar, registro de chamadas sem argumentos | Feita |
| Domínio próprio, cache e rotinas | Pedido de domínio pelo Dono, conferência de DNS por rotina, resposta ao Caddy para TLS sob demanda, endereço provisório assumido como definitivo, cache de página em memória derrubado ao publicar, publicação agendada, limpeza de histórico, catálogo de rotinas com trava no banco | Feita |
| Primeira subida | Container no servidor de aplicações e site de demonstração no ar | Feita em 10/10/2026: painel em [cms.avilaops.com](https://cms.avilaops.com) e demonstração em [demonstracao.sites.avilaops.com](https://demonstracao.sites.avilaops.com), com Lighthouse 100 nas quatro categorias no celular (página inicial, post e listagem). O Caddy já encaminha domínio próprio de cliente. Versão nova vai ao ar com `scripts/publicar.sh` |

## Como publicar

```bash
scripts/publicar.sh
```

Publica o commit de `origin/main`: build no `apps-noclient`, imagem levada ao
servidor de aplicações, dump do banco antes de migrar quando há migração
pendente, troca do contêiner e volta à imagem anterior se o novo não ficar
saudável. Precisa de `apps-noclient` e `applications` no `~/.ssh/config`.

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
